export const API_URL: string =
  typeof process !== "undefined" && process.env
    ? (process.env["VITE_API_URL"] ?? "http://localhost:8080")
    : "http://localhost:8080";
