//! Team papers: PDFs uploaded by project members to shape the research agent's
//! context — topic discovery, innovation mining, writing style, and scheme
//! migration. Files live under `<project_dir>/.openresearch/team-papers/<id>/`
//! with both the original `paper.pdf` and a best-effort `paper.txt` extracted
//! by the external `pdftotext` tool.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use base64::Engine;
use serde::Deserialize;

use crate::error::{anyhow, Result};
use crate::local::model::{LocalProject, TeamPaper};
use crate::paths::canonicalize;
use crate::store::{now_ms, Store};

const TEAM_PAPERS_DIR: &str = ".openresearch/team-papers";

/// A paper id names exactly one directory under the team-papers root. Ids are
/// minted as UUIDs, but they also arrive from an API path segment (percent-
/// decoded), so a `..` or a separator must never reach the `join` below.
fn validate_paper_id(paper_id: &str) -> Result<()> {
    if paper_id.is_empty()
        || !paper_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!("Invalid paper id: {paper_id}"));
    }
    Ok(())
}

/// Project-local storage root for team papers.
pub fn team_papers_dir(project: &LocalProject) -> PathBuf {
    Path::new(&project.project_dir).join(TEAM_PAPERS_DIR)
}

/// Path to the original PDF for a paper.
pub fn paper_pdf_path(project: &LocalProject, paper_id: &str) -> PathBuf {
    team_papers_dir(project).join(paper_id).join("paper.pdf")
}

/// Path to the extracted plain text for a paper.
pub fn paper_text_path(project: &LocalProject, paper_id: &str) -> PathBuf {
    team_papers_dir(project).join(paper_id).join("paper.txt")
}

/// Request shape for creating a team paper. Matches the base64 upload convention
/// used for user skills and chat attachments.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTeamPaperReq {
    pub filename: String,
    pub content_base64: String,
    pub title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub source_url: Option<String>,
}

/// Decode the upload, write the PDF, best-effort extract text, and persist the
/// row. Extraction failure does not fail the upload.
pub fn save_team_paper(
    store: &Store,
    project: &LocalProject,
    req: CreateTeamPaperReq,
) -> Result<TeamPaper> {
    let filename = req.filename.trim().to_string();
    if filename.is_empty() {
        return Err(anyhow!("filename is required"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let base_dir = team_papers_dir(project);
    let dir = base_dir.join(&id);

    std::fs::create_dir_all(&dir)
        .map_err(|e| anyhow!("Could not create {}: {}", dir.display(), e))?;

    // Containment: the generated path must stay inside the team-papers tree.
    let canonical_base = canonicalize(&base_dir)
        .map_err(|e| anyhow!("Could not canonicalize team papers dir: {e}"))?;
    let canonical_dir =
        canonicalize(&dir).map_err(|e| anyhow!("Could not canonicalize paper dir: {e}"))?;
    if !canonical_dir.starts_with(&canonical_base) {
        return Err(anyhow!("paper directory escapes team papers directory"));
    }

    let pdf_path = canonical_dir.join("paper.pdf");
    let text_path = canonical_dir.join("paper.txt");

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(req.content_base64.trim())
        .map_err(|e| anyhow!("invalid file data: {e}"))?;

    std::fs::write(&pdf_path, &bytes)
        .map_err(|e| anyhow!("Could not write {}: {}", pdf_path.display(), e))?;

    let (extracted_at, page_count) = match extract_text_with_pdftotext(&pdf_path, &text_path) {
        Ok(pages) => (Some(now_ms()), pages),
        Err(_) => {
            // Best-effort extraction: leave extracted_at null and continue.
            let _ = std::fs::remove_file(&text_path);
            (None, None)
        }
    };

    let paper = TeamPaper {
        id,
        project_id: project.id.clone(),
        filename,
        title: req.title.filter(|t| !t.trim().is_empty()),
        authors: req.authors.unwrap_or_default(),
        tags: req.tags.unwrap_or_default(),
        notes: req.notes.filter(|n| !n.trim().is_empty()),
        source_url: req.source_url.filter(|u| !u.trim().is_empty()),
        page_count,
        extracted_at,
        created_at: now_ms(),
        updated_at: now_ms(),
    };
    store.create_team_paper(&paper)?;
    Ok(paper)
}

/// Run `pdftotext` if it is available. Returns the best-effort page count from
/// the number of form-feed characters in the output.
fn extract_text_with_pdftotext(pdf_path: &Path, text_path: &Path) -> Result<Option<i64>> {
    let output = std::process::Command::new("pdftotext")
        .arg(pdf_path)
        .arg(text_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| anyhow!("pdftotext failed: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!("pdftotext exited with {}", output.status));
    }
    let text = std::fs::read_to_string(text_path)
        .map_err(|e| anyhow!("Could not read extracted text: {e}"))?;
    let pages = if text.is_empty() {
        0
    } else {
        text.matches('\x0C').count() as i64 + 1
    };
    Ok(Some(pages))
}

/// Delete the paper files and the store row. Files are removed first so a
/// partial failure leaves the row (rather than orphan files).
pub fn delete_team_paper(store: &Store, project: &LocalProject, paper_id: &str) -> Result<()> {
    validate_paper_id(paper_id)?;
    let dir = paper_pdf_path(project, paper_id)
        .parent()
        .expect("paper path has a parent directory")
        .to_path_buf();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| anyhow!("Could not remove {}: {}", dir.display(), e))?;
    }
    store.delete_team_paper(paper_id)?;
    Ok(())
}

/// Remove every stored paper file for a project. Used when the project itself
/// is deleted: the store rows go with it, so the uploaded PDFs and extracted
/// texts under `<project_dir>/.openresearch/team-papers/` would otherwise stay
/// behind as unreferenced orphans. Only that orx-owned subtree is touched.
pub fn remove_team_papers_dir(project: &LocalProject) -> Result<()> {
    let dir = team_papers_dir(project);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| anyhow!("Could not remove {}: {}", dir.display(), e))?;
    }
    Ok(())
}

