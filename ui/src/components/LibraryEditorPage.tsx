// Full-page editor for one library item. A skill (and an agent shim) is a
// *folder* — `SKILL.md` plus references, scripts, templates and assets — so the
// page pairs a file tree with one large editor instead of editing a single file
// in a grid card. The open file is part of the URL, so a reference document is
// deep-linkable; the grid is one back action away.

import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, FileText, FolderOpen, Plus, Trash2 } from "lucide-react";
import {
  fmtBytes,
  type LibraryFile,
  type LibraryItem,
  type LibraryKind,
  type LibrarySource,
} from "../api";
import { m } from "../paraglide/messages.js";
import { ltr } from "../i18n";
import {
  getLibraryItemQuery,
  useCreateLibraryFile,
  useDeleteLibraryFile,
  useUpdateLibraryItem,
} from "../queries/library";
import { CodeEditor } from "./CodeEditor";
import { kindIcon, sourceBadge } from "./SkillsTab";
import { Badge, Button, IconButton, Input, Spinner } from "./ui";

/** Sources this library does not own: whatever installed them owns the files, so
 * every write is disabled rather than failing on save. */
const EXTERNAL_SOURCES: readonly string[] = ["user", "mirrored", "project"];

interface FileNode {
  name: string;
  path: string;
  bytes: number;
}

/** Directories and files of one folder level, so the sidebar reads as the tree
 * the folder actually is. */
interface FileTree {
  dirs: Map<string, FileTree>;
  files: FileNode[];
}

function buildFileTree(files: LibraryFile[]): FileTree {
  const root: FileTree = { dirs: new Map(), files: [] };
  for (const file of files) {
    const segments = file.path.split("/");
    const name = segments.pop() ?? file.path;
    let node = root;
    for (const segment of segments) {
      const child = node.dirs.get(segment) ?? { dirs: new Map(), files: [] };
      node.dirs.set(segment, child);
      node = child;
    }
    node.files.push({ name, path: file.path, bytes: file.bytes });
  }
  return root;
}

function FileTreeRows({
  tree,
  activePath,
  primaryPath,
  readOnly,
  busy,
  onSelect,
  onDelete,
}: {
  tree: FileTree;
  activePath: string | undefined;
  primaryPath: string | undefined;
  readOnly: boolean;
  busy: boolean;
  onSelect: (path: string) => void;
  onDelete: (path: string) => void;
}) {
  const rows = (files: FileNode[]) =>
    files.map((file) => (
      <div key={file.path} className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={() => onSelect(file.path)}
          aria-current={activePath === file.path ? "true" : undefined}
          title={file.path === primaryPath ? m.library_primary_file() : file.path}
          className={`flex min-w-0 flex-1 items-center gap-2 rounded-md px-2.5 py-1.5 text-start font-mono text-xs ${
            activePath === file.path
              ? "bg-panel font-medium text-text"
              : "text-text hover:bg-surface"
          }`}
        >
          <span className="min-w-0 flex-1 truncate">{file.name}</span>
          {file.path === primaryPath && <Badge size="small">{m.library_primary_file()}</Badge>}
        </button>
        {file.path !== primaryPath && !readOnly && (
          <IconButton
            size="small"
            data-tip={m.library_delete_file()}
            aria-label={m.library_delete_file_label({ path: ltr(file.path) })}
            disabled={busy}
            onClick={() => onDelete(file.path)}
          >
            <Trash2 size={12} />
          </IconButton>
        )}
      </div>
    ));

  return (
    <>
      {rows(tree.files)}
      {[...tree.dirs].map(([name, child]) => (
        <div key={name} className="flex flex-col gap-0.5">
          <span className="flex items-center gap-1.5 px-2.5 pt-2 pb-0.5 text-xs text-muted">
            <FolderOpen size={12} />
            {name}
          </span>
          <div className="flex flex-col gap-0.5 ps-3">
            <FileTreeRows
              tree={child}
              activePath={activePath}
              primaryPath={primaryPath}
              readOnly={readOnly}
              busy={busy}
              onSelect={onSelect}
              onDelete={onDelete}
            />
          </div>
        </div>
      ))}
    </>
  );
}

