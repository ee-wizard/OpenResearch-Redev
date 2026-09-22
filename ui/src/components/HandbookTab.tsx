import { useCallback, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Book, Edit, FileText, Plus, Save, Trash2, X } from "lucide-react";
import { m } from "../paraglide/messages.js";
import { ltr } from "../i18n";
import type { HandbookScope } from "../api";
import { Md } from "./Md";
import { Badge, Button, IconButton, Input, Spinner, showAlert } from "./ui";
import {
  getHandbookChapterQuery,
  listHandbookChaptersQuery,
  useCreateHandbookChapter,
  useDeleteHandbookChapter,
  useSaveHandbookChapter,
} from "../queries/handbook";

function scopeBadge(scope: HandbookScope) {
  return scope === "project" ? (
    <Badge size="small" variant="primary">
      {m.handbook_scope_project()}
    </Badge>
  ) : (
    <Badge size="small">{m.handbook_scope_global()}</Badge>
  );
}

/** Handbook: the team-wide guide plus, inside a project, the chapters that
 * belong to it — per-project research notes never leak into the shared guide. */
export function HandbookTab({ projectId = null }: { projectId?: string | null }) {
  const chaptersQuery = useQuery(listHandbookChaptersQuery(projectId));
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [creating, setCreating] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [newContent, setNewContent] = useState("");
  const [newScope, setNewScope] = useState<HandbookScope>(projectId ? "project" : "global");

  const chapters = chaptersQuery.data ?? [];
  const activeChapter = useMemo(
    () => chapters.find((chapter) => chapter.id === selectedId) ?? chapters[0],
    [chapters, selectedId],
  );

  // A project listing mixes both scopes, so the body is fetched under the
  // selected chapter's own scope rather than the tab's.
  const contentQuery = useQuery({
    ...getHandbookChapterQuery(activeChapter?.id ?? "", activeChapter?.projectId ?? null),
    enabled: activeChapter !== undefined && !creating,
  });

  const saveMutation = useSaveHandbookChapter();
  const createMutation = useCreateHandbookChapter();
  const deleteMutation = useDeleteHandbookChapter();

  const startEdit = useCallback(() => {
    setDraft(contentQuery.data ?? "");
    setEditing(true);
  }, [contentQuery.data]);

  const cancelEdit = useCallback(() => {
    setEditing(false);
    setDraft("");
  }, []);

  const save = useCallback(async () => {
    if (!activeChapter) return;
    try {
      await saveMutation.mutateAsync({
        id: activeChapter.id,
        content: draft,
        projectId: activeChapter.projectId,
      });
      setEditing(false);
      setDraft("");
    } catch (error) {
      showAlert(error instanceof Error ? error.message : String(error), "error");
    }
  }, [activeChapter, draft, saveMutation]);

  const remove = useCallback(async () => {
    if (!activeChapter) return;
    if (!window.confirm(m.handbook_delete_confirm({ title: ltr(activeChapter.title) }))) return;
    try {
      await deleteMutation.mutateAsync({
        id: activeChapter.id,
        projectId: activeChapter.projectId,
      });
      setSelectedId(null);
      setEditing(false);
    } catch (error) {
      showAlert(error instanceof Error ? error.message : String(error), "error");
    }
  }, [activeChapter, deleteMutation]);

  const startCreate = useCallback(() => {
    setCreating(true);
    setEditing(false);
    setDraft("");
    setNewTitle("");
    setNewContent("");
    setNewScope(projectId ? "project" : "global");
  }, [projectId]);

  const submitCreate = useCallback(async () => {
    const title = newTitle.trim();
    if (!title) {
      showAlert(m.handbook_title_required(), "error");
      return;
    }
    try {
      const chapter = await createMutation.mutateAsync({
        projectId: newScope === "project" ? projectId : null,
        title,
        content: newContent,
      });
      setSelectedId(chapter.id);
      setCreating(false);
    } catch (error) {
      showAlert(error instanceof Error ? error.message : String(error), "error");
    }
  }, [newTitle, newContent, newScope, projectId, createMutation]);

  return (
    <div className="flex h-full min-h-0">
      <aside className="w-68 shrink-0 flex flex-col border-e border-border bg-background">
        <div className="flex items-center gap-2 border-b border-border px-4 py-3">
          <Book size={16} className="text-text" />
          <h2 className="text-base font-semibold text-text">{m.handbook_title()}</h2>
          <IconButton
            size="small"
            className="ms-auto"
            title={m.handbook_new()}
            aria-label={m.handbook_new()}
            disabled={creating}
            onClick={startCreate}
          >
            <Plus size={14} />
          </IconButton>
        </div>
        <div className="flex-1 min-h-0 overflow-y-auto p-2">
          {chaptersQuery.isPending ? (
            <div className="flex items-center gap-2 p-2 text-sm text-subtext">
              <Spinner />
              {m.common_loading()}
            </div>
          ) : chaptersQuery.error ? (
            <div role="alert" className="p-2 text-sm text-accent-red">
              {m.common_failed_to_load({ error: chaptersQuery.error.message })}
            </div>
          ) : chapters.length === 0 ? (
            <div className="p-2 text-sm text-subtext">{m.handbook_empty()}</div>
          ) : (
            <nav className="flex flex-col gap-0.5">
              {chapters.map((chapter) => (
                <button
                  key={`${chapter.scope}-${chapter.id}`}
                  type="button"
                  onClick={() => {
                    setSelectedId(chapter.id);
                    setCreating(false);
                    setEditing(false);
                    setDraft("");
                  }}
                  className={`flex items-start gap-2 rounded-md px-2.5 py-2 text-start text-sm leading-snug ${
                    !creating && activeChapter?.id === chapter.id
                      ? "bg-panel font-medium text-text"
                      : "text-text hover:bg-surface"
                  }`}
                >
                  <FileText size={14} className="mt-0.5 shrink-0 text-subtext" />
                  <span className="min-w-0 flex-1 break-words">{chapter.title}</span>
                  {scopeBadge(chapter.scope)}
                </button>
              ))}
            </nav>
          )}
        </div>
      </aside>

      <section className="flex flex-1 min-w-0 flex-col bg-canvas">
        <div className="flex items-center justify-between gap-2 border-b border-border bg-background px-4 py-2">
          {creating ? (
            <h3 className="text-base font-medium text-text">{m.handbook_new()}</h3>
          ) : (
            <div className="flex min-w-0 items-center gap-2">
              <h3 className="min-w-0 truncate text-base font-medium text-text">
                {activeChapter?.title ?? m.handbook_title()}
              </h3>
              {activeChapter && scopeBadge(activeChapter.scope)}
            </div>
          )}
          <div className="flex shrink-0 items-center gap-1">
            {creating ? (
              <>
                <Button
                  size="small"
                  variant="primary"
                  onClick={submitCreate}
                  disabled={createMutation.isPending}
                >
                  <Save size={13} className="me-1" />
                  {m.prompt_gists_create()}
                </Button>
                <Button
                  size="small"
                  onClick={() => setCreating(false)}
                  disabled={createMutation.isPending}
                >
                  <X size={13} className="me-1" />
                  {m.handbook_cancel()}
                </Button>
              </>
            ) : editing ? (
              <>
                <Button
                  size="small"
                  variant="primary"
                  onClick={save}
                  disabled={saveMutation.isPending}
                >
                  <Save size={13} className="me-1" />
                  {m.common_save()}
                </Button>
                <Button size="small" onClick={cancelEdit} disabled={saveMutation.isPending}>
                  <X size={13} className="me-1" />
                  {m.handbook_cancel()}
                </Button>
              </>
            ) : (
              activeChapter && (
                <>
                  <IconButton
                    size="small"
                    title={m.handbook_delete()}
                    aria-label={m.handbook_delete()}
                    disabled={deleteMutation.isPending || contentQuery.isPending}
                    onClick={() => void remove()}
                  >
                    <Trash2 size={14} />
                  </IconButton>
                  <Button size="small" onClick={startEdit} disabled={contentQuery.isPending}>
                    <Edit size={13} className="me-1" />
                    {m.handbook_edit()}
                  </Button>
                </>
              )
            )}
          </div>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto p-6">
          {creating ? (
            <div className="flex max-w-readable flex-col gap-3">
              <Input
                placeholder={m.handbook_title_placeholder()}
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
              />
              {projectId !== null && (
                <div className="flex items-center gap-2">
                  {(["project", "global"] as const).map((scope) => (
                    <Button
                      key={scope}
                      size="small"
                      variant={newScope === scope ? "primary" : "default"}
                      aria-pressed={newScope === scope}
                      onClick={() => setNewScope(scope)}
                    >
                      {scope === "project"
                        ? m.handbook_scope_project()
                        : m.handbook_scope_global()}
                    </Button>
                  ))}
                </div>
              )}
              <textarea
                value={newContent}
                onChange={(e) => setNewContent(e.target.value)}
                placeholder={m.library_content_placeholder()}
                spellCheck={false}
                className="min-h-[32rem] w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-sm text-text focus:outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
          ) : activeChapter === undefined ? (
            <div className="text-sm text-subtext">{m.handbook_empty()}</div>
          ) : contentQuery.isPending ? (
            <div className="flex items-center gap-2 text-sm text-subtext">
              <Spinner />
              {m.common_loading()}
            </div>
          ) : contentQuery.error ? (
            <div role="alert" className="text-sm text-accent-red">
              {m.common_failed_to_load({ error: contentQuery.error.message })}
            </div>
          ) : editing ? (
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              className="h-full min-h-[32rem] w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-sm text-text focus:outline-none focus:ring-2 focus:ring-primary"
              spellCheck={false}
            />
          ) : (
            <Md text={contentQuery.data ?? ""} />
          )}
        </div>
      </section>
    </div>
  );
}
