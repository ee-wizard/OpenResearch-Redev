//! Editable team library for skills and agent shims.
//!
//! Built-in skills and agent shims are seeded as editable copies under the data
//! dir so teams can customize them from the web UI without touching the source
//! files embedded in the binary.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{anyhow, Result};
use crate::local::agent_skills::{skills, SkillSet};
use crate::store;

/// Kind of library item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryKind {
    Skill,
    Agent,
}

impl std::fmt::Display for LibraryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryKind::Skill => write!(f, "skill"),
            LibraryKind::Agent => write!(f, "agent"),
        }
    }
}

/// Source scope of a library item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LibrarySource {
    BuiltIn,
    Team,
    Project,
    Supervisor,
}

/// One file inside a library item's folder, as listed to the UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFile {
    /// Path relative to the item's folder, `/`-separated.
    pub path: String,
    pub bytes: u64,
}

/// One item in the team library.
///
/// A skill *is* a folder: its `SKILL.md` plus whatever references, scripts, and
/// assets it ships. [`LibraryItem::file_path`] stays the item's entry point for
/// callers that only ever wanted that one file, while
/// [`LibraryItem::dir_path`] and [`LibraryItem::files`] expose the folder the
/// API browses and edits.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub kind: LibraryKind,
    pub name: String,
    pub source: LibrarySource,
    /// The item's entry-point file: `SKILL.md`, or `shim.md` for agent shims.
    pub file_path: PathBuf,
    /// The item's folder — the unit the library actually stores.
    pub dir_path: PathBuf,
    /// Every regular file in the folder, entry point first, then by path.
    pub files: Vec<LibraryFile>,
    pub editable: bool,
}

impl LibraryItem {
    /// The file that makes an item an item — the one file that may never be
    /// deleted, and the default target of a read or write that names none.
    fn primary_file(kind: LibraryKind) -> &'static str {
        match kind {
            LibraryKind::Skill => "SKILL.md",
            LibraryKind::Agent => "shim.md",
        }
    }

    /// The entry-point file's path relative to the item's folder.
    pub fn primary_path(&self) -> &'static str {
        Self::primary_file(self.kind)
    }

    fn new(
        id: String,
        kind: LibraryKind,
        name: String,
        source: LibrarySource,
        dir: PathBuf,
    ) -> Self {
        let primary = Self::primary_file(kind);
        let file_path = dir.join(primary);
        let files = list_item_files(&dir, primary);
        Self {
            id,
            kind,
            name,
            source,
            file_path,
            dir_path: dir,
            files,
            editable: source != LibrarySource::Project,
        }
    }
}

/// A file inside an item's folder, as an operation touched it.
#[derive(Clone, Debug)]
pub struct ItemFile {
    /// Path relative to the item's folder, `/`-separated.
    pub path: String,
    /// The file's absolute path.
    pub absolute: PathBuf,
    /// The item's folder.
    pub dir: PathBuf,
}

/// Outcome of writing, creating, or deleting one file of an item, so the API can
/// answer 200/404/409 without parsing messages.
#[derive(Clone, Debug)]
pub enum FileOp {
    /// The operation succeeded.
    Done(ItemFile),
    /// There is no such file.
    Missing,
    /// A file already exists where the caller asked to create one.
    Conflict,
}

/// Every regular file under `dir`, `/`-separated and relative to it, with the
/// entry point first and the rest sorted by path.
///
/// Symlinks are skipped rather than followed: an item folder can be a cloned
/// repository, and a link that leaves the folder would never be served anyway —
/// [`validate_item_file_path`] refuses it. Iterative, so a deep tree cannot
/// overflow the stack.
fn list_item_files(dir: &Path, primary: &str) -> Vec<LibraryFile> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            // `file_type` never follows links, so a symlink is neither walked
            // into nor listed.
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() {
                let Ok(relative) = path.strip_prefix(dir) else {
                    continue;
                };
                let Ok(bytes) = entry.metadata().map(|md| md.len()) else {
                    continue;
                };
                files.push(LibraryFile {
                    path: slash_path(relative),
                    bytes,
                });
            }
        }
    }
    files.sort_by(|a, b| (a.path != primary, &a.path).cmp(&(b.path != primary, &b.path)));
    files
}

/// A path as the API spells it: `/`-separated, whatever the platform uses.
fn slash_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// `<data_dir>/library`
pub fn library_dir() -> PathBuf {
    store::data_dir().join("library")
}

/// Idempotently seed the library from built-in sources.
pub fn ensure_team_library() -> Result<()> {
    ensure_skills_library()?;
    ensure_agents_library()?;
    Ok(())
}

fn ensure_skills_library() -> Result<()> {
    let base = library_dir().join("skills");
    for skill in skills(SkillSet::Full) {
        let dir = base.join(skill.name);
        let path = dir.join("SKILL.md");
        // The entry point is seeded once and then owned by the team: it is never
        // overwritten. Its resources are seeded one by one, so an install that
        // predates a resource still receives it.
        if !path.exists() {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&path, skill.content)?;
        }
        for resource in skill.resources {
            let resource_path = dir.join(resource.path);
            if resource_path.exists() {
                continue;
            }
            if let Some(parent) = resource_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&resource_path, resource.content)?;
        }
    }
    Ok(())
}

fn ensure_agents_library() -> Result<()> {
    let base = library_dir().join("agents");
    for (id, _, shim) in builtin_agents() {
        let dir = base.join(id);
        let path = dir.join("shim.md");
        if !path.exists() {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&path, shim)?;
        }
    }
    // Seed the Codex legacy prompt separately.
    let codex_dir = base.join("codex");
    let legacy_path = codex_dir.join("legacy-prompt.md");
    if !legacy_path.exists() {
        std::fs::create_dir_all(&codex_dir)?;
        std::fs::write(&legacy_path, crate::local::harness::CODEX_PROMPT)?;
    }
    Ok(())
}

const SUPERVISOR_SKILLS_URL: &str = "git@github.com:HKUSTDial/Supervisor-Skills.git";

