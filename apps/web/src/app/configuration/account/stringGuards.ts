export function hasNonEmptyTrimmedString(
  value: string | null | undefined,
): value is string {
  return typeof value === "string" && value.trim() !== "";
}
