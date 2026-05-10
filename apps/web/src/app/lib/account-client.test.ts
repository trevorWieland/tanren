import { afterEach, describe, expect, it, vi } from "vitest";

import {
  AccountRequestError,
  myAccountCapabilities,
  myPermissionsWithCapabilityCheck,
  permissionScopesReadView,
  signOut,
  type MyPermissionsResponse,
} from "@/app/lib/account-client";

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("myAccountCapabilities", () => {
  it("requests session-scoped capabilities without a query string", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValueOnce(
      jsonResponse({
        can_view_my_permissions: true,
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await myAccountCapabilities();

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:8080/me/capabilities",
      {
        method: "GET",
        credentials: "include",
      },
    );
  });
});

describe("myPermissionsWithCapabilityCheck", () => {
  it("returns permission_denied without requesting permissions when capability is false", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValueOnce(
      jsonResponse({
        can_view_my_permissions: false,
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    let thrown: unknown;
    try {
      await myPermissionsWithCapabilityCheck();
    } catch (error: unknown) {
      thrown = error;
    }

    expect(thrown).toBeInstanceOf(AccountRequestError);
    expect((thrown as AccountRequestError).failure).toEqual({
      code: "permission_denied",
      summary: "",
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:8080/me/capabilities",
      {
        method: "GET",
        credentials: "include",
      },
    );
  });

  it("checks capability before each page read", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        jsonResponse({
          can_view_my_permissions: true,
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          organizations: [],
          projects: [],
          page: { limit: 100, next_cursor: "cursor-1" },
          read_metadata: {
            source: "permission-read-model",
            generated_at: "2026-01-01T00:00:00Z",
            staleness: "fresh",
            source_checkpoint: {
              max_permission_grant_id: "permission_grant:1",
              max_permission_constraint_id: null,
            },
          },
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          can_view_my_permissions: true,
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          organizations: [],
          projects: [],
          page: { limit: 100, next_cursor: null },
          read_metadata: {
            source: "permission-read-model",
            generated_at: "2026-01-01T00:00:01Z",
            staleness: "fresh",
            source_checkpoint: {
              max_permission_grant_id: "permission_grant:2",
              max_permission_constraint_id: null,
            },
          },
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    await myPermissionsWithCapabilityCheck();
    await myPermissionsWithCapabilityCheck({ cursor: "cursor-1" });

    expect(fetchMock).toHaveBeenCalledTimes(4);
    expect(fetchMock.mock.calls[0]).toEqual([
      "http://localhost:8080/me/capabilities",
      {
        method: "GET",
        credentials: "include",
      },
    ]);
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:8080/me/permissions",
    );
    expect(fetchMock.mock.calls[2]).toEqual([
      "http://localhost:8080/me/capabilities",
      {
        method: "GET",
        credentials: "include",
      },
    ]);
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:8080/me/permissions?cursor=cursor-1",
    );
  });
});

describe("permissionScopesReadView", () => {
  it("preserves page and read metadata while exposing sorted scopes", () => {
    const response: MyPermissionsResponse = {
      organizations: [
        {
          org_id: "org-zulu",
          permissions: [],
        },
      ],
      projects: [
        {
          project_id: "project-alpha",
          permissions: [],
        },
      ],
      page: {
        limit: 50,
        returned: 2,
        request_cursor: null,
        next_cursor: "cursor-2",
      },
      read_metadata: {
        source: "permission-read-model",
        generated_at: "2026-01-02T10:11:12Z",
        staleness: "fresh",
        source_checkpoint: {
          max_permission_grant_id: "permission_grant:12",
          max_permission_constraint_id: "permission_constraint:6",
        },
      },
    };

    const readView = permissionScopesReadView(response);

    expect(readView.scopes).toEqual([
      {
        kind: "organization",
        scope_id: "org-zulu",
        permissions: [],
      },
      {
        kind: "project",
        scope_id: "project-alpha",
        permissions: [],
      },
    ]);
    expect(readView.page.limit).toBe(50);
    expect(readView.page.next_cursor).toBe("cursor-2");
    expect(readView.page).toBe(response.page);
    expect(readView.read_metadata.source).toBe("permission-read-model");
    expect(readView.read_metadata.generated_at).toBe("2026-01-02T10:11:12Z");
    expect(readView.read_metadata.staleness).toBe("fresh");
    expect(readView.read_metadata).toBe(response.read_metadata);
    expect(readView.read_metadata.source_checkpoint).toEqual({
      max_permission_grant_id: "permission_grant:12",
      max_permission_constraint_id: "permission_constraint:6",
    });
  });
});

describe("signOut", () => {
  it("preserves interface error codes from revoke failures", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValueOnce(
      jsonResponse(
        {
          code: "permission_denied",
          summary: "Not allowed to revoke this session",
        },
        403,
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    let thrown: unknown;
    try {
      await signOut();
    } catch (error: unknown) {
      thrown = error;
    }

    expect(thrown).toBeInstanceOf(AccountRequestError);
    expect((thrown as AccountRequestError).failure).toEqual({
      code: "permission_denied",
      summary: "Not allowed to revoke this session",
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:8080/sessions/revoke",
      {
        method: "POST",
        credentials: "include",
      },
    );
  });

  it("falls back to normalized internal_error when revoke failure body is malformed", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValueOnce(
      new Response("{", {
        status: 500,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    let thrown: unknown;
    try {
      await signOut();
    } catch (error: unknown) {
      thrown = error;
    }

    expect(thrown).toBeInstanceOf(AccountRequestError);
    expect((thrown as AccountRequestError).failure).toEqual({
      code: "internal_error",
      summary: "HTTP 500",
    });
  });
});
