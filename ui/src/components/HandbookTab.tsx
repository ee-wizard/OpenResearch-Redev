import { useCallback, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Book, Edit, FileText, Save, X } from "lucide-react";
import { m } from "../paraglide/messages.js";
import { Md } from "./Md";
import { Button, Spinner } from "./ui";
import {
  getHandbookChapterQuery,
  listHandbookChaptersQuery,
  useSaveHandbookChapter,
} from "../queries/handbook";

export function HandbookTab() {
  const chaptersQuery = useQuery(listHandbookChaptersQuery());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const chapters = chaptersQuery.data ?? [];
  const activeId = selectedId ?? chapters[0]?.id ?? null;

  const activeChapter = useMemo(
    () => chapters.find((c) => c.id === activeId),
    [chapters, activeId],
  );

  const contentQuery = useQuery({
    ...getHandbookChapterQuery(activeId ?? ""),
    enabled: activeId !== null,
  });

  const saveMutation = useSaveHandbookChapter();

  const startEdit = useCallback(() => {
    setDraft(contentQuery.data ?? "");
    setEditing(true);
  }, [contentQuery.data]);

  const cancelEdit = useCallback(() => {
    setEditing(false);
    setDraft("");
  }, []);

  const save = useCallback(async () => {
    if (!activeId) return;
    try {
      await saveMutation.mutateAsync({ id: activeId, content: draft });
      setEditing(false);
      setDraft("");
    } catch {
      // Error surfaces through mutation state.
    }
  }, [activeId, draft, saveMutation]);

  return (
    <div className="flex h-full min-h-0">
      <aside className="w-68 shrink-0 flex flex-col border-e border-border bg-background">
        <div className="flex items-center gap-2 border-b border-border px-4 py-3">
          <Book size={16} className="text-text" />
          <h2 className="text-base font-semibold text-text">{m.handbook_title()}</h2>
        </div>
        <div className="flex-1 min-h-0 overflow-y-auto p-2">
          {chaptersQuery.isPending ? (
            <div className="flex items-center gap-2 p-2 text-sm text-subtext">
              <Spinner />
              {m.common_loading()}
            </div>
          ) : chapters.length === 0 ? (
            <div className="p-2 text-sm text-subtext">{m.handbook_empty()}</div>
          ) : (
            <nav className="flex flex-col gap-0.5">
              {chapters.map((chapter) => (
                <button
                  key={chapter.id}
                  type="button"
                  onClick={() => {
                    setSelectedId(chapter.id);
                    setEditing(false);
                    setDraft("");
                  }}
                  className={`flex items-start gap-2 rounded-md px-2.5 py-2 text-start text-sm leading-snug ${
                    activeId === chapter.id
                      ? "bg-panel font-medium text-text"
                      : "text-text hover:bg-surface"
                  }`}
                >
                  <FileText size={14} className="mt-0.5 shrink-0 text-subtext" />
                  <span>{chapter.title}</span>
                </button>
              ))}
            </nav>
          )}
        </div>
      </aside>

      <section className="flex flex-1 min-w-0 flex-col bg-canvas">
        <div className="flex items-center justify-between border-b border-border bg-background px-4 py-2">
          <h3 className="text-base font-medium text-text">
            {activeChapter?.title ?? m.handbook_title()}
          </h3>
          <div className="flex items-center gap-1">
            {editing ? (
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
              <Button size="small" onClick={startEdit} disabled={contentQuery.isPending}>
                <Edit size={13} className="me-1" />
                {m.handbook_edit()}
              </Button>
            )}
          </div>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto p-6">
          {contentQuery.isPending ? (
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
