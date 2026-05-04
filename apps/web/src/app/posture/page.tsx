import type { ReactNode } from "react";

import type {
  ListPosturesResponse,
  PostureView,
  GetPostureResponse,
} from "@/app/lib/posture-client";
import { PosturePicker } from "@/components/posture/posture-picker";
import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

async function fetchPostures(): Promise<ListPosturesResponse> {
  const res = await fetch(`${API_URL}/v0/postures`, { cache: "no-store" });
  if (!res.ok) {
    throw new Error(`Failed to fetch postures: HTTP ${res.status}`);
  }
  return (await res.json()) as ListPosturesResponse;
}

async function fetchCurrentPosture(): Promise<PostureView | null> {
  const res = await fetch(`${API_URL}/v0/posture`, { cache: "no-store" });
  if (res.status === 424) {
    return null;
  }
  if (!res.ok) {
    return null;
  }
  const body = (await res.json()) as GetPostureResponse;
  return body.current;
}

export default async function PosturePage(): Promise<ReactNode> {
  const [listResult, current] = await Promise.all([
    fetchPostures(),
    fetchCurrentPosture(),
  ]);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.posture_title()}</h1>
      <PosturePicker postures={listResult.postures} current={current} />
    </main>
  );
}
