import { afterEach, describe, expect, it, vi } from "vitest";

import {
  AccountRequestError,
  myPermissionsWithCapabilityCheck,
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
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:8080/me/capabilities",
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:8080/me/permissions",
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:8080/me/capabilities",
    );
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:8080/me/permissions?cursor=cursor-1",
    );
  });
});
