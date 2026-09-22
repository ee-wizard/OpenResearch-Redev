import { Blocks, Book, FileText, Lightbulb, type LucideIcon } from "lucide-react";
import { m } from "./paraglide/messages.js";

/** The team-wide panels reachable without an open project, in nav order. */
export const TEAM_SECTIONS = ["library", "handbook", "promptGists", "teamPapers"] as const;
export type TeamSection = (typeof TEAM_SECTIONS)[number];

export const TEAM_SECTION_ICONS: Record<TeamSection, LucideIcon> = {
  library: Blocks,
  handbook: Book,
  promptGists: Lightbulb,
  teamPapers: FileText,
};

// Held as functions so a locale switch relabels the next render.
export const TEAM_SECTION_LABELS: Record<TeamSection, () => string> = {
  library: m.library_title,
  handbook: m.handbook_title,
  promptGists: m.prompt_gists_title,
  teamPapers: m.team_papers_title,
};

export const TEAM_SECTION_DESCRIPTIONS: Record<TeamSection, () => string> = {
  library: m.library_description,
  handbook: m.handbook_description,
  promptGists: m.prompt_gists_description,
  teamPapers: m.team_papers_description,
};

export function parseTeamSection(value: unknown): TeamSection | undefined {
  return TEAM_SECTIONS.find((section) => section === value);
}
