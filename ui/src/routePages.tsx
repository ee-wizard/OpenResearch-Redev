import { setScopedQueryData } from "./queries/client";
import { isCancelledError, useQuery } from "@tanstack/react-query";
import { listProjectsQuery, getUiStateQuery } from "./queries/projects";
import { useRouteContext, Link, useNavigate, type ErrorComponentProps } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { Blocks, Book, FileText, Lightbulb } from "lucide-react";

import { useRuntime } from "./RemoteRuntime";

import { clearReadDemoSessions } from "./demoSessionState";
import { globalResumeLocation, projectResumeLocation } from "./routeResume";
import { getRememberedGlobalWorkspace, globalWorkspaceWriter } from "./workspacePersistence";
import { initialPanelWidth } from "./panelLayout";
import { m } from "./paraglide/messages.js";
import { Onboarding } from "./components/Onboarding";
import { ProjectsHome } from "./components/ProjectsHome";
import { LibraryTab } from "./components/SkillsTab";
import { HandbookTab } from "./components/HandbookTab";
import { PromptGistsTab } from "./components/PromptGistsTab";
import { TeamPapersTab } from "./components/TeamPapersTab";
import { OfflineBanner } from "./components/OfflineBanner";
import { RemoteStatus } from "./components/RemoteStatus";
import { UpdateBanner, useUpdateStatus } from "./components/UpdateBanner";
import { Button, showAlert, Spinner } from "./components/ui";

export function RoutePending() {
  return <div className="flex flex-1 h-full items-center justify-center"><Spinner /></div>;
}

export function RouteNotFound() {
  return (
    <div className="flex flex-1 h-full flex-col items-center justify-center gap-3 text-subtext">
      <p>{m.model_picker_unavailable()}</p>
      <Link to="/projects">{m.app_projects()}</Link>
    </div>
  );
}

export function RouteFailure({ error, reset }: Pick<ErrorComponentProps, "error" | "reset">) {
  return (
    <div className="flex flex-1 h-full flex-col items-center justify-center gap-3 text-subtext">
      <p role="alert">{error.message}</p>
      <Button onClick={reset}>{m.app_retry()}</Button>
      <Link to="/projects">{m.app_projects()}</Link>
    </div>
  );
}

function Resume({ projectId }: { projectId?: string }) {
  const { queryClient: client } = useRouteContext({ from: "__root__" });
  const navigate = useNavigate();
  const [error, setError] = useState<Error | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let current = true;
    setError(null);
    void (projectId ? projectResumeLocation(projectId, client) : globalResumeLocation(client))
      .then((href) => { if (current) void navigate({ href, replace: true }); })
      .catch((cause: unknown) => {
        if (!current) return;
        // A shared read can be cancelled by invalidation or the last observer leaving.
        if (isCancelledError(cause)) setAttempt((value) => value + 1);
        else setError(cause instanceof Error ? cause : new Error(String(cause)));
      });
    return () => { current = false; };
  }, [projectId, attempt, navigate, client]);
  return error ? <RouteFailure error={error} reset={() => setAttempt((value) => value + 1)} /> : <RoutePending />;
}

export function ResumeGlobal() { return <Resume />; }
export function ResumeProject({ projectId }: { projectId: string }) { return <Resume projectId={projectId} />; }

