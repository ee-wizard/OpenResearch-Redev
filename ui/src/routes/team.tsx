import { createFileRoute } from "@tanstack/react-router";
import { TeamPage } from "../routePages";
import { parseTeamSection, type TeamSection } from "../teamSections";

export const Route = createFileRoute("/team")({
  validateSearch: (search: Record<string, unknown>): { section?: TeamSection } => ({
    section: parseTeamSection(search.section),
  }),
  component: TeamPage,
});
