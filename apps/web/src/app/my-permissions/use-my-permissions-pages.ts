import { useCallback, useEffect, useMemo, useState } from "react";

import {
  AccountRequestError,
  myPermissionsWithCapabilityCheck,
  permissionScopes,
  type AccountFailure,
  type MyPermissionsResponse,
  type PermissionScopeView,
} from "@/app/lib/account-client";

export interface PermissionPageView {
  pageNumber: number;
  response: MyPermissionsResponse;
  organizationScopes: Extract<PermissionScopeView, { kind: "organization" }>[];
  projectScopes: Extract<PermissionScopeView, { kind: "project" }>[];
}

interface UseMyPermissionsPagesResult {
  pageViews: PermissionPageView[];
  loading: boolean;
  failure: AccountFailure | null;
  loadingMore: boolean;
  loadMoreFailure: AccountFailure | null;
  nextCursor: null | string;
  hasPages: boolean;
  loadNextPage: () => void;
}

function isOrganizationScope(
  scope: PermissionScopeView,
): scope is Extract<PermissionScopeView, { kind: "organization" }> {
  return scope.kind === "organization";
}

function isProjectScope(
  scope: PermissionScopeView,
): scope is Extract<PermissionScopeView, { kind: "project" }> {
  return scope.kind === "project";
}

function unknownFailure(cause: unknown): AccountFailure {
  return {
    code: "internal_error",
    summary: cause instanceof Error ? cause.message : String(cause),
  };
}

function permissionPageViews(
  pages: MyPermissionsResponse[],
): PermissionPageView[] {
  return pages.map((response, index) => {
    const scopes = permissionScopes(response);
    return {
      pageNumber: index + 1,
      response,
      organizationScopes: scopes.filter(isOrganizationScope),
      projectScopes: scopes.filter(isProjectScope),
    };
  });
}

export function useMyPermissionsPages(): UseMyPermissionsPagesResult {
  const [pages, setPages] = useState<MyPermissionsResponse[]>([]);
  const [failure, setFailure] = useState<AccountFailure | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreFailure, setLoadMoreFailure] = useState<AccountFailure | null>(
    null,
  );

  const pageViews = useMemo(() => permissionPageViews(pages), [pages]);
  const latestPage = pageViews.at(-1)?.response ?? null;
  const nextCursor = latestPage?.page.next_cursor ?? null;

  useEffect(() => {
    let cancelled = false;

    myPermissionsWithCapabilityCheck()
      .then((permissionsResponse) => {
        if (!cancelled) {
          setPages([permissionsResponse]);
        }
      })
      .catch((cause: unknown) => {
        if (cancelled) {
          return;
        }
        if (cause instanceof AccountRequestError) {
          setFailure(cause.failure);
          return;
        }
        setFailure(unknownFailure(cause));
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const loadNextPage = useCallback(() => {
    if (!nextCursor || loadingMore) {
      return;
    }
    setLoadingMore(true);
    setLoadMoreFailure(null);
    myPermissionsWithCapabilityCheck({ cursor: nextCursor })
      .then((response) => {
        setPages((current) => [...current, response]);
      })
      .catch((cause: unknown) => {
        if (cause instanceof AccountRequestError) {
          setLoadMoreFailure(cause.failure);
          return;
        }
        setLoadMoreFailure(unknownFailure(cause));
      })
      .finally(() => {
        setLoadingMore(false);
      });
  }, [nextCursor, loadingMore]);

  return {
    pageViews,
    loading,
    failure,
    loadingMore,
    loadMoreFailure,
    nextCursor,
    hasPages: pageViews.length > 0,
    loadNextPage,
  };
}