/// `<data_dir>/library/skills/.supervisor`
pub fn supervisor_skills_dir() -> PathBuf {
    library_dir().join("skills").join(".supervisor")
}

/// Directory holding the cloned skill packages. The Supervisor-Skills repository
/// keeps them one level down in `skills/`; a flat layout is honoured too, so an
/// upstream reorganisation cannot silently empty the listing.
fn supervisor_skills_base() -> PathBuf {
    supervisor_skills_base_in(&supervisor_skills_dir())
}

fn supervisor_skills_base_in(root: &Path) -> PathBuf {
    let nested = root.join("skills");
    if nested.is_dir() {
        nested
    } else {
        root.to_path_buf()
    }
}

/// Whether `path` holds a repository, i.e. a clone we can pull into. Checks the
/// metadata on disk rather than spawning git, so a missing git binary can never
/// make an intact clone look disposable.
fn is_clone(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Clone or pull the Supervisor-Skills repository so its skill packages are
/// available as a deletable built-in source. Returns an error when git is
/// unavailable or the network/SSH operation fails; callers decide whether to
/// treat this as fatal.
pub fn ensure_supervisor_skills() -> Result<()> {
    let path = supervisor_skills_dir();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // A clone that died before Git wrote the repository metadata leaves a tree
    // that `git pull` can never repair, so start over rather than error forever.
    if path.exists() && !is_clone(&path) {
        std::fs::remove_dir_all(&path).map_err(|e| {
            anyhow!(
                "Could not clear the incomplete supervisor skills clone at {}: {e}",
                path.display()
            )
        })?;
    }
    if path.exists() {
        let status = std::process::Command::new("git")
            .current_dir(&path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
            .args(["pull", "--ff-only"])
            .status()
            .map_err(|e| anyhow!("Failed to spawn git pull for supervisor skills: {e}"))?;
        if !status.success() {
            return Err(anyhow!("git pull failed for supervisor skills: {status}"));
        }
        return Ok(());
    }
    // Clone beside the library and rename into place: the skills directory then
    // appears atomically, so a reader never lists the half-checked-out tree.
    let staging = library_dir().join(format!(".supervisor-clone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let status = std::process::Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .args(["clone", "--depth", "1", SUPERVISOR_SKILLS_URL])
        .arg(&staging)
        .status()
        .map_err(|e| anyhow!("Failed to spawn git clone for supervisor skills: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(anyhow!("git clone failed for supervisor skills: {status}"));
    }
    if let Err(error) = std::fs::rename(&staging, &path) {
        let _ = std::fs::remove_dir_all(&staging);
        // Another process installed its own clone first; ours is redundant.
        if !path.exists() {
            return Err(anyhow!(
                "Could not install the supervisor skills clone at {}: {error}",
                path.display()
            ));
        }
    }
    Ok(())
}

/// List library items filtered by kind and source.
pub fn list_library_items(
    kind: Option<LibraryKind>,
    source: LibrarySource,
) -> Result<Vec<LibraryItem>> {
    ensure_team_library()?;
    let mut items = Vec::new();
    if kind != Some(LibraryKind::Agent) {
        items.extend(list_skills(source)?);
    }
    if kind != Some(LibraryKind::Skill) {
        items.extend(list_agents(source)?);
    }
    items.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
    Ok(items)
}

/// Fetch a single library item's metadata.
pub fn get_library_item(
    kind: LibraryKind,
    id: &str,
    source: LibrarySource,
) -> Result<Option<LibraryItem>> {
    ensure_team_library()?;
    validate_item_id(id)?;
    match kind {
        LibraryKind::Skill => get_skill_item(id, source),
        LibraryKind::Agent => get_agent_item(id, source),
    }
}

/// Read the content of a library item's entry-point file.
pub fn read_library_content(item: &LibraryItem) -> Result<String> {
    read_library_file(item, None)?
        .map(|(_, content)| content)
        .ok_or_else(|| anyhow!("Could not read {}", item.file_path.display()))
}

/// Read one file of an item's folder, as `(relative path, content)`. `path` is
/// relative to the folder and defaults to the entry point; an empty `path` is
/// the default too. `Ok(None)` means there is no such file, which the API
/// answers with 404.
pub fn read_library_file(
    item: &LibraryItem,
    path: Option<&str>,
) -> Result<Option<(String, String)>> {
    let (rel, file) = item_file_path(&item.dir_path, item_file_rel(item.kind, path))?;
    if !file.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&file)
        .map_err(|e| anyhow!("Could not read {}: {e}", file.display()))?;
    Ok(Some((rel, content)))
}

/// Write content to a library item's entry-point file, creating parent
/// directories as needed. Built-in edits update the editable copy in the data
/// dir.
pub fn write_library_content(
    kind: LibraryKind,
    id: &str,
    source: LibrarySource,
    content: &str,
) -> Result<PathBuf> {
    match write_library_file(kind, id, source, None, content)? {
        // Naming no file always writes the entry point, so it cannot go missing.
        FileOp::Done(file) => Ok(file.absolute),
        FileOp::Missing | FileOp::Conflict => Err(anyhow!("Could not write library item {id}")),
    }
}

/// Replace the content of one file of an item's folder. Naming no `path` writes
/// the entry point, exactly as this always did.
///
/// An explicit `path` must already be a file: creating one is
/// [`create_library_file`]'s job, so a typo cannot silently fork the item.
pub fn write_library_file(
    kind: LibraryKind,
    id: &str,
    source: LibrarySource,
    path: Option<&str>,
    content: &str,
) -> Result<FileOp> {
    ensure_team_library()?;
    validate_item_id(id)?;
    let dir = item_dir(kind, id, source).ok_or_else(|| not_editable(source))?;
    match path.filter(|path| !path.is_empty()) {
        Some(raw) => {
            let (rel, file) = item_file_path(&dir, raw)?;
            if !file.is_file() {
                return Ok(FileOp::Missing);
            }
            std::fs::write(&file, content)?;
            Ok(FileOp::Done(item_file(&dir, rel, file)))
        }
        None => {
            let (rel, file) = item_file_path(&dir, LibraryItem::primary_file(kind))?;
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file, content)?;
            Ok(FileOp::Done(item_file(&dir, rel, file)))
        }
    }
}

/// Create a new file inside an item's folder, creating parent directories. A
/// file already at the path is a conflict — updating it is
/// [`write_library_file`]'s job.
pub fn create_library_file(
    kind: LibraryKind,
    id: &str,
    source: LibrarySource,
    path: &str,
    content: &str,
) -> Result<FileOp> {
    ensure_team_library()?;
    validate_item_id(id)?;
    let dir = item_dir(kind, id, source).ok_or_else(|| not_editable(source))?;
    let (rel, file) = item_file_path(&dir, path)?;
    if file.exists() {
        return Ok(FileOp::Conflict);
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&file, content)?;
    Ok(FileOp::Done(item_file(&dir, rel, file)))
}

/// Delete one file of an item's folder. The entry point cannot be deleted: an
/// item without its `SKILL.md`/`shim.md` is not an item any more.
pub fn delete_library_file(
    kind: LibraryKind,
    id: &str,
    source: LibrarySource,
    path: &str,
) -> Result<FileOp> {
    ensure_team_library()?;
    validate_item_id(id)?;
    let dir = item_dir(kind, id, source).ok_or_else(|| not_editable(source))?;
    let (rel, file) = item_file_path(&dir, path)?;
    if rel == LibraryItem::primary_file(kind) {
        return Err(anyhow!("Cannot delete {rel}: it is the item's entry point"));
    }
    if !file.is_file() {
        return Ok(FileOp::Missing);
    }
    std::fs::remove_file(&file)?;
    Ok(FileOp::Done(item_file(&dir, rel, file)))
}

/// Resolve a caller-supplied path for a file inside an item's folder, as
/// `(relative path, absolute path)`.
///
/// The same value reaches the API two ways — a `?path=` query value the
/// extractor has already percent-decoded, and a JSON body field that arrives
/// verbatim — so it is decoded here exactly once, never trusting the spelling
/// that arrived, and only then validated against the item folder. An encoded
/// separator therefore behaves exactly like a literal one: `..%2F..%2Fescaped.md`
/// is refused in either form instead of landing as a file with that name.
///
/// A percent-escape that is not valid hex is left alone rather than refused, so
/// a file the team named `100%.md` stays addressable.
pub fn item_file_path(item_dir: &Path, raw: &str) -> Result<(String, PathBuf)> {
    let decoded =
        urlencoding::decode(raw).map_err(|e| anyhow!("Invalid library file path: {raw} ({e})"))?;
    let absolute = validate_item_file_path(item_dir, &decoded)?;
    Ok((decoded.into_owned(), absolute))
}

/// The caller-supplied path a request addresses, defaulting to the item's
/// entry-point file when it names none (or names an empty one).
fn item_file_rel(kind: LibraryKind, path: Option<&str>) -> &str {
    path.filter(|path| !path.is_empty())
        .unwrap_or(LibraryItem::primary_file(kind))
}

fn item_file(dir: &Path, rel: String, absolute: PathBuf) -> ItemFile {
    ItemFile {
        path: rel,
        absolute,
        dir: dir.to_path_buf(),
    }
}

/// Create a new team-owned library item holding a single entry-point file.
pub fn create_team_library_item(
    kind: LibraryKind,
    name: &str,
    content: &str,
) -> Result<LibraryItem> {
    ensure_team_library()?;
    let id = slugify_name(name)?;
    validate_team_id(kind, &id)?;
    let dir = team_item_dir(kind, &id);
    if dir.exists() {
        return Err(anyhow!(
            "A team {kind} named '{name}' already exists",
            kind = kind_label(kind)
        ));
    }
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(LibraryItem::primary_file(kind));
    std::fs::write(&path, content)?;
    let display_name = parse_name_from_frontmatter(content).unwrap_or_else(|| name.to_string());
    Ok(LibraryItem::new(
        id,
        kind,
        display_name,
        LibrarySource::Team,
        dir,
    ))
}

/// Create a new team-owned item from a `.zip` of a whole folder — the shape a
/// skill actually has, with its references, scripts, and assets beside the
/// `SKILL.md`. The archive is held to the same caps and path rules as a user
/// skill upload, and must carry the item's entry-point file.
pub fn create_team_library_item_from_zip(
    kind: LibraryKind,
    name: &str,
    zip_bytes: &[u8],
) -> Result<LibraryItem> {
    ensure_team_library()?;
    let id = slugify_name(name)?;
    validate_team_id(kind, &id)?;
    let dir = team_item_dir(kind, &id);
    if dir.exists() {
        return Err(anyhow!(
            "A team {kind} named '{name}' already exists",
            kind = kind_label(kind)
        ));
    }
    let files =
        crate::local::user_skills::extract_zip_folder(zip_bytes, LibraryItem::primary_file(kind))?;
    std::fs::create_dir_all(&dir)?;
    // A folder half written from a rejected archive is not an item: leave
    // nothing behind the caller could mistake for one.
    let written = write_item_files(&dir, files);
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(error);
    }
    let primary = std::fs::read_to_string(dir.join(LibraryItem::primary_file(kind))).ok();
    let display_name = primary
        .as_deref()
        .and_then(parse_name_from_frontmatter)
        .unwrap_or_else(|| name.to_string());
    Ok(LibraryItem::new(
        id,
        kind,
        display_name,
        LibrarySource::Team,
        dir,
    ))
}

/// Write extracted archive entries under an item folder, each path re-checked so
/// nothing a zip spells can land outside it.
fn write_item_files(dir: &Path, files: Vec<(String, Vec<u8>)>) -> Result<()> {
    for (rel, bytes) in files {
        let dest = validate_item_file_path(dir, &rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, bytes)?;
    }
    Ok(())
}

/// Delete a library item. Returns `true` if it existed.
/// Built-in items cannot be deleted; supervisor skills can be deleted, which
/// removes the cloned directory.
pub fn delete_library_item(kind: LibraryKind, id: &str, source: LibrarySource) -> Result<bool> {
    ensure_team_library()?;
    validate_item_id(id)?;
    let dir = match (kind, source) {
        (LibraryKind::Skill, LibrarySource::Team) | (LibraryKind::Agent, LibrarySource::Team) => {
            team_item_dir(kind, id)
        }
        (LibraryKind::Skill, LibrarySource::Supervisor) => supervisor_skills_base().join(id),
        _ => return Err(anyhow!("Cannot delete {source:?} {kind}")),
    };
    if !dir.exists() {
        return Ok(false);
    }
    match kind {
        LibraryKind::Skill if is_builtin_skill(id) && source == LibrarySource::Team => {
            return Err(anyhow!("Cannot delete built-in skill {id}"))
        }
        LibraryKind::Agent if is_builtin_agent(id) => {
            return Err(anyhow!("Cannot delete built-in agent {id}"))
        }
        _ => {}
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(true)
}

// --- built-in catalogs ------------------------------------------------------

/// Built-in agent shims: (id, display name, primary shim content).
pub(crate) fn builtin_agents() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "claude-code",
            "Claude Code",
            crate::local::harness::CLAUDE_SKILL,
        ),
        ("codex", "Codex", crate::local::harness::CLAUDE_SKILL),
        ("opencode", "OpenCode", crate::local::harness::CLAUDE_SKILL),
        ("cursor", "Cursor", crate::local::harness::CLAUDE_SKILL),
        (
            "antigravity",
            "Antigravity",
            crate::local::harness::CLAUDE_SKILL,
        ),
        ("oh-my-pi", "Oh My Pi", crate::local::harness::CLAUDE_SKILL),
    ]
}

