import { createFileRoute } from "@tanstack/react-router";
import { TeamPage } from "../routePages";
import { parseTeamSection, type TeamSection } from "../teamSections";
import { parseLibraryEditorSearch, type LibraryEditorSearch } from "../libraryEditorRoute";

export const Route = createFileRoute("/team")({
  validateSearch: (
    search: Record<string, unknown>,
  ): { section?: TeamSection } & LibraryEditorSearch => ({
    section: parseTeamSection(search.section),
    ...parseLibraryEditorSearch(search),
  }),
  component: TeamPage,
});
