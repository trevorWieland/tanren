"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

import {
  PermissionFailureSection,
  PermissionMetadataSection,
  PermissionPageSection,
} from "./permission-sections";
import { useMyPermissionsPages } from "./use-my-permissions-pages";

export default function MyPermissionsPage(): ReactNode {
  const {
    pageViews,
    loading,
    failure,
    loadingMore,
    loadMoreFailure,
    nextCursor,
    hasPages,
    loadNextPage,
  } = useMyPermissionsPages();

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-5xl flex-col gap-6 px-4 py-6 sm:px-6 sm:py-8">
      <header className="space-y-2">
        <Link
          href="/"
          className="inline-flex rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-1 text-sm hover:bg-[--color-bg-elevated]"
        >
          {m.myPermissions_backToHome()}
        </Link>
        <h1 className="text-2xl font-semibold sm:text-3xl">
          {m.myPermissions_title()}
        </h1>
        <p className="text-sm text-[--color-fg-muted] sm:text-base">
          {m.myPermissions_subtitle()}
        </p>
      </header>

      {loading ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
          {m.myPermissions_loading()}
        </section>
      ) : failure !== null ? (
        <PermissionFailureSection failure={failure} />
      ) : !hasPages ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
          {m.myPermissions_empty()}
        </section>
      ) : (
        <div className="space-y-6">
          <PermissionMetadataSection
            pagesLoaded={pageViews.length}
            nextCursor={nextCursor}
            loadingMore={loadingMore}
            loadMoreFailure={loadMoreFailure}
            onLoadMore={loadNextPage}
          />
          <div className="space-y-6">
            {pageViews.map((pageView) => (
              <PermissionPageSection
                key={`permissions-page-${pageView.pageNumber}`}
                pageView={pageView}
              />
            ))}
          </div>
        </div>
      )}
    </main>
  );
}