fn is_builtin_skill(name: &str) -> bool {
    skills(SkillSet::Full).iter().any(|s| s.name == name)
}

fn is_builtin_agent(id: &str) -> bool {
    builtin_agents().into_iter().any(|(bid, _, _)| bid == id)
}

// --- listing ----------------------------------------------------------------

fn list_skills(source: LibrarySource) -> Result<Vec<LibraryItem>> {
    match source {
        LibrarySource::BuiltIn => list_builtin_skills(),
        LibrarySource::Team => list_team_dirs(
            &library_dir().join("skills").join("team"),
            LibraryKind::Skill,
            LibrarySource::Team,
        ),
        LibrarySource::Project => Ok(Vec::new()),
        LibrarySource::Supervisor => list_supervisor_skills(),
    }
}

fn list_supervisor_skills() -> Result<Vec<LibraryItem>> {
    list_supervisor_skills_in(&supervisor_skills_base())
}

fn list_supervisor_skills_in(base: &Path) -> Result<Vec<LibraryItem>> {
    let mut items = Vec::new();
    if !base.exists() {
        return Ok(items);
    }
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let file_path = path.join("SKILL.md");
        if !file_path.exists() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let name = std::fs::read_to_string(&file_path)
            .ok()
            .and_then(|content| parse_name_from_frontmatter(&content))
            .unwrap_or_else(|| id.clone());
        items.push(LibraryItem::new(
            id,
            LibraryKind::Skill,
            name,
            LibrarySource::Supervisor,
            path,
        ));
    }
    Ok(items)
}

