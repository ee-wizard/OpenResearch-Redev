// The library editor is a destination of the project-independent `/team` shell:
// which item and which of its files are open lives in the URL, so an editor page
// is deep-linkable and the browser's back button returns to the grid. Parsing
// lives apart from the route so the shell and the grid agree on one shape.

import type { LibraryKind, LibrarySource } from "./api";

/** One item, and optionally one file inside it; `path` omitted means the item's
 * primary file. */
export interface LibraryEditorTarget {
  kind: LibraryKind;
  source: LibrarySource;
  id: string;
  path?: string;
}

/** The subset of the `/team` search params that names an editor target. A
 * partial target is not a destination — the grid renders instead. */
export type LibraryEditorSearch = Partial<LibraryEditorTarget>;

const LIBRARY_KINDS: readonly LibraryKind[] = ["skill", "agent"];
const LIBRARY_SOURCES: readonly LibrarySource[] = ["builtin", "team", "project", "supervisor"];

export function parseLibraryEditorSearch(search: Record<string, unknown>): LibraryEditorSearch {
  const kind =
    typeof search.kind === "string" ? LIBRARY_KINDS.find((value) => value === search.kind) : undefined;
  const source =
    typeof search.source === "string"
      ? LIBRARY_SOURCES.find((value) => value === search.source)
      : undefined;
  const id = typeof search.id === "string" && search.id.trim() !== "" ? search.id : undefined;
  // Half a target would render an editor that cannot load anything, so an
  // incomplete one degrades to the plain library.
  if (!kind || !source || !id) return {};
  const path = typeof search.path === "string" && search.path.trim() !== "" ? search.path : undefined;
  return path ? { kind, source, id, path } : { kind, source, id };
}
