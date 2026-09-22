//! Unified skill + agent catalog for the chat composer picker.
//!
//! Scans every source the dashboard can draw from: built-in `orx-*` skills,
//! editable team-library copies, user-uploaded skills, skills mirrored from
//! installed coding agents, and every registered harness (chat-capable or not).

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;
use crate::local::{agent_skills, harness, library, user_skills};

/// One skill that can be inserted into the composer.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentSkill {
    pub id: String,
    pub name: String,
    pub source: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Full body content, ready to insert. Kept in the list response so the
    /// picker can insert a skill in one round trip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// One harness/agent that can be attached to a turn.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentAgent {
    pub id: String,
    pub name: String,
    pub harness_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub installed: bool,
    pub authenticated: bool,
}

/// The complete picker payload.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachments {
    pub skills: Vec<ChatAttachmentSkill>,
    pub agents: Vec<ChatAttachmentAgent>,
}

/// Build the unified catalog. Detection of installed agents is async; the rest
/// is synchronous local filesystem scanning.
pub async fn list_attachments() -> Result<ChatAttachments> {
    let detected = harness::detect_harnesses().await;
    let detected_by_id: HashMap<&'static str, harness::HarnessInfo> =
        detected.into_iter().map(|info| (info.id, info)).collect();

    let mut agents = Vec::new();
    for h in harness::registry() {
        let id = h.id();
        let info = detected_by_id.get(id);
        agents.push(ChatAttachmentAgent {
            id: id.to_string(),
            name: info
                .map(|i| i.name.to_string())
                .unwrap_or_else(|| h.name().to_string()),
            harness_id: id.to_string(),
            description: None,
            installed: info.map(|i| i.installed).unwrap_or(false),
            authenticated: info.map(|i| i.authenticated).unwrap_or(false),
        });
    }

    let skills = tokio::task::spawn_blocking(build_skills).await??;

    Ok(ChatAttachments { skills, agents })
}

fn build_skills() -> Result<Vec<ChatAttachmentSkill>> {
    library::ensure_team_library()?;

    let mut skills = Vec::new();

    // Built-in skills: editable copies in the team library, descriptions from
    // the embedded catalog.
    let builtin_descs: HashMap<&str, &str> = agent_skills::skills(agent_skills::SkillSet::Full)
        .into_iter()
        .map(|s| (s.name, s.description))
        .collect();
    for item in library::list_library_items(Some(library::LibraryKind::Skill), library::LibrarySource::BuiltIn)? {
        let content = read_skill_body(&item.file_path);
        skills.push(ChatAttachmentSkill {
            id: item.id.clone(),
            name: item.name.clone(),
            source: "builtin".to_string(),
            file_path: item.file_path.to_string_lossy().into_owned(),
            description: builtin_descs
                .get(item.id.as_str())
                .map(|d| d.to_string()),
            content_preview: content.as_deref().map(preview_of),
            scope: Some("global".to_string()),
            content,
        });
    }

    // Team-owned library skills.
    for item in library::list_library_items(Some(library::LibraryKind::Skill), library::LibrarySource::Team)? {
        let content = read_skill_body(&item.file_path);
        let description = frontmatter_description(&item.file_path);
        skills.push(ChatAttachmentSkill {
            id: item.id.clone(),
            name: item.name.clone(),
            source: "team".to_string(),
            file_path: item.file_path.to_string_lossy().into_owned(),
            description,
            content_preview: content.as_deref().map(preview_of),
            scope: Some("global".to_string()),
            content,
        });
    }

    // Supervisor skills cloned from the Supervisor-Skills repository.
    for item in library::list_library_items(Some(library::LibraryKind::Skill), library::LibrarySource::Supervisor)? {
        let content = read_skill_body(&item.file_path);
        let description = frontmatter_description(&item.file_path);
        skills.push(ChatAttachmentSkill {
            id: item.id.clone(),
            name: item.name.clone(),
            source: "supervisor".to_string(),
            file_path: item.file_path.to_string_lossy().into_owned(),
            description,
            content_preview: content.as_deref().map(preview_of),
            scope: Some("global".to_string()),
            content,
        });
    }

    // User-uploaded skills.
    for (skill, path) in user_skills::list_uploaded_with_paths() {
        let content = read_skill_body(&path.join("SKILL.md"));
        skills.push(ChatAttachmentSkill {
            id: skill.name.clone(),
            name: skill.name.clone(),
            source: "user".to_string(),
            file_path: path.to_string_lossy().into_owned(),
            description: Some(skill.description).filter(|d| !d.is_empty()),
            content_preview: content.as_deref().map(preview_of),
            scope: Some("global".to_string()),
            content,
        });
    }

    // Skills mirrored from installed coding agents and their plugins.
    for (skill, path) in user_skills::list_mirrored_with_paths() {
        let content = read_skill_body(&path.join("SKILL.md"));
        skills.push(ChatAttachmentSkill {
            id: skill.name.clone(),
            name: skill.name.clone(),
            source: "mirrored".to_string(),
            file_path: path.to_string_lossy().into_owned(),
            description: Some(skill.description).filter(|d| !d.is_empty()),
            content_preview: content.as_deref().map(preview_of),
            scope: Some("global".to_string()),
            content,
        });
    }

    skills.sort_by_key(|s| s.name.to_lowercase());
    Ok(skills)
}

/// Read a `SKILL.md` and return its body with YAML frontmatter stripped.
/// If the file has no frontmatter, the full file (minus a BOM) is returned.
fn read_skill_body(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
    let Some(after_open) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return Some(content.to_string());
    };
    let Some(end) = after_open.find("\n---") else {
        return Some(content.to_string());
    };
    Some(
        after_open[end + 4..]
            .trim_start_matches(['\r', '\n'])
            .to_string(),
    )
}

fn frontmatter_description(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    user_skills::parse_frontmatter(&content)
        .ok()
        .map(|f| f.description)
        .filter(|d| !d.is_empty())
}

fn preview_of(body: &str) -> String {
    let trimmed = body.trim();
    let limit = 300;
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        let mut out = String::with_capacity(limit + 1);
        for (i, ch) in trimmed.chars().enumerate() {
            if i >= limit {
                break;
            }
            out.push(ch);
        }
        out.push('…');
        out
    }
}