/// Read the extracted plain text for a paper.
pub fn read_text(project: &LocalProject, paper_id: &str) -> Result<String> {
    validate_paper_id(paper_id)?;
    let path = paper_text_path(project, paper_id);
    std::fs::read_to_string(&path).map_err(|e| anyhow!("Could not read {}: {}", path.display(), e))
}

/// Copy the team-papers tree into a session worktree when the worktree is not
/// already inside the project directory (and therefore cannot reach the files
/// directly). The destination is already git-ignored via `.openresearch/`.
/// Returns `true` when files were copied.
pub fn stage_into_worktree(project: &LocalProject, worktree: &Path) -> Result<bool> {
    let source = team_papers_dir(project);
    if !source.exists() {
        return Ok(false);
    }
    let canonical_project = canonicalize(Path::new(&project.project_dir))
        .map_err(|e| anyhow!("Could not canonicalize project dir: {e}"))?;
    let canonical_worktree =
        canonicalize(worktree).map_err(|e| anyhow!("Could not canonicalize worktree: {e}"))?;
    if canonical_worktree.starts_with(&canonical_project) {
        return Ok(false);
    }
    let dest = worktree.join(TEAM_PAPERS_DIR);
    crate::local::user_skills::copy_dir_all(&source, &dest)
        .map_err(|e| anyhow!("Could not stage team papers into worktree: {e}"))?;
    Ok(true)
}

