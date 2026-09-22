import { useState, useCallback, useRef, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  listPromptGistsQuery,
  useCreatePromptGist,
  useUpdatePromptGist,
  useDeletePromptGist,
} from "../queries/promptGists";
import { m } from "../paraglide/messages.js";
import { Badge, Button, IconButton, Input, Spinner } from "./ui";
import { AnimatedCard, StaggerList } from "./rare-ui";
import { usePopover } from "./ModelPicker";
import { showAlert } from "./ui";
import { Copy, Lightbulb, Pencil, Plus, Trash2, X, Check } from "lucide-react";

interface GistFormState {
  name: string;
  content: string;
  description: string;
  tags: string;
  projectScoped: boolean;
}

const EMPTY_FORM: GistFormState = {
  name: "",
  content: "",
  description: "",
  tags: "",
  projectScoped: true,
};

function parseTags(input: string): string[] {
  return input
    .split(/[,\n]+/)
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
}

function GistForm({
  initial,
  projectId,
  onCancel,
  onSaved,
}: {
  initial?: { id: string; projectId: string | null } & GistFormState;
  projectId: string;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const [form, setForm] = useState<GistFormState>(
    initial ?? { ...EMPTY_FORM, projectScoped: true },
  );
  const createMutation = useCreatePromptGist();
  const updateMutation = useUpdatePromptGist();
  const busy = createMutation.isPending || updateMutation.isPending;

  const submit = useCallback(() => {
    const name = form.name.trim();
    const content = form.content;
    if (!name) {
      showAlert(m.prompt_gists_name_required(), "error");
      return;
    }
    if (!content) {
      showAlert(m.prompt_gists_content_required(), "error");
      return;
    }
    const fields = {
      name,
      content,
      description: form.description.trim(),
      tags: parseTags(form.tags),
    };
    const promise = initial
      ? updateMutation.mutateAsync({
          id: initial.id,
          projectId: initial.projectId,
          ...fields,
        })
      : createMutation.mutateAsync({
          projectId: form.projectScoped ? projectId : (null as string | null),
          ...fields,
        });
    promise.then(onSaved).catch((error: unknown) => {
      showAlert(error instanceof Error ? error.message : String(error), "error");
    });
  }, [form, initial, projectId, createMutation, updateMutation, onSaved]);

  return (
    <div className="flex flex-col gap-3">
      <Input
        placeholder={m.prompt_gists_name()}
        value={form.name}
        onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
      />
      <textarea
        className="w-full min-h-[8rem] rounded-md border border-border bg-background px-3 py-2 text-sm text-text placeholder:text-muted focus-visible:outline-2 focus-visible:outline-primary resize-y"
        placeholder={m.prompt_gists_content()}
        value={form.content}
        onChange={(e) => setForm((f) => ({ ...f, content: e.target.value }))}
      />
      <Input
        placeholder={m.prompt_gists_description()}
        value={form.description}
        onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
      />
      <Input
        placeholder={m.prompt_gists_tags()}
        value={form.tags}
        onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))}
      />
      {!initial && (
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={form.projectScoped}
            onChange={(e) => setForm((f) => ({ ...f, projectScoped: e.target.checked }))}
            className="rounded border-border"
          />
          {m.prompt_gists_project_scoped()}
        </label>
      )}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancel} disabled={busy}>
          {m.prompt_gists_cancel()}
        </Button>
        <Button variant="primary" onClick={submit} disabled={busy}>
          {initial ? m.prompt_gists_save() : m.prompt_gists_create()}
        </Button>
      </div>
    </div>
  );
}