export function ProjectsPage() {
  const runtime = useRuntime();
  const navigate = useNavigate();
  const projectsOptions = listProjectsQuery();
  const projectsQuery = useQuery(projectsOptions);
  const stateQuery = useQuery(getUiStateQuery());
  const projects = projectsQuery.data;
  const state = stateQuery.data;
  const error = projectsQuery.error ?? stateQuery.error;
  const retry = () => { void projectsQuery.refetch(); void stateQuery.refetch(); };
  const { status } = useUpdateStatus(runtime.kind === "local");
  useEffect(() => {
    document.title = "OpenResearch";
    if (!state) return;
    globalWorkspaceWriter.queue({
      ...(getRememberedGlobalWorkspace() ?? state.workspace ?? { railOpen: true, panelWidth: initialPanelWidth(), experimentsView: "table" }),
      lastLocation: "/projects",
    });
  }, [state]);
  const openProject = (projectId: string) => void navigate({ to: "/projects/$projectId", params: { projectId } });

  return (
    <div className="app flex flex-col h-full">
      {runtime.kind === "local" && <><OfflineBanner /><UpdateBanner status={status} /></>}
      {error && (!projects || !state) ? <RouteFailure error={error} reset={retry} />
        : !projects || !state ? <RoutePending />
          : projects.length === 0 && !state.onboardingCompleted ? (
            <Onboarding
              remote={runtime.kind === "ssh"}
              preferredAgent={state.preferredAgent}
              onDone={(project) => {
                clearReadDemoSessions();
                openProject(project.id);
              }}
            />
          ) : (
            <ProjectsHome
              remote={runtime.kind === "ssh"}
              projects={projects}
              onOpen={openProject}
              onOpenTeamLibrary={() => void navigate({ to: "/team" })}
              onCreated={(project, publicationError) => {
                if (publicationError) {
                  showAlert(publicationError, "error");
                  void navigate({ to: "/projects/$projectId/settings/$tab", params: { projectId: project.id, tab: "git" } });
                } else openProject(project.id);
              }}
              onDeleted={(id) => setScopedQueryData(projectsOptions.queryKey, (current) => current?.filter((project) => project.id !== id))}
            />
          )}
      {runtime.kind === "ssh" && <RemoteStatus runtime={runtime} corner />}
    </div>
  );
}

const TEAM_SECTIONS = ["library", "handbook", "promptGists", "teamPapers"] as const;
type TeamSection = (typeof TEAM_SECTIONS)[number];

const TEAM_SECTION_ICONS: Record<TeamSection, LucideIcon> = {
  library: Blocks,
  handbook: Book,
  promptGists: Lightbulb,
  teamPapers: FileText,
};

// Held as functions so a locale switch relabels the nav on the next render.
const TEAM_SECTION_LABELS: Record<TeamSection, () => string> = {
  library: m.library_title,
  handbook: m.handbook_title,
  promptGists: m.prompt_gists_title,
  teamPapers: m.team_papers_title,
};

/** Project-independent destination for the team-wide panels, so the Library,
 * Handbook, Prompt Gists, and Team Papers stay reachable with zero projects. */
export function TeamPage() {
  const [section, setSection] = useState<TeamSection>("library");
  useEffect(() => { document.title = m.team_shell_title(); }, []);
  return (
    <div className="app flex h-full flex-col bg-canvas">
      <header className="flex shrink-0 flex-wrap items-baseline gap-x-3 gap-y-1 border-b border-border bg-background px-4 py-3">
        <h1 className="m-0 text-xl font-semibold text-text">{m.team_shell_title()}</h1>
        <p className="m-0 text-sm text-subtext">{m.team_shell_description()}</p>
        <Link to="/projects" className="ms-auto text-sm text-primary hover:underline">
          {m.app_projects()}
        </Link>
      </header>
      <div className="flex flex-1 min-h-0">
        <nav
          aria-label={m.team_shell_title()}
          className="flex w-56 shrink-0 flex-col gap-0.5 border-e border-border bg-background p-2"
        >
          {TEAM_SECTIONS.map((id) => {
            const Icon = TEAM_SECTION_ICONS[id];
            return (
              <button
                key={id}
                type="button"
                aria-current={section === id ? "page" : undefined}
                className={`flex items-center gap-2.5 rounded-md px-2.5 py-[7px] text-start text-base text-text [&:hover:not(.active)]:bg-surface [&.active]:bg-panel [&.active]:font-medium ${section === id ? "active" : ""}`}
                onClick={() => setSection(id)}
              >
                <Icon size={15} />
                <span>{TEAM_SECTION_LABELS[id]()}</span>
              </button>
            );
          })}
        </nav>
        <section className="flex min-w-0 min-h-0 flex-1 flex-col">
          {section === "handbook" ? (
            <HandbookTab />
          ) : section === "library" ? (
            <div className="min-h-0 flex-1 overflow-y-auto [scrollbar-gutter:stable_both-edges]">
              <LibraryTab />
            </div>
          ) : section === "promptGists" ? (
            <PromptGistsTab />
          ) : (
            <TeamPapersTab />
          )}
        </section>
      </div>
    </div>
  );
}