/// Markdown line for the playbook listing team papers with a path relative to
/// the session worktree. Returns an empty string when there are no papers.
pub fn playbook_line(project: &LocalProject, worktree: &Path, store: &Store) -> Result<String> {
    let papers = store.list_team_papers(&project.id)?;
    if papers.is_empty() {
        return Ok(String::new());
    }
    let canonical_project = canonicalize(Path::new(&project.project_dir))
        .map_err(|e| anyhow!("Could not canonicalize project dir: {e}"))?;
    let canonical_worktree =
        canonicalize(worktree).map_err(|e| anyhow!("Could not canonicalize worktree: {e}"))?;
    let inside = canonical_worktree.starts_with(&canonical_project);
    let lines: Vec<String> = papers
        .iter()
        .map(|p| {
            let rel = if inside {
                format!("../../{TEAM_PAPERS_DIR}/{}/paper.txt", p.id)
            } else {
                format!("{TEAM_PAPERS_DIR}/{}/paper.txt", p.id)
            };
            format!("  - {} (`{rel}`)", p.display_title())
        })
        .collect();
    Ok(format!(
        "- Team papers ({}):\n{}\n",
        papers.len(),
        lines.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn project_fixture(dir: &Path) -> LocalProject {
        LocalProject {
            id: "proj-1".to_string(),
            name: "Demo".to_string(),
            slug: "demo".to_string(),
            github_owner: String::new(),
            github_repo: String::new(),
            github_sync_enabled: false,
            baseline_branch: "main".to_string(),
            repo_path: dir.join("repo").to_string_lossy().into_owned(),
            project_dir: dir.to_string_lossy().into_owned(),
            run_command: None,
            paper_id: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn pdf_bytes() -> Vec<u8> {
        // A minimal syntactically valid PDF with one empty page.
        b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]>>endobj\nxref\n0 4\n0000000000 65535 f\n0000000009 00000 n\n0000000052 00000 n\n0000000101 00000 n\ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n147\n%%EOF\n".to_vec()
    }

    #[test]
    fn save_team_paper_writes_pdf_and_row() {
        let dir = std::env::temp_dir().join(format!("orx-team-papers-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let project = project_fixture(&dir);
        let store_dir = dir.join("store");
        let store = Store::open_at(store_dir).unwrap();
        store.create_local_project(&project).unwrap();

        let encoded = base64::engine::general_purpose::STANDARD.encode(pdf_bytes());
        let paper = save_team_paper(
            &store,
            &project,
            CreateTeamPaperReq {
                filename: "paper.pdf".to_string(),
                content_base64: encoded,
                title: Some("Test Paper".to_string()),
                authors: Some(vec!["A. Author".to_string()]),
                tags: Some(vec!["test".to_string()]),
                notes: Some("note".to_string()),
                source_url: Some("https://example.com".to_string()),
            },
        )
        .unwrap();

        assert_eq!(paper.project_id, project.id);
        assert_eq!(paper.filename, "paper.pdf");
        assert_eq!(paper.title.as_deref(), Some("Test Paper"));
        assert_eq!(paper.authors, vec!["A. Author"]);
        assert!(paper_pdf_path(&project, &paper.id).exists());

        let listed = store.list_team_papers(&project.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, paper.id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ids reach these functions from an API path segment (percent-decoded), so
    /// a `..` must be refused before it becomes a directory to remove or read.
    #[test]
    fn paper_ids_cannot_escape_the_team_papers_root() {
        let dir = std::env::temp_dir().join(format!("orx-team-papers-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let project = project_fixture(&dir);
        let store = Store::open_at(dir.join("store")).unwrap();
        store.create_local_project(&project).unwrap();

        for id in ["..", "", "../../etc", "a/b"] {
            assert!(read_text(&project, id).is_err(), "{id}");
            assert!(delete_team_paper(&store, &project, id).is_err(), "{id}");
        }
        assert!(dir.is_dir());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_team_paper_removes_files_and_row() {
        let dir = std::env::temp_dir().join(format!("orx-team-papers-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let project = project_fixture(&dir);
        let store_dir = dir.join("store");
        let store = Store::open_at(store_dir).unwrap();
        store.create_local_project(&project).unwrap();

        let encoded = base64::engine::general_purpose::STANDARD.encode(pdf_bytes());
        let paper = save_team_paper(
            &store,
            &project,
            CreateTeamPaperReq {
                filename: "paper.pdf".to_string(),
                content_base64: encoded,
                title: None,
                authors: None,
                tags: None,
                notes: None,
                source_url: None,
            },
        )
        .unwrap();

        delete_team_paper(&store, &project, &paper.id).unwrap();
        assert!(store.get_team_paper(&paper.id).unwrap().is_none());
        assert!(!paper_pdf_path(&project, &paper.id).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Deleting the project drops the rows, so the uploaded files have to go with
    /// them or they stay behind as unreferenced orphans.
    #[test]
    fn remove_team_papers_dir_clears_every_paper_tree() {
        let dir = std::env::temp_dir().join(format!("orx-team-papers-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let project = project_fixture(&dir);
        let store = Store::open_at(dir.join("store")).unwrap();
        store.create_local_project(&project).unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(pdf_bytes());
        for name in ["one.pdf", "two.pdf"] {
            save_team_paper(
                &store,
                &project,
                CreateTeamPaperReq {
                    filename: name.to_string(),
                    content_base64: encoded.clone(),
                    title: None,
                    authors: None,
                    tags: None,
                    notes: None,
                    source_url: None,
                },
            )
            .unwrap();
        }
        assert_eq!(store.list_team_papers(&project.id).unwrap().len(), 2);
        assert!(team_papers_dir(&project).is_dir());

        remove_team_papers_dir(&project).unwrap();
        assert!(!team_papers_dir(&project).exists());
        // The rest of the project directory is untouched.
        assert!(dir.is_dir());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