export function PromptGistsTab({ projectId }: { projectId: string }) {
  const gistsQuery = useQuery(listPromptGistsQuery(projectId));
  const gists = gistsQuery.data ?? [];
  const deleteMutation = useDeletePromptGist();
  const [showCreate, setShowCreate] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const copy = useCallback(async (gist: { id: string; content: string }) => {
    try {
      await navigator.clipboard.writeText(gist.content);
      setCopiedId(gist.id);
      window.setTimeout(() => setCopiedId((current) => (current === gist.id ? null : current)), 1500);
    } catch {
      showAlert(m.file_tree_clipboard_unavailable(), "error");
    }
  }, []);

  const remove = useCallback(
    (gist: { id: string; projectId: string | null }) => {
      if (!window.confirm(m.prompt_gists_confirm_delete())) return;
      deleteMutation
        .mutateAsync({ id: gist.id, projectId: gist.projectId })
        .catch((error: unknown) =>
          showAlert(error instanceof Error ? error.message : String(error), "error"),
        );
    },
    [deleteMutation],
  );

  return (
    <div className="pane-content flex-1 min-h-0 relative overflow-y-auto bg-background p-4">
      <div className="flex items-center justify-between mb-4">
        <h2 className="m-0 text-base font-semibold text-text">{m.prompt_gists_title()}</h2>
        <Button
          size="small"
          variant="primary"
          onClick={() => {
            setEditingId(null);
            setShowCreate((value) => !value);
          }}
        >
          {showCreate ? (
            <X size={14} />
          ) : (
            <Plus size={14} />
          )}
          <span className="ms-1.5">{showCreate ? m.prompt_gists_cancel() : m.prompt_gists_new()}</span>
        </Button>
      </div>
      {showCreate && (
        <AnimatedCard className="mb-4 p-4" hover="none">
          <GistForm
            projectId={projectId}
            onCancel={() => setShowCreate(false)}
            onSaved={() => setShowCreate(false)}
          />
        </AnimatedCard>
      )}
      {gistsQuery.isPending ? (
        <div className="flex items-center gap-2 text-subtext py-6">
          <Spinner />
          {m.artifacts_tab_loading()}
        </div>
      ) : gistsQuery.error ? (
        <div className="py-6 text-subtext">{m.prompt_gists_failed_to_load()}</div>
      ) : gists.length === 0 ? (
        <div className="py-6 text-subtext">{m.prompt_gists_empty()}</div>
      ) : (
        <StaggerList className="flex flex-col gap-3">
          {gists.map((gist) => (
            <AnimatedCard key={gist.id} className="p-4" hover="lift">
              {editingId === gist.id ? (
                <GistForm
                  initial={{
                    id: gist.id,
                    projectId: gist.projectId,
                    name: gist.name,
                    content: gist.content,
                    description: gist.description ?? "",
                    tags: gist.tags.join(", "),
                    projectScoped: gist.projectId !== null,
                  }}
                  projectId={projectId}
                  onCancel={() => setEditingId(null)}
                  onSaved={() => setEditingId(null)}
                />
              ) : (
                <div className="flex flex-col gap-2">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <h3 className="m-0 text-sm font-semibold text-text truncate">{gist.name}</h3>
                        {gist.projectId === null && (
                          <Badge size="small" variant="default">{m.prompt_gists_global()}</Badge>
                        )}
                      </div>
                      {gist.description && (
                        <p className="m-0 mt-0.5 text-xs text-subtext">{gist.description}</p>
                      )}
                    </div>
                    <div className="flex items-center gap-1 shrink-0">
                      <IconButton
                        size="small"
                        title={m.prompt_gists_copy()}
                        aria-label={m.prompt_gists_copy()}
                        onClick={() => copy(gist)}
                      >
                        {copiedId === gist.id ? <Check size={14} /> : <Copy size={14} />}
                      </IconButton>
                      <IconButton
                        size="small"
                        title={m.prompt_gists_edit()}
                        aria-label={m.prompt_gists_edit()}
                        onClick={() => {
                          setShowCreate(false);
                          setEditingId(gist.id);
                        }}
                      >
                        <Pencil size={14} />
                      </IconButton>
                      <IconButton
                        size="small"
                        title={m.prompt_gists_delete()}
                        aria-label={m.prompt_gists_delete()}
                        onClick={() => remove(gist)}
                      >
                        <Trash2 size={14} />
                      </IconButton>
                    </div>
                  </div>
                  {gist.tags.length > 0 && (
                    <div className="flex flex-wrap gap-1">
                      {gist.tags.map((tag) => (
                        <Badge key={tag} size="small" variant="primary">{tag}</Badge>
                      ))}
                    </div>
                  )}
                  <pre className="m-0 mt-1 p-2.5 rounded-md bg-surface border border-border text-xs text-text whitespace-pre-wrap wrap-anywhere max-h-32 overflow-y-auto">
                    {gist.content}
                  </pre>
                </div>
              )}
            </AnimatedCard>
          ))}
        </StaggerList>
      )}
    </div>
  );
}

export function PromptGistPicker({
  projectId,
  textareaRef,
  draft,
  onDraftChange,
}: {
  projectId: string;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  draft: string;
  onDraftChange: (text: string, cursor: number) => void;
}) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const { open, setOpen, ref } = usePopover(triggerRef);
  const gistsQuery = useQuery(listPromptGistsQuery(projectId));
  const gists = gistsQuery.data ?? [];

  const insert = useCallback(
    (content: string) => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      const start = textarea.selectionStart ?? draft.length;
      const end = textarea.selectionEnd ?? start;
      const before = draft.slice(0, start);
      const after = draft.slice(end);
      const prefix = before.length > 0 && !before.endsWith(" ") && !before.endsWith("\n") ? " " : "";
      const suffix = after.length > 0 && !after.startsWith(" ") && !after.startsWith("\n") ? " " : "";
      const next = `${before}${prefix}${content}${suffix}${after}`;
      const cursor = start + prefix.length + content.length + suffix.length;
      onDraftChange(next, cursor);
      window.requestAnimationFrame(() => {
        textarea.focus();
        textarea.setSelectionRange(cursor, cursor);
      });
      setOpen(false);
    },
    [draft, textareaRef, onDraftChange, setOpen],
  );

  return (
    <div className="option-picker relative inline-flex shrink-0" ref={ref}>
      <IconButton
        ref={triggerRef}
        type="button"
        className="composer-bare"
        title={m.prompt_gists_insert()}
        aria-label={m.prompt_gists_insert()}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <Lightbulb size={16} />
      </IconButton>
      {open && (
        <div className="composer-sources-menu absolute bottom-[calc(100%_+_8px)] start-0 z-50 flex min-w-64 max-h-[min(24rem,60vh)] flex-col gap-1 rounded-md border border-border bg-background p-2 shadow-dropdown overflow-y-auto">
          <span className="px-1 text-sm font-medium text-muted">{m.prompt_gists_title()}</span>
          {gistsQuery.isPending ? (
            <div className="flex items-center gap-2 px-1 py-2 text-subtext text-sm">
              <Spinner />
              {m.artifacts_tab_loading()}
            </div>
          ) : gists.length === 0 ? (
            <div className="px-1 py-2 text-sm text-subtext">{m.prompt_gists_empty()}</div>
          ) : (
            gists.map((gist) => (
              <button
                key={gist.id}
                type="button"
                className="flex flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-start hover:bg-surface"
                onClick={() => insert(gist.content)}
              >
                <span className="text-sm text-text font-medium">{gist.name}</span>
                {gist.description && (
                  <span className="text-xs text-subtext line-clamp-1">{gist.description}</span>
                )}
                {gist.tags.length > 0 && (
                  <span className="flex flex-wrap gap-1">
                    {gist.tags.map((tag) => (
                      <Badge key={tag} size="small" variant="primary">{tag}</Badge>
                    ))}
                  </span>
                )}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