fn list_builtin_skills() -> Result<Vec<LibraryItem>> {
    let base = library_dir().join("skills");
    let mut items = Vec::new();
    for skill in skills(SkillSet::Full) {
        let dir = base.join(skill.name);
        if dir.join("SKILL.md").exists() {
            items.push(LibraryItem::new(
                skill.name.to_string(),
                LibraryKind::Skill,
                skill.name.to_string(),
                LibrarySource::BuiltIn,
                dir,
            ));
        }
    }
    Ok(items)
}

fn list_agents(source: LibrarySource) -> Result<Vec<LibraryItem>> {
    let base = library_dir().join("agents");
    let mut items = Vec::new();
    if !base.exists() {
        return Ok(items);
    }
    let builtin_ids: HashSet<&str> = builtin_agents().into_iter().map(|(id, _, _)| id).collect();
    for entry in std::fs::read_dir(&base)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let is_builtin = builtin_ids.contains(id.as_str());
        let matches_source = match source {
            LibrarySource::BuiltIn => is_builtin,
            LibrarySource::Team => !is_builtin,
            LibrarySource::Project | LibrarySource::Supervisor => false,
        };
        if !matches_source {
            continue;
        }
        let name = builtin_agents()
            .iter()
            .find(|(bid, _, _)| *bid == id.as_str())
            .map(|(_, name, _)| name.to_string())
            .unwrap_or_else(|| id.clone());
        items.push(LibraryItem::new(id, LibraryKind::Agent, name, source, path));
    }
    Ok(items)
}

fn list_team_dirs(
    dir: &Path,
    kind: LibraryKind,
    source: LibrarySource,
) -> Result<Vec<LibraryItem>> {
    let mut items = Vec::new();
    if !dir.exists() {
        return Ok(items);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let file_path = path.join(LibraryItem::primary_file(kind));
        let name = if file_path.exists() {
            std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|content| parse_name_from_frontmatter(&content))
                .unwrap_or_else(|| id.clone())
        } else {
            id.clone()
        };
        items.push(LibraryItem::new(id, kind, name, source, path));
    }
    Ok(items)
}

fn get_skill_item(id: &str, source: LibrarySource) -> Result<Option<LibraryItem>> {
    let Some(dir) = item_dir(LibraryKind::Skill, id, source) else {
        return Ok(None);
    };
    let path = dir.join("SKILL.md");
    if !path.exists() {
        return Ok(None);
    }
    let name = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| parse_name_from_frontmatter(&content))
        .unwrap_or_else(|| id.to_string());
    Ok(Some(LibraryItem::new(
        id.to_string(),
        LibraryKind::Skill,
        name,
        source,
        dir,
    )))
}

