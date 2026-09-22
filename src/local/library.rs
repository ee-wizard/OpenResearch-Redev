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

/// One item in the team library.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub kind: LibraryKind,
    pub name: String,
    pub source: LibrarySource,
    pub file_path: PathBuf,
    pub editable: bool,
}

impl LibraryItem {
    fn new(
        id: String,
        kind: LibraryKind,
        name: String,
        source: LibrarySource,
        file_path: PathBuf,
    ) -> Self {
        Self {
            id,
            kind,
            name,
            source,
            file_path,
            editable: source != LibrarySource::Project,
        }
    }
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
        if path.exists() {
            continue;
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, skill.content)?;
        for resource in skill.resources {
            let resource_path = dir.join(resource.path);
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

/// Read the content of a library item.
pub fn read_library_content(item: &LibraryItem) -> Result<String> {
    std::fs::read_to_string(&item.file_path)
        .map_err(|e| anyhow!("Could not read {}: {}", item.file_path.display(), e))
}

/// Write content to a library item, creating parent directories as needed.
/// Built-in edits update the editable copy in the data dir.
pub fn write_library_content(
    kind: LibraryKind,
    id: &str,
    source: LibrarySource,
    content: &str,
) -> Result<PathBuf> {
    ensure_team_library()?;
    validate_item_id(id)?;
    let path = match (kind, source) {
        (LibraryKind::Skill, LibrarySource::BuiltIn) => {
            library_dir().join("skills").join(id).join("SKILL.md")
        }
        (LibraryKind::Skill, LibrarySource::Team) => {
            library_dir().join("skills").join("team").join(id).join("SKILL.md")
        }
        (LibraryKind::Skill, LibrarySource::Supervisor) => {
            supervisor_skills_base().join(id).join("SKILL.md")
        }
        (LibraryKind::Agent, LibrarySource::BuiltIn) => {
            library_dir().join("agents").join(id).join("shim.md")
        }
        (LibraryKind::Agent, LibrarySource::Team) => {
            library_dir().join("agents").join(id).join("shim.md")
        }
        (_, LibrarySource::Project) => {
            return Err(anyhow!("Project-scoped library items are not editable here"))
        }
        (LibraryKind::Agent, LibrarySource::Supervisor) => {
            return Err(anyhow!("Supervisor source only supports skills"))
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(path)
}

/// Create a new team-owned library item.
pub fn create_team_library_item(
    kind: LibraryKind,
    name: &str,
    content: &str,
) -> Result<LibraryItem> {
    ensure_team_library()?;
    let id = slugify_name(name)?;
    validate_team_id(kind, &id)?;
    let (dir, file_name): (PathBuf, &str) = match kind {
        LibraryKind::Skill => (
            library_dir().join("skills").join("team").join(&id),
            "SKILL.md",
        ),
        LibraryKind::Agent => (library_dir().join("agents").join(&id), "shim.md"),
    };
    if dir.exists() {
        return Err(anyhow!(
            "A team {kind} named '{name}' already exists",
            kind = kind_label(kind)
        ));
    }
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(file_name);
    std::fs::write(&path, content)?;
    let display_name = parse_name_from_frontmatter(content).unwrap_or_else(|| name.to_string());
    Ok(LibraryItem::new(
        id,
        kind,
        display_name,
        LibrarySource::Team,
        path,
    ))
}

/// Delete a library item. Returns `true` if it existed.
/// Built-in items cannot be deleted; supervisor skills can be deleted, which
/// removes the cloned directory.
pub fn delete_library_item(kind: LibraryKind, id: &str, source: LibrarySource) -> Result<bool> {
    ensure_team_library()?;
    validate_item_id(id)?;
    let dir = match (kind, source) {
        (LibraryKind::Skill, LibrarySource::Team) => {
            library_dir().join("skills").join("team").join(id)
        }
        (LibraryKind::Skill, LibrarySource::Supervisor) => supervisor_skills_base().join(id),
        (LibraryKind::Agent, LibrarySource::Team) => library_dir().join("agents").join(id),
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
            file_path,
        ));
    }
    Ok(items)
}

fn list_builtin_skills() -> Result<Vec<LibraryItem>> {
    let base = library_dir().join("skills");
    let mut items = Vec::new();
    for skill in skills(SkillSet::Full) {
        let path = base.join(skill.name).join("SKILL.md");
        if path.exists() {
            items.push(LibraryItem::new(
                skill.name.to_string(),
                LibraryKind::Skill,
                skill.name.to_string(),
                LibrarySource::BuiltIn,
                path,
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
        let file_path = path.join("shim.md");
        let name = builtin_agents()
            .iter()
            .find(|(bid, _, _)| *bid == id.as_str())
            .map(|(_, name, _)| name.to_string())
            .unwrap_or_else(|| id.clone());
        items.push(LibraryItem::new(
            id,
            LibraryKind::Agent,
            name,
            source,
            file_path,
        ));
    }
    Ok(items)
}

fn list_team_dirs(dir: &Path, kind: LibraryKind, source: LibrarySource) -> Result<Vec<LibraryItem>> {
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
        let file_path = path.join(if kind == LibraryKind::Skill {
            "SKILL.md"
        } else {
            "shim.md"
        });
        let name = if file_path.exists() {
            std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|content| parse_name_from_frontmatter(&content))
                .unwrap_or_else(|| id.clone())
        } else {
            id.clone()
        };
        items.push(LibraryItem::new(id, kind, name, source, file_path));
    }
    Ok(items)
}

fn get_skill_item(id: &str, source: LibrarySource) -> Result<Option<LibraryItem>> {
    let path = match source {
        LibrarySource::BuiltIn => library_dir().join("skills").join(id).join("SKILL.md"),
        LibrarySource::Team => library_dir()
            .join("skills")
            .join("team")
            .join(id)
            .join("SKILL.md"),
        LibrarySource::Project => return Ok(None),
        LibrarySource::Supervisor => supervisor_skills_base().join(id).join("SKILL.md"),
    };
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
        path,
    )))
}

fn get_agent_item(id: &str, source: LibrarySource) -> Result<Option<LibraryItem>> {
    let path = library_dir().join("agents").join(id).join("shim.md");
    if !path.exists() {
        return Ok(None);
    }
    let is_builtin = is_builtin_agent(id);
    match source {
        LibrarySource::BuiltIn if !is_builtin => return Ok(None),
        LibrarySource::Team if is_builtin => return Ok(None),
        LibrarySource::Project => return Ok(None),
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
        path,
    )))
}

// --- helpers ----------------------------------------------------------------

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
        LibraryKind::Agent if is_builtin_agent(id) => {
            Err(anyhow!("'{id}' is a built-in agent id"))
        }
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
    std::fs::read_to_string(&path).unwrap_or_else(|_| crate::local::harness::CODEX_PROMPT.to_string())
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
            let guard = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
        assert!(get_library_item(LibraryKind::Skill, "Supervisor_Skills.v2", LibrarySource::Supervisor)
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
}
