import createClient from "openapi-fetch";

import type { paths } from "./api";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export const apiClient = createClient<paths>({
  baseUrl: API_URL,
  fetch: (request: Request) => {
    return fetch(new Request(request, { credentials: "include" }));
  },
});