fn get_agent_item(id: &str, source: LibrarySource) -> Result<Option<LibraryItem>> {
    let Some(dir) = item_dir(LibraryKind::Agent, id, source) else {
        return Ok(None);
    };
    if !dir.join("shim.md").exists() {
        return Ok(None);
    }
    let is_builtin = is_builtin_agent(id);
    match source {
        LibrarySource::BuiltIn if !is_builtin => return Ok(None),
        LibrarySource::Team if is_builtin => return Ok(None),
        _ => {}
    }
    let name = builtin_agents()
        .iter()
        .find(|(bid, _, _)| *bid == id)
        .map(|(_, name, _)| name.to_string())
        .unwrap_or_else(|| id.to_string());
    Ok(Some(LibraryItem::new(
        id.to_string(),
        LibraryKind::Agent,
        name,
        source,
        dir,
    )))
}

// --- helpers ----------------------------------------------------------------

/// The folder a team-owned item is created in.
fn team_item_dir(kind: LibraryKind, id: &str) -> PathBuf {
    match kind {
        LibraryKind::Skill => library_dir().join("skills").join("team").join(id),
        LibraryKind::Agent => library_dir().join("agents").join(id),
    }
}

/// The folder an item of this kind, id, and source lives in — the one place that
/// knows the layout. `None` for the sources that hold no folder of their own:
/// project-scoped items, and agent shims from the supervisor clone (the clone
/// ships skills only).
fn item_dir(kind: LibraryKind, id: &str, source: LibrarySource) -> Option<PathBuf> {
    match (kind, source) {
        (LibraryKind::Skill, LibrarySource::BuiltIn) => Some(library_dir().join("skills").join(id)),
        (LibraryKind::Skill, LibrarySource::Team) => Some(team_item_dir(kind, id)),
        (LibraryKind::Skill, LibrarySource::Supervisor) => Some(supervisor_skills_base().join(id)),
        (LibraryKind::Agent, LibrarySource::BuiltIn | LibrarySource::Team) => {
            Some(team_item_dir(kind, id))
        }
        (LibraryKind::Agent, LibrarySource::Supervisor) | (_, LibrarySource::Project) => None,
    }
}

/// Why an item of this source cannot be edited here.
fn not_editable(source: LibrarySource) -> anyhow::Error {
    match source {
        LibrarySource::Project => anyhow!("Project-scoped library items are not editable here"),
        _ => anyhow!("Supervisor source only supports skills"),
    }
}

/// Resolve a `/`-separated relative path against an item's folder, refusing
/// anything that could leave it.
///
/// Takes an already-decoded relative path: a value that came from a caller goes
/// through [`item_file_path`] first, which is where percent-decoding happens.
///
/// The lexical rules come first: an absolute path, a `.`/`..` segment, a
/// backslash, a drive colon, or a NUL is refused outright. Containment is then
/// confirmed against the real filesystem — the path is canonicalized as far as
/// it exists, and a symlink is never followed out of the folder (a dangling one
/// is refused too, since writing through it would create its target). A path
/// that does not exist yet is fine: the remaining segments can only add names
/// inside the folder.
pub fn validate_item_file_path(item_dir: &Path, rel: &str) -> Result<PathBuf> {
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.contains(['\\', ':', '\0'])
        || rel
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(anyhow!("Invalid library file path: {rel}"));
    }
    let base = crate::paths::canonicalize(item_dir)
        .map_err(|e| anyhow!("Could not resolve {}: {e}", item_dir.display()))?;
    let segments: Vec<&str> = rel.split('/').collect();
    let mut resolved = base.clone();
    let mut index = 0;
    while index < segments.len() {
        let candidate = resolved.join(segments[index]);
        match std::fs::symlink_metadata(&candidate) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = crate::paths::canonicalize(&candidate)
                    .map_err(|_| anyhow!("Path escapes the library item folder: {rel}"))?;
                if !target.starts_with(&base) {
                    return Err(anyhow!("Path escapes the library item folder: {rel}"));
                }
                resolved = target;
            }
            Ok(meta) => {
                // A file where a directory is required can never hold the rest
                // of the path.
                if !meta.is_dir() && index + 1 < segments.len() {
                    return Err(anyhow!("Invalid library file path: {rel}"));
                }
                resolved = candidate;
            }
            Err(_) => break,
        }
        index += 1;
    }
    Ok(segments[index..]
        .iter()
        .fold(resolved, |path, seg| path.join(seg)))
}

fn slugify_name(name: &str) -> Result<String> {
    let slug = crate::local::slugify(name);
    if slug.is_empty() {
        return Err(anyhow!("Name must contain letters or digits"));
    }
    Ok(slug)
}

/// An id *used as a path component* must be exactly one directory name. Callers
/// receive the id from an API path segment, which arrives percent-decoded — a
/// request for `..%2F..%2Fvictim` hands us `../../victim`, and even a single `.`
/// or `..` redirects the `join` below out of the source root. Checked against
/// `join`'s own rules rather than a slug pattern, because supervisor skill ids
/// are directory names from the cloned repository.
fn validate_item_id(id: &str) -> Result<()> {
    // A drive/UNC prefix (`C:foo`, `\\server\share`) replaces the base too.
    if id.is_empty() || id == "." || id == ".." || id.contains(['/', '\\', ':', '\0']) {
        return Err(anyhow!("Invalid library item id: {id}"));
    }
    Ok(())
}

fn validate_team_id(kind: LibraryKind, id: &str) -> Result<()> {
    if !crate::local::user_skills::is_valid_slug(id) {
        return Err(anyhow!(
            "Invalid name: must be lowercase letters, digits, and dashes"
        ));
    }
    match kind {
        LibraryKind::Skill if is_builtin_skill(id) => {
            Err(anyhow!("'{id}' is a built-in skill name"))
        }
        LibraryKind::Agent if is_builtin_agent(id) => Err(anyhow!("'{id}' is a built-in agent id")),
        _ => Ok(()),
    }
}

