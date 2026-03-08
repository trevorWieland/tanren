# None vs Empty Lists

Use `None` to mean "not provided." Use `[]` to mean "provided but empty."

Rules:
- Optional list fields default to `None`
- If a phase runs and produces no items, return `[]`
- Do not omit required list fields; pass an empty list if there are no items
