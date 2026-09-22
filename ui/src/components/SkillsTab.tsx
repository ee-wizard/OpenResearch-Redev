import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { listUserSkillsQuery, listLatexTemplatesQuery } from "../queries/settings";
import {
  listChatAttachmentsQuery,
  listLibraryItemsQuery,
  useCreateLibraryItem,
  useDeleteLibraryItem,
} from "../queries/library";
import { m } from "../paraglide/messages.js";
import { ltr } from "../i18n";
import { RefreshCw, Trash2, Upload, Bot, WandSparkles, Plus } from "lucide-react";
import { useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import {
  deleteLatexTemplate,
  deleteUserSkill,
  fmtBytes,
  fmtNumber,
  timeAgo,
  uploadLatexTemplate,
  uploadUserSkill,
  type ChatAttachmentSkill,
  type ChatAttachmentSource,
  type LatexTemplate,
  type UserSkill,
  type LibraryItem,
  type LibraryKind,
} from "../api";
import { Badge, Button, IconButton, Input, Spinner } from "./ui";
import { usePopover } from "./ModelPicker";
import { StaggerList } from "./rare-ui";

const MAX_UPLOAD_BYTES = 20 * 1024 * 1024;

const CARD_CLASS_NAME =
  "bg-background border border-border rounded-lg py-4 px-4.5 mb-4 transition-[transform,box-shadow] duration-200 ease-standard motion-safe:hover:-translate-y-0.5 hover:shadow-elevated [&_h3]:mt-0 [&_h3]:mx-0 [&_h3]:mb-2.5 [&_h3]:text-base [&_h3]:font-semibold [&_h3]:text-text";
const CARD_SUB_CLASS_NAME = "mt-0 mx-0 mb-3 text-sm leading-relaxed text-text";
const SKILL_ROW_CLASS_NAME =
  "flex items-start gap-3 py-2.5 border-t border-t-border first:border-t-0";
const SKILL_NAME_CLASS_NAME = "text-sm font-normal text-text";
const ROW_DETAIL_CLASS_NAME = "mt-1 mb-0 text-sm leading-relaxed text-text";
const LIBRARY_PATH_CLASS_NAME = "m-0 text-sm leading-relaxed text-text wrap-anywhere";
const LIBRARY_CARD_CLASS_NAME =
  "flex h-full min-w-0 flex-col gap-2 rounded-lg border border-border bg-background p-3";
/** Container-query columns: the Library lives in panes of very different widths
 * (a resizable middle pane, or the wide standalone shell). */
const LIBRARY_GRID_CLASS_NAME = "grid grid-cols-1 gap-3 @2xl:grid-cols-2 @5xl:grid-cols-3";

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

function isAcceptedName(name: string): boolean {
  const lower = name.toLowerCase();
  return lower.endsWith(".md") || lower.endsWith(".markdown") || lower.endsWith(".zip");
}

function DropZone({
  accept,
  busy,
  prompt,
  onFile,
}: {
  accept: string;
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
        accept={accept}
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

/** Size and last-changed, shared by the skill and template rows. */
function RowMeta({ bytes, updatedAt }: { bytes: number; updatedAt: number }) {
  return (
    <div className="shrink-0 text-end whitespace-nowrap pt-0.5 text-xs text-subtext">
      {fmtBytes(bytes)}
      {updatedAt > 0 && <span className="text-muted"> · {timeAgo(updatedAt)}</span>}
    </div>
  );
}

/** Uploaded and discovered skills can both be removed from ORX. */
function SkillRow({
  skill,
  onError,
}: {
  skill: UserSkill;
  onError: (message: string) => void;
}) {
  const deleteUserSkillMutation = useMutation({ mutationFn: deleteUserSkill });

  const busy = deleteUserSkillMutation.isPending;
  return (
    <div className="flex items-center gap-2 py-1 border-t border-t-border first:border-t-0">
      <div className="flex-1 min-w-0 flex items-center gap-2">
        <span className={SKILL_NAME_CLASS_NAME}>{skill.name}</span>
        {skill.origin && <Badge size="small">{skill.origin}</Badge>}
      </div>
      <RowMeta bytes={skill.bytes} updatedAt={skill.updatedAt} />
      <IconButton
        size="small"
        data-tip={skill.origin ? m.skills_remove_imported() : m.skills_tab_delete_skill()}
        data-tip-align="end"
        aria-label={skill.origin ? m.skills_remove_imported_label({ name: ltr(skill.name) }) : m.skills_delete_skill_label({ name: ltr(skill.name) })}
        disabled={busy}
        onClick={() => {
          if (!window.confirm(skill.origin ? m.skills_remove_imported_confirm({ name: ltr(skill.name) }) : m.skills_delete_skill_confirm({ name: ltr(skill.name) }))) return;
          deleteUserSkillMutation.mutateAsync(skill.name)
            .catch((e) => {
              onError(e instanceof Error ? e.message : String(e));
            });
        }}
      >
        <Trash2 size={13} />
      </IconButton>
    </div>
  );
}

function LatexTemplateRow({
  template,
  onError,
}: {
  template: LatexTemplate;
  onError: (message: string) => void;
}) {
  const deleteLatexTemplateMutation = useMutation({ mutationFn: deleteLatexTemplate });

  const busy = deleteLatexTemplateMutation.isPending;
  const support = template.supportFiles.length;
  return (
    <div className={SKILL_ROW_CLASS_NAME}>
      <div className="flex-1 min-w-0">
        <span className="text-base font-medium text-text">{template.name}</span>
        <p className={ROW_DETAIL_CLASS_NAME}>
          {template.entry}
          {support > 0 &&
            (support === 1
              ? m.skills_one_support_file()
              : m.skills_support_files({ count: fmtNumber(support) }))}
        </p>
      </div>
      <RowMeta bytes={template.bytes} updatedAt={template.updatedAt} />
      <IconButton
        data-tip={m.skills_tab_delete_template()}
        data-tip-align="end"
        aria-label={m.skills_delete_template_label({ name: ltr(template.name) })}
        disabled={busy}
        onClick={() => {
          if (!window.confirm(m.skills_delete_template_confirm({ name: ltr(template.name) }))) return;
          deleteLatexTemplateMutation.mutateAsync(template.name)
            .catch((e) => {
              onError(e instanceof Error ? e.message : String(e));
            });
        }}
      >
        <Trash2 size={13} />
      </IconButton>
    </div>
  );
}

/** Everything the agent can invoke with `/name`: skills uploaded here, and the
 * ones already installed in the user's coding agents, mirrored automatically. */
function SkillsCard() {
  const uploadUserSkillMutation = useMutation({ mutationFn: uploadUserSkill });

  const skillsQuery = useQuery(listUserSkillsQuery());
  const skills = skillsQuery.data;
  const listRef = useRef<HTMLDivElement>(null);
  const [hasMoreAbove, setHasMoreAbove] = useState(false);
  const [hasMoreBelow, setHasMoreBelow] = useState(false);
  const updateScrollFade = useCallback(() => {
    const list = listRef.current;
    setHasMoreAbove(!!list && list.scrollTop > 1);
    setHasMoreBelow(!!list && list.scrollHeight - list.scrollTop - list.clientHeight > 1);
  }, []);

  useLayoutEffect(() => {
    updateScrollFade();
    const list = listRef.current;
    if (!list) return;
    const observer = new ResizeObserver(updateScrollFade);
    observer.observe(list);
    return () => observer.disconnect();
  }, [skills, updateScrollFade]);
  const refreshing = skillsQuery.isFetching;
  const loadError = skillsQuery.error?.message;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refresh = () => { void skillsQuery.refetch(); };

  const busyRef = useRef(false);
  const upload = useCallback(
    async (file: File) => {
      if (busyRef.current) return; // ignore a second drop/pick mid-upload
      setError(null);
      if (!isAcceptedName(file.name)) {
        setError(m.skills_upload_skill_error());
        return;
      }
      if (file.size > MAX_UPLOAD_BYTES) {
        setError(m.skills_file_too_large());
        return;
      }
      busyRef.current = true;
      setBusy(true);
      try {
        await uploadUserSkillMutation.mutateAsync({
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
    [],
  );

  return (
    <section className={CARD_CLASS_NAME}>
      {/* Baseline-aligned so the heading's own bottom margin still spaces the card. */}
      <div className="flex items-baseline gap-2.5">
        <h3>{m.skills_tab_skills()}</h3>
        <Button className="ms-auto" size="small" onClick={refresh} disabled={refreshing}>
          <RefreshCw
            size={12}
            className={refreshing ? "animate-[spin_0.9s_linear_infinite]" : ""}
          />{" "}
          {m.settings_page_refresh()}
        </Button>
      </div>
      <p className={CARD_SUB_CLASS_NAME}>{m.skills_description()}</p>

      <DropZone
        accept=".md,.markdown,.zip"
        busy={busy}
        prompt={<><span>{m.skills_drop_skill()}</span><span className="block ps-4 mt-1 text-text">{m.skills_agent_alternative()}</span></>}
        onFile={(file) => void upload(file)}
      />

      {error && (
        <div role="alert" className="mt-2.5 text-base text-accent-red whitespace-pre-wrap">
          {error}
        </div>
      )}

      {loadError && (
        <div role="alert" className="pt-3 text-base text-accent-red">
          {m.skills_tab_could_not_load_skills()} {loadError}
        </div>
      )}
      {skills === undefined ? (loadError ? null : (
        <div className="flex items-center gap-2 pt-3 text-sm text-subtext">
          <Spinner /> {m.skills_tab_loading_skills()}
        </div>
      )) : skills.length === 0 ? (
        <div className="pt-3 text-sm text-subtext">{m.skills_tab_no_skills_yet()}</div>
      ) : (
        <div className="relative mt-1">
          <div ref={listRef} onScroll={updateScrollFade} className="flex flex-col max-h-120 overflow-y-auto overscroll-contain">
            <StaggerList className="contents" itemHover="lift">
              {skills.map((s) => (
                <SkillRow key={s.name} skill={s} onError={setError} />
              ))}
            </StaggerList>
          </div>
          {hasMoreAbove && (
            <div aria-hidden="true" className="pointer-events-none absolute inset-x-0 top-0 h-10 bg-gradient-to-b from-background to-transparent" />
          )}
          {hasMoreBelow && (
            <div aria-hidden="true" className="pointer-events-none absolute inset-x-0 bottom-0 h-10 bg-gradient-to-t from-background to-transparent" />
          )}
        </div>
      )}
    </section>
  );
}

/** LaTeX templates the `orx-paper` skill follows instead of its built-in
 * preamble — a conference class, a lab style. */
function LatexTemplatesCard() {
  const uploadLatexTemplateMutation = useMutation({ mutationFn: uploadLatexTemplate });

  const templatesQuery = useQuery(listLatexTemplatesQuery());
  const templates = templatesQuery.data;
  const loadError = templatesQuery.error?.message;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const busyRef = useRef(false);
  const upload = useCallback(
    async (file: File) => {
      if (busyRef.current) return;
      setError(null);
      const lower = file.name.toLowerCase();
      if (!lower.endsWith(".tex") && !lower.endsWith(".zip")) {
        setError(m.skills_upload_template_error());
        return;
      }
      if (file.size > MAX_UPLOAD_BYTES) {
        setError(m.skills_file_too_large());
        return;
      }
      busyRef.current = true;
      setBusy(true);
      try {
        await uploadLatexTemplateMutation.mutateAsync({
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
    [],
  );

  return (
    <section className={CARD_CLASS_NAME}>
      <h3>{m.skills_tab_la_te_x_templates()}</h3>
      <p className={CARD_SUB_CLASS_NAME}>{m.skills_templates_description()}</p>

      <DropZone
        accept=".tex,.zip"
        busy={busy}
        prompt={<><span>{m.skills_drop_template()}</span><span className="block ps-4 mt-1 text-text">{m.templates_agent_alternative()}</span></>}
        onFile={(file) => void upload(file)}
      />

      {error && (
        <div role="alert" className="mt-2.5 text-base text-accent-red whitespace-pre-wrap">
          {error}
        </div>
      )}

      {loadError && (
        <div role="alert" className="pt-3 text-base text-accent-red">
          {m.skills_tab_could_not_load_templates()} {loadError}
        </div>
      )}
      {templates === undefined ? (loadError ? null : (
        <div className="flex items-center gap-2 pt-3 text-sm text-subtext">
          <Spinner /> {m.skills_tab_loading_templates()}
        </div>
      )) : templates.length === 0 ? (
        <div className="pt-3 text-sm text-subtext">{m.skills_tab_no_templates_yet()}</div>
      ) : (
        <div className="flex flex-col mt-1">
          <StaggerList className="contents" itemHover="lift">
            {templates.map((t) => (
              <LatexTemplateRow key={t.name} template={t} onError={setError} />
            ))}
          </StaggerList>
        </div>
      )}
    </section>
  );
}

export function sourceBadge(source: string) {
  switch (source) {
    case "builtin":
      return <Badge size="small">{m.library_built_in()}</Badge>;
    case "team":
      return (
        <Badge size="small" variant="primary">
          {m.library_team()}
        </Badge>
      );
    case "supervisor":
      return (
        <Badge size="small" variant="primary">
          {m.library_supervisor()}
        </Badge>
      );
    case "project":
      return <Badge size="small">{m.library_project()}</Badge>;
    case "user":
      return <Badge size="small" variant="primary">{m.library_source_user()}</Badge>;
    case "mirrored":
      return <Badge size="small">{m.library_source_mirrored()}</Badge>;
  }
}

export function kindIcon(kind: LibraryKind) {
  return kind === "agent" ? <Bot size={15} /> : <WandSparkles size={15} />;
}

function LibraryRow({
  item,
  onError,
}: {
  item: LibraryItem;
  onError: (message: string) => void;
}) {
  const navigate = useNavigate();
  const deleteMutation = useDeleteLibraryItem();

  // The whole card opens the editor page: a skill folder needs far more room
  // than a grid cell, and the page is deep-linkable from wherever it was opened.
  const openEditor = useCallback(() => {
    void navigate({
      to: "/team",
      search: { section: "library", kind: item.kind, source: item.source, id: item.id },
    });
  }, [navigate, item]);

  const remove = useCallback(async () => {
    if (!window.confirm(m.library_delete_confirm({ name: ltr(item.name) }))) return;
    try {
      await deleteMutation.mutateAsync({
        kind: item.kind,
        id: item.id,
        source: item.source,
      });
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [item, deleteMutation, onError]);

  return (
    <div className={LIBRARY_CARD_CLASS_NAME}>
      <div className="flex items-start gap-2">
        <button
          type="button"
          onClick={openEditor}
          title={ltr(item.dirPath)}
          className="flex min-w-0 flex-1 flex-col gap-1 rounded-md text-start"
        >
          <span className="flex flex-wrap items-center gap-2">
            {kindIcon(item.kind)}
            <span className="text-base font-medium text-text">{item.name}</span>
            {sourceBadge(item.source)}
          </span>
          <span className={LIBRARY_PATH_CLASS_NAME}>{item.filePath}</span>
          <span className="text-xs text-subtext">
            {item.files.length === 1
              ? m.library_one_file()
              : m.library_file_count({ count: fmtNumber(item.files.length) })}
          </span>
        </button>
        {(item.source === "team" || item.source === "supervisor") && (
          <IconButton
            size="small"
            data-tip={m.prompt_gists_delete()}
            aria-label={m.prompt_gists_delete()}
            onClick={() => void remove()}
            disabled={deleteMutation.isPending}
          >
            <Trash2 size={13} />
          </IconButton>
        )}
      </div>
    </div>
  );
}

/** One responsive grid per library section, sub-grouped by source so built-in,
 * team, and supervisor items stay distinguishable. */
function LibraryGrid({
  items,
  onError,
}: {
  items: LibraryItem[];
  onError: (message: string) => void;
}) {
  const groups = useMemo(
    () =>
      SKILL_SOURCE_ORDER.map((source) => [source, items.filter((item) => item.source === source)] as const)
        .filter(([, group]) => group.length > 0),
    [items],
  );

  return (
    <div className="mt-1 flex flex-col gap-4">
      {groups.map(([source, group]) => (
        <div key={source}>
          <div className="mb-2 text-xs font-medium tracking-[0.06em] text-muted uppercase">
            {SKILL_SOURCE_LABELS[source]}
          </div>
          <div className={LIBRARY_GRID_CLASS_NAME}>
            <StaggerList className="contents" itemHover="lift">
              {group.map((item) => (
                <LibraryRow
                  key={`${item.kind}-${item.id}-${item.source}`}
                  item={item}
                  onError={onError}
                />
              ))}
            </StaggerList>
          </div>
        </div>
      ))}
    </div>
  );
}

function LibraryCreateRow({
  kind,
  onDone,
  onError,
}: {
  kind: LibraryKind;
  onDone: () => void;
  onError: (message: string) => void;
}) {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<"file" | "zip">("file");
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [zipFile, setZipFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const createMutation = useCreateLibraryItem();

  const chooseZip = useCallback(
    (file: File) => {
      if (!file.name.toLowerCase().endsWith(".zip")) {
        onError(m.library_folder_zip_required());
        return;
      }
      if (file.size > MAX_UPLOAD_BYTES) {
        onError(m.skills_file_too_large());
        return;
      }
      setZipFile(file);
    },
    [onError],
  );

  const submit = useCallback(async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    if (mode === "zip" && !zipFile) {
      onError(m.library_folder_zip_required());
      return;
    }
    setBusy(true);
    try {
      const created = await createMutation.mutateAsync({
        kind,
        body:
          mode === "zip" && zipFile
            ? { name: trimmed, filename: zipFile.name, contentBase64: await fileToBase64(zipFile) }
            : { name: trimmed, content },
      });
      setName("");
      setContent("");
      setZipFile(null);
      setOpen(false);
      onDone();
      // Straight into the new folder: a skill the author cannot see is a skill
      // they cannot finish.
      void navigate({
        to: "/team",
        search: {
          section: "library",
          kind: created.item.kind,
          source: created.item.source,
          id: created.item.id,
        },
      });
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [kind, mode, name, content, zipFile, createMutation, onDone, onError, navigate]);

  if (!open) {
    return (
      <Button size="small" variant="ghost" onClick={() => setOpen(true)}>
        <Plus size={13} className="me-1" />
        {kind === "agent" ? m.library_create_agent() : m.library_create_skill()}
      </Button>
    );
  }

  const pending = busy || createMutation.isPending;
  return (
    <div className="flex flex-col gap-2 py-2">
      <div className="flex items-center gap-2">
        <Button
          size="small"
          variant={mode === "file" ? "primary" : "default"}
          aria-pressed={mode === "file"}
          disabled={pending}
          onClick={() => setMode("file")}
        >
          {m.library_single_file()}
        </Button>
        <Button
          size="small"
          variant={mode === "zip" ? "primary" : "default"}
          aria-pressed={mode === "zip"}
          disabled={pending}
          onClick={() => setMode("zip")}
        >
          {m.library_skill_folder()}
        </Button>
      </div>
      <Input
        value={name}
        onChange={(e) => setName(e.currentTarget.value)}
        placeholder={m.library_name_placeholder()}
        disabled={pending}
      />
      {mode === "file" ? (
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={10}
          placeholder={m.library_content_placeholder()}
          className="w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-sm text-text focus:outline-none focus:ring-2 focus:ring-primary"
        />
      ) : (
        <DropZone
          accept=".zip"
          busy={pending}
          prompt={
            zipFile ? (
              <>
                <span>{m.library_zip_selected({ filename: ltr(zipFile.name) })}</span>
                <span className="block ps-4 mt-1 text-text">{m.library_drop_folder()}</span>
              </>
            ) : (
              <span>{m.library_drop_folder()}</span>
            )
          }
          onFile={chooseZip}
        />
      )}
      <div className="flex gap-2">
        <Button size="small" variant="primary" onClick={() => void submit()} disabled={pending}>
          {m.prompt_gists_create()}
        </Button>
        <Button
          size="small"
          disabled={pending}
          onClick={() => {
            setOpen(false);
            setZipFile(null);
          }}
        >
          {m.prompt_gists_cancel()}
        </Button>
      </div>
    </div>
  );
}

function LibrarySection({
  kind,
  title,
}: {
  kind: LibraryKind;
  title: string;
}) {
  const [error, setError] = useState<string | null>(null);
  const builtinQuery = useQuery(listLibraryItemsQuery(kind, "builtin"));
  const teamQuery = useQuery(listLibraryItemsQuery(kind, "team"));
  const items = [...(builtinQuery.data ?? []), ...(teamQuery.data ?? [])];
  const loading = builtinQuery.isPending || teamQuery.isPending;
  const loadError = builtinQuery.error?.message ?? teamQuery.error?.message;

  return (
    <section className={`${CARD_CLASS_NAME} @container`}>
      <div className="flex items-baseline gap-2.5">
        <h3>{title}</h3>
        <Button
          className="ms-auto"
          size="small"
          onClick={() => {
            void builtinQuery.refetch();
            void teamQuery.refetch();
          }}
          disabled={loading}
        >
          <RefreshCw size={12} className={loading ? "animate-[spin_0.9s_linear_infinite]" : ""} />{" "}
          {m.settings_page_refresh()}
        </Button>
      </div>
      {error && (
        <div role="alert" className="mt-2.5 text-base text-accent-red whitespace-pre-wrap">
          {error}
        </div>
      )}
      {loadError && (
        <div role="alert" className="pt-3 text-base text-accent-red">
          {m.common_failed_to_load({ error: loadError })}
        </div>
      )}
      {loading ? (
        <div className="flex items-center gap-2 pt-3 text-sm text-subtext">
          <Spinner /> {m.common_loading()}
        </div>
      ) : items.length === 0 ? (
        <div className="pt-3 text-sm text-subtext">{m.library_empty()}</div>
      ) : (
        <LibraryGrid items={items} onError={setError} />
      )}
      <LibraryCreateRow kind={kind} onDone={() => { void teamQuery.refetch(); }} onError={setError} />
    </section>
  );
}

const SKILL_SOURCE_ORDER: ChatAttachmentSource[] = [
  "builtin",
  "team",
  "supervisor",
  "user",
  "mirrored",
  "project",
];

const SKILL_SOURCE_LABELS: Record<ChatAttachmentSource, string> = {
  builtin: m.library_built_in(),
  team: m.library_team(),
  supervisor: m.library_supervisor(),
  user: m.library_source_user(),
  mirrored: m.library_source_mirrored(),
  project: m.library_project(),
};

/** Composer picker that inserts a skill's content or an agent marker. */
export function LibraryPicker({
  textareaRef,
  draft,
  onDraftChange,
}: {
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  draft: string;
  onDraftChange: (text: string, cursor: number) => void;
}) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const { open, setOpen, ref } = usePopover(triggerRef);
  const attachmentsQuery = useQuery(listChatAttachmentsQuery());
  const skills = attachmentsQuery.data?.skills ?? [];
  const agents = attachmentsQuery.data?.agents ?? [];

  const groupedSkills = useMemo(() => {
    const bySource = new Map<ChatAttachmentSource, ChatAttachmentSkill[]>();
    for (const skill of skills) {
      const list = bySource.get(skill.source) ?? [];
      list.push(skill);
      bySource.set(skill.source, list);
    }
    return SKILL_SOURCE_ORDER.filter((source) => bySource.has(source)).map(
      (source) => [source, bySource.get(source)!] as const,
    );
  }, [skills]);

  const insert = useCallback(
    (value: string) => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      const start = textarea.selectionStart ?? draft.length;
      const end = textarea.selectionEnd ?? start;
      const before = draft.slice(0, start);
      const after = draft.slice(end);
      const prefix = before.length > 0 && !before.endsWith(" ") && !before.endsWith("\n") ? " " : "";
      const suffix = after.length > 0 && !after.startsWith(" ") && !after.startsWith("\n") ? " " : "";
      const next = `${before}${prefix}${value}${suffix}${after}`;
      const cursor = start + prefix.length + value.length + suffix.length;
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
        title={m.library_insert()}
        aria-label={m.library_insert()}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <WandSparkles size={16} />
      </IconButton>
      {open && (
        <div className="composer-sources-menu absolute bottom-[calc(100%_+_8px)] start-0 z-50 flex min-w-80 max-h-[min(24rem,60vh)] flex-col gap-1 rounded-md border border-border bg-background p-2 shadow-dropdown overflow-y-auto">
          <span className="px-1 text-sm font-medium text-muted">{m.library_insert()}</span>
          {attachmentsQuery.isPending ? (
            <div className="flex items-center gap-2 px-1 py-2 text-subtext text-sm">
              <Spinner />
              {m.common_loading()}
            </div>
          ) : skills.length === 0 && agents.length === 0 ? (
            <div className="px-1 py-2 text-sm text-subtext">{m.library_empty()}</div>
          ) : (
            <>
              {agents.length > 0 && (
                <div className="flex flex-col gap-1">
                  <span className="px-1 text-xs font-medium text-muted">{m.library_agents()}</span>
                  {agents.map((agent) => (
                    <button
                      key={`agent-${agent.id}`}
                      type="button"
                      className="flex items-center gap-2 rounded-md px-2 py-1.5 text-start hover:bg-surface"
                      onClick={() => insert(`@${agent.id}`)}
                      title={agent.installed ? m.library_agent_installed({ name: agent.name }) : m.library_agent_not_installed({ name: agent.name })}
                    >
                      <Bot size={14} />
                      <span className="text-sm text-text">{agent.name}</span>
                    </button>
                  ))}
                </div>
              )}
              {groupedSkills.map(([source, items]) => (
                <div key={`source-${source}`} className="flex flex-col gap-1">
                  <span className="px-1 text-xs font-medium text-muted">{SKILL_SOURCE_LABELS[source]}</span>
                  {items.map((skill) => (
                    <button
                      key={`skill-${skill.id}`}
                      type="button"
                      className="flex items-start gap-2 rounded-md px-2 py-1.5 text-start hover:bg-surface"
                      onClick={() => insert(skill.content ?? `/${skill.id}`)}
                      title={skill.filePath}
                    >
                      <WandSparkles size={14} className="mt-0.5 shrink-0" />
                      <div className="flex min-w-0 flex-col items-start">
                        <span className="text-sm text-text">{skill.name}</span>
                        <span className="max-w-full truncate text-xs text-subtext">{skill.filePath}</span>
                      </div>
                      {sourceBadge(skill.source)}
                    </button>
                  ))}
                </div>
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}

function SupervisorSection() {
  const [error, setError] = useState<string | null>(null);
  const query = useQuery(listLibraryItemsQuery("skill", "supervisor"));
  const items = query.data ?? [];
  const loading = query.isPending;
  const loadError = query.error?.message;

  return (
    <section className={`${CARD_CLASS_NAME} @container`}>
      <div className="flex items-baseline gap-2.5">
        <h3>{m.library_supervisor()}</h3>
        <Button
          className="ms-auto"
          size="small"
          onClick={() => void query.refetch()}
          disabled={loading}
        >
          <RefreshCw size={12} className={loading ? "animate-[spin_0.9s_linear_infinite]" : ""} />{" "}
          {m.settings_page_refresh()}
        </Button>
      </div>
      {error && (
        <div role="alert" className="mt-2.5 text-base text-accent-red whitespace-pre-wrap">
          {error}
        </div>
      )}
      {loadError && (
        <div role="alert" className="pt-3 text-base text-accent-red">
          {m.common_failed_to_load({ error: loadError })}
        </div>
      )}
      {loading ? (
        <div className="flex items-center gap-2 pt-3 text-sm text-subtext">
          <Spinner /> {m.common_loading()}
        </div>
      ) : items.length === 0 ? (
        <div className="pt-3 text-sm text-subtext">{m.library_supervisor_empty()}</div>
      ) : (
        <LibraryGrid items={items} onError={setError} />
      )}
    </section>
  );
}

/** Middle-pane Library tab — editable team skills, agent shims, plus the
 * uploaded user skills and LaTeX templates that already lived here. */
export function LibraryTab() {
  return (
    <div className="settings-view max-w-290 my-0 mx-auto pt-6 px-8 pb-15 [&_h1]:mt-0 [&_h1]:mx-0 [&_h1]:mb-1.5 [&_h1]:text-3xl">
      <h1>{m.library_title()}</h1>
      <p className="mt-0 mx-0 mb-5 text-base leading-relaxed text-text">
        {m.library_description()}
      </p>

      <LibrarySection kind="skill" title={m.skills_tab_skills()} />
      <LibrarySection kind="agent" title={m.library_agents()} />
      <SupervisorSection />
      <SkillsCard />
      <LatexTemplatesCard />
    </div>
  );
}

// Backward-compatible alias for any callers that still import SkillsTab.
export const SkillsTab = LibraryTab;
