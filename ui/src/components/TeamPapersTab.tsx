import { useCallback, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";

import { m } from "../paraglide/messages.js";
import { ltr } from "../i18n";
import { fmtNumber, timeAgo, type TeamPaper } from "../api";
import {
  getTeamPaperTextQuery,
  listTeamPapersQuery,
  useDeleteTeamPaper,
  useUpdateTeamPaper,
  useUploadTeamPaper,
} from "../queries/teamPapers";
import { Badge, Button, IconButton, Input, Spinner, showAlert } from "./ui";
import { AnimatedSection, StaggerList } from "./rare-ui";
import {
  ChevronDown,
  ChevronRight,
  FileText,
  Pencil,
  RefreshCw,
  Trash2,
  Upload,
} from "lucide-react";

/** Matches the Skills tab cap so the two upload surfaces agree. */
const MAX_UPLOAD_BYTES = 20 * 1024 * 1024;

const CARD_CLASS_NAME =
  "bg-background border border-border rounded-lg py-4 px-4.5 mb-4 transition-[transform,box-shadow] duration-200 ease-standard [&_h3]:mt-0 [&_h3]:mx-0 [&_h3]:mb-2.5 [&_h3]:text-base [&_h3]:font-semibold [&_h3]:text-text";

/** Read a File into base64 (strips the `data:...;base64,` prefix). */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("could not read file"));
        return;
      }
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("could not read file"));
    reader.readAsDataURL(file);
  });
}

function DropZone({
  busy,
  prompt,
  onFile,
}: {
  busy: boolean;
  prompt: ReactNode;
  onFile: (file: File) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <div
      className={`flex flex-col items-center justify-center gap-2 py-6.5 px-4.5 border-[1.5px] border-dashed rounded-md text-center text-sm text-text transition-[border-color,background] duration-120 ${
        busy ? "cursor-default" : "cursor-pointer"
        } ${
        dragging
          ? "border-primary bg-surface text-text"
          : "border-border-variant bg-surface [&:hover]:border-primary"
        }`}
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        if (busy) return;
        const file = e.dataTransfer.files?.[0];
        if (file) onFile(file);
      }}
      onClick={() => {
        if (!busy) inputRef.current?.click();
      }}
      role="button"
      tabIndex={0}
      aria-disabled={busy}
      aria-busy={busy}
      onKeyDown={(e) => {
        if ((e.key === "Enter" || e.key === " ") && !busy) {
          e.preventDefault();
          inputRef.current?.click();
        }
      }}
    >
      <input
        ref={inputRef}
        type="file"
        accept="application/pdf,.pdf"
        hidden
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) onFile(file);
          e.target.value = "";
        }}
      />
      {busy ? (
        <>
          <Spinner />
          <span>{m.skills_tab_uploading()}</span>
        </>
      ) : (
        <>
          <Upload size={20} strokeWidth={1.5} />
          <span>{prompt}</span>
        </>
      )}
    </div>
  );
}