fn kind_label(kind: LibraryKind) -> &'static str {
    match kind {
        LibraryKind::Skill => "skill",
        LibraryKind::Agent => "agent",
    }
}

fn parse_name_from_frontmatter(content: &str) -> Option<String> {
    crate::local::user_skills::parse_frontmatter(content)
        .ok()
        .map(|f| f.name)
}

// --- helper exports for consumers that avoid a direct library dependency -----

/// Read the primary shim for an agent from the library, falling back to the
/// static string shipped with this binary.
pub fn read_agent_shim(agent_id: &str) -> String {
    let path = library_dir().join("agents").join(agent_id).join("shim.md");
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        builtin_agents()
            .into_iter()
            .find(|(id, _, _)| *id == agent_id)
            .map(|(_, _, shim)| shim)
            .unwrap_or(crate::local::harness::CLAUDE_SKILL)
            .to_string()
    })
}

/// Read the legacy Codex prompt from the library, falling back to the static
/// string shipped with this binary.
pub fn read_codex_legacy_prompt() -> String {
    let path = library_dir()
        .join("agents")
        .join("codex")
        .join("legacy-prompt.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| crate::local::harness::CODEX_PROMPT.to_string())
}

/// Return the path to the editable copy of a built-in skill package, if it has
/// been seeded. Agents and the library UI use this to read customized skill
/// content.
pub fn builtin_skill_source_dir(name: &str) -> Option<PathBuf> {
    let path = library_dir().join("skills").join(name);
    path.is_dir().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TmpDataDir(std::path::PathBuf, std::sync::MutexGuard<'static, ()>);

    /// `library_dir()` reads `$ORX_DATA_DIR` fresh on every call, so only one of
    /// these may exist at a time: a sibling's `Drop` would otherwise unset the
    /// variable mid-test and send the other test into the real data dir.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    impl TmpDataDir {
        fn new() -> Self {
            let guard = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let path = std::env::temp_dir().join(format!(
                "orx-library-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            std::env::set_var("ORX_DATA_DIR", &path);
            Self(path, guard)
        }
    }

    impl Drop for TmpDataDir {
        fn drop(&mut self) {
            std::env::remove_var("ORX_DATA_DIR");
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn ensure_team_library_seeds_builtins() {
        let _tmp = TmpDataDir::new();
        ensure_team_library().unwrap();
        assert!(library_dir().join("skills/orx-compute/SKILL.md").exists());
        assert!(library_dir().join("skills/orx-git/SKILL.md").exists());
        assert!(library_dir().join("agents/claude-code/shim.md").exists());
        assert!(library_dir().join("agents/codex/shim.md").exists());
        assert!(library_dir().join("agents/codex/legacy-prompt.md").exists());
    }

    /// Ids reach these functions percent-decoded from an API path segment, so a
    /// request for `..%2F..%2F…` arrives as a real traversal. Nothing may leave
    /// its source root — least of all `remove_dir_all`.
    #[test]
    fn item_ids_cannot_escape_the_library_root() {
        let _tmp = TmpDataDir::new();
        let escape = "../../../../victim";

        // `..` names the source root itself: `<library>/skills/team/..` is every
        // seeded skill, and `<library>/skills/.supervisor/..` all of them.
        assert!(delete_library_item(LibraryKind::Skill, "..", LibrarySource::Team).is_err());
        assert!(delete_library_item(LibraryKind::Skill, "..", LibrarySource::Supervisor).is_err());
        assert!(delete_library_item(LibraryKind::Agent, "..", LibrarySource::Team).is_err());
        assert!(library_dir().join("skills").join("orx-git").is_dir());

        assert!(get_library_item(LibraryKind::Skill, escape, LibrarySource::BuiltIn).is_err());
        assert!(
            write_library_content(LibraryKind::Skill, escape, LibrarySource::Team, "x").is_err()
        );

        // Supervisor skill ids are directory names from the cloned repo, so the
        // guard must not reject an id a real repository could contain.
        assert!(get_library_item(
            LibraryKind::Skill,
            "Supervisor_Skills.v2",
            LibrarySource::Supervisor
        )
        .is_ok());
    }

    #[test]
    fn team_skill_round_trip() {
        let _tmp = TmpDataDir::new();
        let item = create_team_library_item(
            LibraryKind::Skill,
            "my-skill",
            "---\nname: my-skill\ndescription: test\n---\nbody\n",
        )
        .unwrap();
        assert_eq!(item.name, "my-skill");
        assert_eq!(item.source, LibrarySource::Team);
        let content = read_library_content(&item).unwrap();
        assert!(content.contains("body"));
        let items = list_library_items(Some(LibraryKind::Skill), LibrarySource::Team).unwrap();
        assert!(items.iter().any(|i| i.id == "my-skill"));
    }

    /// The Supervisor-Skills repository keeps its packages under `skills/`, so a
    /// listing that only looked at the clone root came back empty and no
    /// supervisor skill could be read or edited.
    #[test]
    fn supervisor_skills_are_listed_from_the_clones_skills_dir() {
        let root = std::env::temp_dir().join(format!("orx-supervisor-{}", uuid::Uuid::new_v4()));
        let deep_research = root.join("skills/deep-research");
        std::fs::create_dir_all(&deep_research).unwrap();
        std::fs::write(
            deep_research.join("SKILL.md"),
            "---\nname: Deep Research\ndescription: d\n---\nbody\n",
        )
        .unwrap();

        let base = supervisor_skills_base_in(&root);
        assert_eq!(base, root.join("skills"));

        let items = list_supervisor_skills_in(&base).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "deep-research");
        assert_eq!(items[0].name, "Deep Research");
        assert!(std::fs::read_to_string(&items[0].file_path)
            .unwrap()
            .contains("body"));

        // A clone that already keeps skills at its root still lists them.
        let flat = root.join("flat");
        std::fs::create_dir_all(flat.join("flat-skill")).unwrap();
        std::fs::write(flat.join("flat-skill/SKILL.md"), "---\nname: Flat\n---\n").unwrap();
        assert_eq!(supervisor_skills_base_in(&flat), flat);
        assert_eq!(list_supervisor_skills_in(&flat).unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            for (name, data) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    fn team_skill(id: &str) -> LibraryItem {
        create_team_library_item(
            LibraryKind::Skill,
            id,
            &format!("---\nname: {id}\ndescription: d\n---\nbody\n"),
        )
        .unwrap()
    }

    /// A skill is a folder, so listing one has to show the files beside its
    /// `SKILL.md` — the shipped `references/*.md` a user could never see, let
    /// alone edit, while the library read exactly one path.
    #[test]
    fn a_builtin_skill_lists_its_whole_folder() {
        let _tmp = TmpDataDir::new();
        let items = list_library_items(Some(LibraryKind::Skill), LibrarySource::BuiltIn).unwrap();
        let compute = items.iter().find(|item| item.id == "orx-compute").unwrap();

        let paths: Vec<&str> = compute
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(paths.first(), Some(&"SKILL.md"), "{paths:?}");
        assert!(paths.contains(&"references/hf.md"), "{paths:?}");
        // The entry point leads; everything after it is in path order.
        let mut rest = paths[1..].to_vec();
        rest.sort_unstable();
        assert_eq!(&paths[1..], rest.as_slice(), "{paths:?}");
        assert!(compute.files.iter().all(|file| file.bytes > 0));
        assert_eq!(compute.dir_path, compute.file_path.parent().unwrap());
        assert_eq!(compute.primary_path(), "SKILL.md");

        // And the resource is readable through the folder API.
        let (path, content) = read_library_file(compute, Some("references/hf.md"))
            .unwrap()
            .expect("a seeded resource");
        assert_eq!(path, "references/hf.md");
        assert!(content.contains('#'));
    }

    /// An install seeded before resources shipped kept its `SKILL.md`, so the
    /// seeding skipped the folder and the resources never arrived. Seeding must
    /// fill in what is missing without touching what the team has edited.
    #[test]
    fn seeding_fills_in_resources_of_an_existing_install() {
        let _tmp = TmpDataDir::new();
        ensure_team_library().unwrap();
        let dir = library_dir().join("skills/orx-compute");
        let references = dir.join("references");
        std::fs::remove_dir_all(&references).unwrap();
        let skill_md = dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: orx-compute\n---\nteam edit\n").unwrap();

        ensure_team_library().unwrap();

        assert!(references.join("hf.md").exists());
        assert!(references.join("k8s.md").exists());
        assert!(std::fs::read_to_string(&skill_md)
            .unwrap()
            .contains("team edit"));
    }

    /// A cleanup that deletes more than it was asked to is the worst failure a
    /// folder API can have, so every verb resolves its path the same way.
    #[test]
    fn item_file_paths_cannot_escape_the_item_folder() {
        let _tmp = TmpDataDir::new();
        let item = team_skill("paths");
        for escape in [
            "../evil.md",
            "../../../../etc/passwd",
            "/etc/passwd",
            "references/../../evil.md",
            "..\\evil.md",
            "references/./x.md",
            "references//x.md",
            "C:/evil.md",
            "nul\0.md",
            "",
        ] {
            assert!(
                validate_item_file_path(&item.dir_path, escape).is_err(),
                "{escape} was accepted"
            );
        }
        assert!(validate_item_file_path(&item.dir_path, "references/x.md").is_ok());

        // Lexically clean but pointed out of the folder: a symlink is refused
        // rather than followed, dangling or not.
        #[cfg(unix)]
        {
            let outside = item.dir_path.parent().unwrap().join("outside.md");
            std::fs::write(&outside, "not mine").unwrap();
            std::os::unix::fs::symlink(&outside, item.dir_path.join("link.md")).unwrap();
            std::os::unix::fs::symlink(item.dir_path.parent().unwrap(), item.dir_path.join("up"))
                .unwrap();
            std::os::unix::fs::symlink("/etc/hostname", item.dir_path.join("etc-host")).unwrap();

            // A link to a file outside, to a directory outside, and to a system
            // file: all three are refused rather than followed.
            assert!(validate_item_file_path(&item.dir_path, "link.md").is_err());
            assert!(validate_item_file_path(&item.dir_path, "up/outside.md").is_err());
            assert!(validate_item_file_path(&item.dir_path, "etc-host").is_err());
            assert!(read_library_file(&item, Some("link.md")).is_err());
            assert!(write_library_file(
                LibraryKind::Skill,
                "paths",
                LibrarySource::Team,
                Some("link.md"),
                "overwritten"
            )
            .is_err());
            // The target outside the item is untouched.
            assert_eq!(std::fs::read_to_string(&outside).unwrap(), "not mine");
        }
    }

    /// Create, write, and delete of one file of a folder: parents are created,
    /// an existing file is a conflict for create and a target for write, and the
    /// entry point can never be deleted.
    #[test]
    fn folder_files_round_trip_through_the_library() {
        let _tmp = TmpDataDir::new();
        team_skill("crates");

        let created = create_library_file(
            LibraryKind::Skill,
            "crates",
            LibrarySource::Team,
            "scripts/run.sh",
            "#!/bin/sh\necho hi\n",
        )
        .unwrap();
        let crate::local::library::FileOp::Done(file) = created else {
            panic!("nested file should be created");
        };
        assert_eq!(file.path, "scripts/run.sh");
        assert!(file.absolute.starts_with(&file.dir));

        assert!(matches!(
            create_library_file(
                LibraryKind::Skill,
                "crates",
                LibrarySource::Team,
                "scripts/run.sh",
                "clobber"
            )
            .unwrap(),
            FileOp::Conflict
        ));
        assert!(matches!(
            write_library_file(
                LibraryKind::Skill,
                "crates",
                LibrarySource::Team,
                Some("scripts/run.sh"),
                "echo rewritten\n"
            )
            .unwrap(),
            FileOp::Done(_)
        ));
        // A path that does not exist is a miss, not an accidental create.
        assert!(matches!(
            write_library_file(
                LibraryKind::Skill,
                "crates",
                LibrarySource::Team,
                Some("missing.md"),
                "x"
            )
            .unwrap(),
            FileOp::Missing
        ));

        let item = get_library_item(LibraryKind::Skill, "crates", LibrarySource::Team)
            .unwrap()
            .unwrap();
        assert_eq!(
            item.files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["SKILL.md", "scripts/run.sh"]
        );
        assert_eq!(
            read_library_file(&item, Some("scripts/run.sh"))
                .unwrap()
                .unwrap()
                .1,
            "echo rewritten\n"
        );

        assert!(delete_library_file(
            LibraryKind::Skill,
            "crates",
            LibrarySource::Team,
            "SKILL.md"
        )
        .is_err());
        assert!(matches!(
            delete_library_file(
                LibraryKind::Skill,
                "crates",
                LibrarySource::Team,
                "scripts/run.sh"
            )
            .unwrap(),
            FileOp::Done(_)
        ));
        assert!(matches!(
            delete_library_file(
                LibraryKind::Skill,
                "crates",
                LibrarySource::Team,
                "scripts/run.sh"
            )
            .unwrap(),
            FileOp::Missing
        ));
    }

    /// The same path reaches the API as a percent-decoded query value and as a
    /// verbatim body field, so both forms are normalized before anything touches
    /// the disk — otherwise an encoded `..%2F..%2F` would be refused in the
    /// query and accepted as a filename in the body.
    #[test]
    fn a_file_path_is_normalized_once_whichever_form_carried_it() {
        let _tmp = TmpDataDir::new();
        let item = team_skill("decoding");

        let (rel, absolute) = item_file_path(&item.dir_path, "references/x.md").unwrap();
        assert_eq!(rel, "references/x.md");
        assert_eq!(absolute, item.dir_path.join("references/x.md"));
        // A space only ever arrives encoded; it lands as a space, not as `%20`.
        assert_eq!(
            item_file_path(&item.dir_path, "a%20b.md").unwrap().0,
            "a b.md"
        );
        // An escape that is not valid hex is a literal name, not a refusal.
        assert_eq!(
            item_file_path(&item.dir_path, "100%.md").unwrap().0,
            "100%.md"
        );

        for escape in [
            "..%2F..%2Fescaped.md",
            "%2e%2e%2f%2e%2e%2fescaped.md",
            "%2fetc%2fpasswd",
            "..%5Cescaped.md",
            "..%2fSKILL.md",
        ] {
            assert!(
                item_file_path(&item.dir_path, escape).is_err(),
                "{escape} was accepted"
            );
            assert!(
                create_library_file(
                    LibraryKind::Skill,
                    "decoding",
                    LibrarySource::Team,
                    escape,
                    "x"
                )
                .is_err(),
                "create accepted {escape}"
            );
            assert!(
                write_library_file(
                    LibraryKind::Skill,
                    "decoding",
                    LibrarySource::Team,
                    Some(escape),
                    "x"
                )
                .is_err(),
                "write accepted {escape}"
            );
            assert!(
                delete_library_file(LibraryKind::Skill, "decoding", LibrarySource::Team, escape)
                    .is_err(),
                "delete accepted {escape}"
            );
        }
        assert!(!item.dir_path.join("..%2F..%2Fescaped.md").exists());
        assert!(!item.dir_path.parent().unwrap().join("escaped.md").exists());

        // The happy path still round-trips, and an encoded spelling addresses
        // the very same file as the literal one.
        assert!(matches!(
            create_library_file(
                LibraryKind::Skill,
                "decoding",
                LibrarySource::Team,
                "references/new.md",
                "hi"
            )
            .unwrap(),
            FileOp::Done(_)
        ));
        assert!(matches!(
            write_library_file(
                LibraryKind::Skill,
                "decoding",
                LibrarySource::Team,
                Some("references/ne%77.md"),
                "rewritten"
            )
            .unwrap(),
            FileOp::Done(_)
        ));
        let item = get_library_item(LibraryKind::Skill, "decoding", LibrarySource::Team)
            .unwrap()
            .unwrap();
        assert_eq!(
            read_library_file(&item, Some("references/new.md"))
                .unwrap()
                .unwrap()
                .1,
            "rewritten"
        );
    }

    /// Creating a whole folder from an upload: the archive's files all land in
    /// the item, its frontmatter names it, and a malformed archive leaves no
    /// half-built item behind.
    #[test]
    fn a_zip_creates_a_whole_team_folder() {
        let _tmp = TmpDataDir::new();
        let zip = zip_of(&[
            (
                "pack/SKILL.md",
                b"---\nname: from-zip\ndescription: d\n---\nbody\n",
            ),
            ("pack/references/x.md", b"# x\n"),
            ("pack/scripts/run.sh", b"#!/bin/sh\n"),
            ("__MACOSX/pack/._SKILL.md", b"junk"),
        ]);
        let item = create_team_library_item_from_zip(LibraryKind::Skill, "from-zip", &zip).unwrap();
        assert_eq!(item.name, "from-zip");
        assert_eq!(
            item.files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["SKILL.md", "references/x.md", "scripts/run.sh"]
        );
        assert_eq!(
            read_library_file(&item, Some("references/x.md"))
                .unwrap()
                .unwrap()
                .1,
            "# x\n"
        );

        // The same archive again: the name is taken.
        assert!(create_team_library_item_from_zip(LibraryKind::Skill, "from-zip", &zip).is_err());
        // No entry point, and a broken archive: refused, with nothing left on
        // disk to mistake for an item.
        let no_md = zip_of(&[("pack/readme.md", b"hi")]);
        assert!(create_team_library_item_from_zip(LibraryKind::Skill, "no-md", &no_md).is_err());
        assert!(
            create_team_library_item_from_zip(LibraryKind::Skill, "broken", b"not a zip").is_err()
        );
        assert!(!library_dir().join("skills/team/no-md").exists());
        assert!(!library_dir().join("skills/team/broken").exists());
    }
}