/** One file of the item: the large syntax-highlighted editor plus its own
 * save/revert. Remounted per file, so a draft never leaks across files. */
function LibraryFileEditor({
  path,
  content,
  bytes,
  readOnly,
  saving,
  save,
}: {
  path: string;
  content: string;
  bytes: number;
  readOnly: boolean;
  saving: boolean;
  save: (content: string) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const value = draft ?? content;
  const dirty = draft !== null && draft !== content;

  return (
    <div className="file-view flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border bg-background px-4 py-2">
        <FileText size={14} className="text-subtext" />
        <span className="min-w-0 flex-1 truncate font-mono text-sm text-text">{path}</span>
        <span className="text-xs text-subtext">{fmtBytes(bytes)}</span>
        {dirty && (
          <span className="text-xs text-accent-amber" title={m.file_viewer_unsaved_tip()}>
            {m.file_viewer_unsaved()}
          </span>
        )}
        <Button
          size="small"
          variant="primary"
          disabled={readOnly || !dirty || saving}
          onClick={() => save(value)}
        >
          {saving ? m.common_saving() : m.common_save()}
        </Button>
        <Button size="small" disabled={readOnly || !dirty || saving} onClick={() => setDraft(null)}>
          {m.library_revert()}
        </Button>
      </div>
      <div className="flex-1 min-h-0">
        <CodeEditor
          value={value}
          onChange={setDraft}
          readOnly={readOnly}
          path={path}
          onSave={() => {
            if (!readOnly && dirty && !saving) save(value);
          }}
        />
      </div>
    </div>
  );
}

/** The item's content once its folder listing and one file have loaded. */
function LibraryItemEditor({
  kind,
  id,
  source,
  item,
  path,
  content,
  readOnly,
  onSelectFile,
}: {
  kind: LibraryKind;
  id: string;
  source: LibrarySource;
  item: LibraryItem;
  path: string;
  content: string;
  readOnly: boolean;
  onSelectFile: (path: string | undefined) => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newPath, setNewPath] = useState("");
  const updateMutation = useUpdateLibraryItem();
  const createFileMutation = useCreateLibraryFile();
  const deleteFileMutation = useDeleteLibraryFile();

  const primaryPath = item.files[0]?.path;
  const tree = buildFileTree(item.files);
  const bytes = item.files.find((file) => file.path === path)?.bytes ?? 0;

  const save = useCallback(
    (next: string) => {
      void updateMutation
        .mutateAsync({ kind, id, source, content: next, path })
        .then(() => setError(null))
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
    },
    [kind, id, source, path, updateMutation],
  );

  const createFile = useCallback(() => {
    const trimmed = newPath.trim();
    if (!trimmed) {
      setError(m.library_file_path_required());
      return;
    }
    void createFileMutation
      .mutateAsync({ kind, id, source, path: trimmed, content: "" })
      .then((created) => {
        setNewPath("");
        setCreating(false);
        setError(null);
        onSelectFile(created.path);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [kind, id, source, newPath, createFileMutation, onSelectFile]);

  const deleteFile = useCallback(
    (filePath: string) => {
      if (!window.confirm(m.library_delete_file_confirm({ path: ltr(filePath) }))) return;
      void deleteFileMutation
        .mutateAsync({ kind, id, source, path: filePath })
        .then(() => {
          setError(null);
          if (filePath === path) onSelectFile(undefined);
        })
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
    },
    [kind, id, source, path, deleteFileMutation, onSelectFile],
  );

  return (
    <div className="flex flex-1 min-h-0">
      <aside className="flex w-72 shrink-0 flex-col border-e border-border bg-background">
        <div className="flex items-center gap-2 border-b border-border px-3 py-2">
          <span className="text-sm font-medium text-text">{m.library_files()}</span>
          {!readOnly && (
            <IconButton
              size="small"
              className="ms-auto"
              data-tip={m.library_new_file()}
              aria-label={m.library_new_file()}
              disabled={creating}
              onClick={() => setCreating(true)}
            >
              <Plus size={13} />
            </IconButton>
          )}
        </div>
        {creating && (
          <div className="flex flex-col gap-2 border-b border-border p-2">
            <Input
              autoFocus
              value={newPath}
              placeholder={m.library_new_file_placeholder()}
              disabled={createFileMutation.isPending}
              onChange={(e) => setNewPath(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") createFile();
                if (e.key === "Escape") {
                  setCreating(false);
                  setNewPath("");
                }
              }}
            />
            <div className="flex gap-2">
              <Button
                size="small"
                variant="primary"
                disabled={createFileMutation.isPending}
                onClick={createFile}
              >
                {m.library_create_file()}
              </Button>
              <Button
                size="small"
                disabled={createFileMutation.isPending}
                onClick={() => {
                  setCreating(false);
                  setNewPath("");
                }}
              >
                {m.prompt_gists_cancel()}
              </Button>
            </div>
          </div>
        )}
        <div className="flex flex-1 min-h-0 flex-col gap-0.5 overflow-y-auto p-2">
          <FileTreeRows
            tree={tree}
            activePath={path}
            primaryPath={primaryPath}
            readOnly={readOnly}
            busy={deleteFileMutation.isPending}
            onSelect={(next) => onSelectFile(next)}
            onDelete={deleteFile}
          />
        </div>
        <div className="flex flex-col gap-2 border-t border-border p-3 text-xs text-subtext">
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-muted">{m.library_folder_path()}</span>
            <span className="break-all font-mono">{item.dirPath}</span>
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-muted">{m.library_primary_file()}</span>
            <span className="break-all font-mono">{item.filePath}</span>
          </div>
        </div>
      </aside>
      <section className="flex min-w-0 flex-1 flex-col">
        {error && (
          <div
            role="alert"
            className="shrink-0 border-b border-border px-4 py-2 text-sm text-accent-red whitespace-pre-wrap"
          >
            {error}
          </div>
        )}
        <div className="flex-1 min-h-0">
          <LibraryFileEditor
            key={path}
            path={path}
            content={content}
            bytes={bytes}
            readOnly={readOnly}
            saving={updateMutation.isPending}
            save={save}
          />
        </div>
      </section>
    </div>
  );
}

/** Full-page editor for one library item, addressed by kind/source/id and the
 * file inside it. Reached from the library grid in the `/team` shell and from
 * the in-project Library tab. */
export function LibraryEditorPage({
  kind,
  source,
  id,
  path,
  onSelectFile,
  onBack,
}: {
  kind: LibraryKind;
  source: LibrarySource;
  id: string;
  path?: string;
  onSelectFile: (path: string | undefined) => void;
  onBack: () => void;
}) {
  const itemQuery = useQuery(getLibraryItemQuery(kind, id, source, path));
  const item = itemQuery.data?.item;
  const activePath = itemQuery.data?.path;
  const readOnly = EXTERNAL_SOURCES.includes(source) || item?.editable === false;

  return (
    <div className="flex h-full min-h-0 flex-col bg-canvas">
      <header className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-border bg-background px-4 py-2">
        <Button size="small" onClick={onBack}>
          <ArrowLeft size={13} className="me-1" />
          {m.library_back_to_grid()}
        </Button>
        <span className="flex min-w-0 items-center gap-2">
          {kindIcon(kind)}
          <span className="min-w-0 truncate text-base font-medium text-text">{item?.name ?? id}</span>
          {sourceBadge(source)}
        </span>
      </header>
      {readOnly && (
        <div className="shrink-0 border-b border-border bg-surface px-4 py-2 text-sm text-subtext">
          {m.library_external_source_read_only()}
        </div>
      )}
      {itemQuery.error ? (
        <div role="alert" className="p-4 text-sm text-accent-red">
          {m.common_failed_to_load({ error: itemQuery.error.message })}
        </div>
      ) : item === undefined || activePath === undefined || itemQuery.data === undefined ? (
        <div className="flex flex-1 items-center justify-center">
          <Spinner />
        </div>
      ) : (
        <LibraryItemEditor
          kind={kind}
          id={id}
          source={source}
          item={item}
          path={activePath}
          content={itemQuery.data.content}
          readOnly={readOnly}
          onSelectFile={onSelectFile}
        />
      )}
    </div>
  );
}