/** Comma/newline separated free text into a trimmed, non-empty list. */
function parseList(input: string): string[] {
  return input
    .split(/[,\n]+/)
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

function PaperEditor({
  projectId,
  paper,
  onDone,
  onCancel,
}: {
  projectId: string;
  paper: TeamPaper;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState(paper.title ?? "");
  const [authors, setAuthors] = useState(paper.authors.join(", "));
  const [tags, setTags] = useState(paper.tags.join(", "));
  const [notes, setNotes] = useState(paper.notes ?? "");
  const [sourceUrl, setSourceUrl] = useState(paper.sourceUrl ?? "");
  const updateMutation = useUpdateTeamPaper();

  const submit = useCallback(() => {
    updateMutation
      .mutateAsync({
        projectId,
        id: paper.id,
        title: title.trim() || null,
        authors: parseList(authors),
        tags: parseList(tags),
        notes: notes.trim() || null,
        sourceUrl: sourceUrl.trim() || null,
      })
      .then(onDone)
      .catch((error: unknown) =>
        showAlert(error instanceof Error ? error.message : String(error), "error"),
      );
  }, [projectId, paper.id, title, authors, tags, notes, sourceUrl, updateMutation, onDone]);

  return (
    <div className="mt-2 flex flex-col gap-2">
      <Input
        placeholder={m.team_papers_edit_title()}
        value={title}
        onChange={(e) => setTitle(e.target.value)}
      />
      <Input
        placeholder={m.team_papers_edit_authors()}
        value={authors}
        onChange={(e) => setAuthors(e.target.value)}
      />
      <Input
        placeholder={m.team_papers_edit_tags()}
        value={tags}
        onChange={(e) => setTags(e.target.value)}
      />
      <Input
        placeholder={m.team_papers_edit_source_url()}
        value={sourceUrl}
        onChange={(e) => setSourceUrl(e.target.value)}
      />
      <textarea
        placeholder={m.team_papers_edit_notes()}
        value={notes}
        onChange={(e) => setNotes(e.target.value)}
        rows={3}
        className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-text placeholder:text-muted focus-visible:outline-2 focus-visible:outline-primary resize-y"
      />
      <div className="flex justify-end gap-2">
        <Button variant="ghost" size="small" onClick={onCancel} disabled={updateMutation.isPending}>
          {m.prompt_gists_cancel()}
        </Button>
        <Button
          variant="primary"
          size="small"
          onClick={submit}
          disabled={updateMutation.isPending}
        >
          {m.prompt_gists_save()}
        </Button>
      </div>
    </div>
  );
}

function PaperTextPreview({ projectId, paperId }: { projectId: string; paperId: string }) {
  const textQuery = useQuery(getTeamPaperTextQuery(projectId, paperId));
  if (textQuery.isPending) {
    return (
      <div className="flex items-center gap-2 text-sm text-subtext">
        <Spinner /> {m.common_loading()}
      </div>
    );
  }
  if (textQuery.error) {
    return (
      <div role="alert" className="text-sm text-accent-red">
        {m.team_papers_text_failed()}{" "}
        {textQuery.error instanceof Error ? textQuery.error.message : String(textQuery.error)}
      </div>
    );
  }
  if (!textQuery.data) {
    return <div className="text-sm text-subtext">{m.team_papers_no_text()}</div>;
  }
  return (
    <pre className="m-0 p-2.5 rounded-md bg-surface border border-border text-xs text-text whitespace-pre-wrap wrap-anywhere max-h-96 overflow-y-auto">
      {textQuery.data}
    </pre>
  );
}

function PaperRow({ projectId, paper }: { projectId: string; paper: TeamPaper }) {
  const [editing, setEditing] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const deleteMutation = useDeleteTeamPaper();
  const title = paper.title?.trim() || paper.filename;

  const remove = useCallback(() => {
    if (!window.confirm(m.team_papers_delete_confirm({ title: ltr(title) }))) return;
    deleteMutation
      .mutateAsync({ projectId, id: paper.id })
      .catch((error: unknown) =>
        showAlert(error instanceof Error ? error.message : String(error), "error"),
      );
  }, [projectId, paper.id, title, deleteMutation]);

  return (
    <div className={CARD_CLASS_NAME}>
      <div className="flex items-start gap-3">
        <FileText size={18} strokeWidth={1.5} className="mt-0.5 shrink-0 text-muted" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate">{title}</h3>
            {paper.pageCount != null && (
              <Badge size="small">{m.team_papers_pages({ count: fmtNumber(paper.pageCount) })}</Badge>
            )}
            <span className="text-xs text-muted">
              {m.team_papers_uploaded({ time: timeAgo(paper.createdAt) })}
            </span>
          </div>
          {paper.title?.trim() && (
            <p className="m-0 mt-0.5 text-xs text-subtext">{paper.filename}</p>
          )}
          {paper.authors.length > 0 && (
            <p className="m-0 mt-1 text-sm leading-relaxed text-text">
              {paper.authors.join(", ")}
            </p>
          )}
          {paper.tags.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1.5">
              {paper.tags.map((tag) => (
                <Badge key={tag} size="small" variant="primary">
                  {tag}
                </Badge>
              ))}
            </div>
          )}
          {paper.notes && (
            <p className="m-0 mt-1.5 text-sm leading-relaxed text-subtext">{paper.notes}</p>
          )}
          {paper.sourceUrl && (
            <a
              href={paper.sourceUrl}
              target="_blank"
              rel="noreferrer"
              className="mt-1.5 inline-block text-sm text-primary hover:underline"
            >
              {m.team_papers_source_link()}
            </a>
          )}
          {editing && (
            <PaperEditor
              projectId={projectId}
              paper={paper}
              onDone={() => setEditing(false)}
              onCancel={() => setEditing(false)}
            />
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <IconButton
            size="small"
            title={expanded ? m.team_papers_hide_text() : m.team_papers_show_text()}
            aria-label={expanded ? m.team_papers_hide_text() : m.team_papers_show_text()}
            onClick={() => setExpanded((value) => !value)}
          >
            {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </IconButton>
          <IconButton
            size="small"
            title={m.prompt_gists_edit()}
            aria-label={m.prompt_gists_edit()}
            disabled={editing}
            onClick={() => setEditing((value) => !value)}
          >
            <Pencil size={14} />
          </IconButton>
          <IconButton
            size="small"
            title={m.prompt_gists_delete()}
            aria-label={m.prompt_gists_delete()}
            disabled={deleteMutation.isPending}
            onClick={remove}
          >
            <Trash2 size={14} />
          </IconButton>
        </div>
      </div>
      {expanded && (
        <div className="mt-3 border-t border-t-border pt-3">
          <PaperTextPreview projectId={projectId} paperId={paper.id} />
        </div>
      )}
    </div>
  );
}

/** Right-panel Team Papers tab: upload PDFs the research agent reads in every
 * session, edit their metadata, and inspect the extracted text. */
export function TeamPapersTab({ projectId }: { projectId: string }) {
  const papersQuery = useQuery(listTeamPapersQuery(projectId));
  const uploadMutation = useUploadTeamPaper();
  const papers = papersQuery.data;
  const refreshing = papersQuery.isFetching;
  const loadError = papersQuery.error?.message;

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busyRef = useRef(false);

  const upload = useCallback(
    async (file: File) => {
      if (busyRef.current) return;
      setError(null);
      if (!file.name.toLowerCase().endsWith(".pdf")) {
        setError(m.team_papers_upload_error());
        return;
      }
      if (file.size > MAX_UPLOAD_BYTES) {
        setError(m.skills_file_too_large());
        return;
      }
      busyRef.current = true;
      setBusy(true);
      try {
        await uploadMutation.mutateAsync({
          projectId,
          filename: file.name,
          contentBase64: await fileToBase64(file),
        });
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        busyRef.current = false;
        setBusy(false);
      }
    },
    [projectId, uploadMutation],
  );

  return (
    <div className="pane-content flex-1 min-h-0 relative overflow-y-auto bg-background p-4">
      <div className="flex items-center justify-between mb-4">
        <h2 className="m-0 text-base font-semibold text-text">{m.team_papers_title()}</h2>
        <Button
          size="small"
          onClick={() => void papersQuery.refetch()}
          disabled={refreshing}
        >
          <RefreshCw
            size={12}
            className={refreshing ? "animate-[spin_0.9s_linear_infinite]" : ""}
          />{" "}
          {m.settings_page_refresh()}
        </Button>
      </div>
      <p className="mt-0 mx-0 mb-4 text-sm leading-relaxed text-subtext">
        {m.team_papers_description()}
      </p>

      <AnimatedSection className="mb-4">
        <DropZone
          busy={busy}
          prompt={m.team_papers_drop()}
          onFile={(file) => void upload(file)}
        />
      </AnimatedSection>

      {error && (
        <div role="alert" className="mb-3 text-sm text-accent-red whitespace-pre-wrap">
          {error}
        </div>
      )}
      {loadError && (
        <div role="alert" className="mb-3 text-sm text-accent-red">
          {m.team_papers_failed_to_load()} {loadError}
        </div>
      )}

      {papers === undefined ? (
        loadError ? null : (
          <div className="flex items-center gap-2 py-3 text-sm text-subtext">
            <Spinner /> {m.team_papers_loading()}
          </div>
        )
      ) : papers.length === 0 ? (
        <div className="py-3 text-sm text-subtext">{m.team_papers_empty()}</div>
      ) : (
        <StaggerList className="contents" itemHover="lift">
          {papers.map((paper) => (
            <PaperRow key={paper.id} projectId={projectId} paper={paper} />
          ))}
        </StaggerList>
      )}
    </div>
  );
}
