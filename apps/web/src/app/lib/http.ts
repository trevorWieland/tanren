export function withJsonContentType(headers?: HeadersInit): Headers {
  const merged = new Headers(headers);
  if (!merged.has("content-type")) {
    merged.set("content-type", "application/json");
  }
  return merged;
}
