import { useCallback, useEffect, useMemo, useState } from "react";

import {
  AccountRequestError,
  myPermissionsWithCapabilityCheck,
  permissionScopesReadView,
  type AccountFailure,
  type PermissionScopesReadView,
  type PermissionScopeView,
} from "@/app/lib/account-client";

export interface PermissionPageView {
  pageNumber: number;
  page: PermissionScopesReadView["page"];
  read_metadata: PermissionScopesReadView["read_metadata"];
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
  pages: PermissionScopesReadView[],
): PermissionPageView[] {
  return pages.map((pageReadView, index) => {
    return {
      pageNumber: index + 1,
      page: pageReadView.page,
      read_metadata: pageReadView.read_metadata,
      organizationScopes: pageReadView.scopes.filter(isOrganizationScope),
      projectScopes: pageReadView.scopes.filter(isProjectScope),
    };
  });
}

export function useMyPermissionsPages(): UseMyPermissionsPagesResult {
  const [pages, setPages] = useState<PermissionScopesReadView[]>([]);
  const [failure, setFailure] = useState<AccountFailure | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreFailure, setLoadMoreFailure] = useState<AccountFailure | null>(
    null,
  );

  const pageViews = useMemo(() => permissionPageViews(pages), [pages]);
  const nextCursor = pageViews.at(-1)?.page.next_cursor ?? null;

  useEffect(() => {
    let cancelled = false;

    myPermissionsWithCapabilityCheck()
      .then((permissionsResponse) => {
        if (!cancelled) {
          setPages([permissionScopesReadView(permissionsResponse)]);
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
        setPages((current) => [...current, permissionScopesReadView(response)]);
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
