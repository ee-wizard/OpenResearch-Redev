//! Oh My Pi harness (`omp`).
//!
//! Chat: one `omp --print --mode json` child per turn. Multi-turn continues via
//! `--resume <session_id-prefix>` from the `session` event's `id`. Isolated ORX
//! worktrees are the child's `--cwd`. omp writes its sessions under
//! `~/.omp/agent/sessions` (or `PI_CODING_AGENT_DIR`) — orx leaves them at the
//! default so the same conversation can be reopened with `omp --resume` in a
//! terminal.
//!
//! The playbook is pointed at on the first turn (the file is already in the
//! worktree via [`ensure_playbook`]); session skills land in `.omp/skills`,
//! which omp auto-discovers from the working directory.
//!
//! Permission mapping: omp's `--approval-mode` — Ask is a read-only print run
//! (`--no-tools`), Auto auto-approves every tool call, Full access is the same
//! auto-approval (omp sandboxes nothing extra). Plan mode runs with
//! `--no-tools` so the agent can only describe the plan; the synthesized plan
//! card's approve resumes with tools enabled.
//!
//! Detection: `omp` on PATH; auth = any configured provider — `omp models`
//! answering with a catalog (10s probe, like cursor's `agent models`), a
//! provider key in `~/.omp/agent/models.yml`, or one of the well-known provider
//! API-key env vars.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use super::detect::{resolve_symlinks, title_case, HarnessAuthState, HarnessInfo, ModelInfo};
use super::options::{HarnessOptions, OptionChoice, PermissionMode, PlanActivation};
use super::{
    Harness, OneShot, OneShotQuality, ResumeAction, TurnFailure, TurnOutcome, TurnResult,
    TURN_WATCHDOG,
};
use crate::error::{anyhow, Result};
use crate::local::chat::{
    find_part_mut, harness_log, prepare_env, set_chat_session_env, DeliveryState, PromptAnswer,
    ResumeCtx, TurnCtx, WirePart, WirePrompt, WireToolState,
};
use crate::local::opencode::{ensure_playbook, PLAYBOOK_REL};
use crate::local::shell_env::find_on_path;

const OMP_REINSTALL: &str = "Reinstall it from ohmy.pi (or your package manager)";
/// `omp models` cold-starts at ~12s (provider catalog fetch), so cursor's 10s
/// budget would intermittently read a working install as signed-out.
const MODELS_TIMEOUT: Duration = Duration::from_secs(20);

/// Env vars omp reads provider credentials from; any one set counts as a
/// configured provider. (omp has no OAuth login of its own — auth is keys.)
const PROVIDER_KEY_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "OPENROUTER_API_KEY",
    "DEEPSEEK_API_KEY",
    "MISTRAL_API_KEY",
    "GROQ_API_KEY",
    "XAI_API_KEY",
    "TOGETHER_API_KEY",
    "FIREWORKS_API_KEY",
    "CEREBRAS_API_KEY",
    "KIMI_API_KEY",
    "MOONSHOT_API_KEY",
    "ZAI_API_KEY",
    "MINIMAX_API_KEY",
    "OPENCODE_API_KEY",
    "CURSOR_ACCESS_TOKEN",
    "AI_GATEWAY_API_KEY",
];

pub struct OhMyPi;

#[async_trait]
impl Harness for OhMyPi {
    fn id(&self) -> &'static str {
        "oh-my-pi"
    }

    fn name(&self) -> &'static str {
        "Oh My Pi"
    }

    fn supports_chat(&self) -> bool {
        true
    }

    async fn detect(&self) -> Option<HarnessInfo> {
        let mut info = HarnessInfo::new(self.id(), self.name());
        if let Some((bin, probe)) = find_omp_working().await {
            info.record_bin(&bin, probe);
        }
        if info.installed && !info.install_broken {
            let bin = info.bin_path.as_deref().map(std::path::Path::new);
            let providers = configured_providers();
            let models = match bin {
                Some(bin) => omp_model_list(bin, &providers).await,
                None => None,
            };
            // The catalog probe doubles as the auth probe: omp prints a
            // provider's models only when it can reach it with credentials.
            // A bare `omp models` exit with an empty catalog means no provider
            // is configured.
            if models.as_ref().is_some_and(|models| !models.is_empty()) || has_provider_key() {
                info.authenticated = true;
                info.auth_state = HarnessAuthState::Ready;
                info.auth_method = Some("apiKey");
            } else if info.installed {
                info.auth_state = HarnessAuthState::NeedsLogin;
            }
            info.agent_ready = info.ready();
            if info.agent_ready {
                info = info.with_models(
                    models
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(fallback_models),
                );
            } else {
                info.agent_note = Some(
                    "Configure a provider in ~/.omp/agent/models.yml (e.g. add a kimi section with an apiKey), then re-check this harness."
                        .to_string(),
                );
            }
        } else if info.install_broken {
            info.agent_ready = false;
            info.agent_note = Some(info.broken_note(OMP_REINSTALL));
        } else {
            info.agent_ready = false;
            info.agent_note = Some(
                "Install Oh My Pi (`omp`) from ohmy.pi, then configure a provider API key."
                    .to_string(),
            );
        }
        Some(info)
    }

    async fn run_turn(&self, ctx: &mut TurnCtx) -> TurnResult {
        run_turn(ctx)
            .await
            .map(|()| TurnOutcome::Completed)
            .map_err(|error| TurnFailure::adapter(error, ctx.delivery_state()))
    }

    async fn one_shot(&self, request: OneShot<'_>) -> Option<String> {
        omp_one_shot(&find_omp()?, request).await
    }

    fn options(&self) -> HarnessOptions {
        HarnessOptions::none().with_permission_choices(
            vec![
                OptionChoice::described("ask", "Ask", "Answer questions without changing files"),
                OptionChoice::described("auto", "Auto", "Auto-approve all tool calls"),
                OptionChoice::described(
                    "full-access",
                    "Full access",
                    "Auto-approve all tool calls (omp runs unsandboxed either way)",
                ),
            ],
            "auto",
            PlanActivation::Command,
        )
    }

    async fn resume_from_prompt(
        &self,
        _ctx: &ResumeCtx,
        prompt: &WirePrompt,
        answer: &PromptAnswer,
    ) -> Result<ResumeAction> {
        // End-turn plan cards only — print mode has no live protocol to reply on.
        if prompt.kind != "plan" {
            return Ok(ResumeAction::Nothing);
        }
        if !answer.approve && answer.note.as_deref().is_none_or(|s| s.trim().is_empty()) {
            return Ok(ResumeAction::Nothing);
        }
        let note = answer.note.as_deref().filter(|s| !s.trim().is_empty());
        let (text, plan_mode) = if answer.approve {
            let mut text = "Implement the plan.".to_string();
            if let Some(note) = note {
                text.push_str(&format!("\n\nAdditional guidance: {note}"));
            }
            (text, false)
        } else {
            (super::synthesize_resume("plan", answer).0, true)
        };
        Ok(ResumeAction::SendMessage {
            text,
            mode: None,
            plan_mode: Some(plan_mode),
        })
    }

    fn config_home(&self) -> Option<PathBuf> {
        // PI_CODING_AGENT_DIR overrides the default `~/.omp/agent`.
        std::env::var("PI_CODING_AGENT_DIR")
            .ok()
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .or_else(|| dirs::home_dir().map(|home| home.join(".omp").join("agent")))
    }

    fn skill_target(&self) -> Option<PathBuf> {
        Some(
            self.config_home()?
                .join("skills")
                .join("orx")
                .join("SKILL.md"),
        )
    }

    fn skill_shim(&self) -> Option<&'static str> {
        Some(super::CLAUDE_SKILL)
    }

    fn session_skills_dir(&self) -> Option<&'static str> {
        // omp discovers native SKILL.md dirs from `.omp/skills` in the cwd.
        Some(".omp/skills")
    }
}

/// `omp` on PATH, then the common user-level drop. The name is distinctive
/// enough to skip cursor's is-this-really-the-right-`agent` dance.
fn omp_candidates() -> Vec<PathBuf> {
    let drops = dirs::home_dir()
        .map(|home| home.join(".local").join("bin"))
        .into_iter()
        .filter_map(|dir| crate::local::shell_env::find_in_dir(&dir, "omp"));
    find_on_path("omp")
        .into_iter()
        .chain(drops)
        .map(resolve_symlinks)
        .collect()
}

/// The executable detection selected, else the first candidate — sync callers
/// cannot probe.
pub(crate) fn find_omp() -> Option<PathBuf> {
    super::detect::selected_bin("oh-my-pi", omp_candidates())
}

pub(super) async fn find_omp_working() -> Option<(PathBuf, super::detect::BinProbe)> {
    super::detect::select_working("oh-my-pi", omp_candidates(), None).await
}

/// A provider key in the environment (orx's synced env file included) or any
/// `apiKey` line in omp's models.yml — omp authenticates per provider, not per
/// account.
fn has_provider_key() -> bool {
    if PROVIDER_KEY_VARS
        .iter()
        .any(|key| super::detect::api_key(key).is_some())
    {
        return true;
    }
    let Some(home) = OhMyPi.config_home() else {
        return false;
    };
    let Ok(raw) = std::fs::read_to_string(home.join("models.yml")) else {
        return false;
    };
    raw.lines().any(|line| {
        let line = line.trim();
        line.starts_with("apiKey:") && line.len() > "apiKey:".len() + 1
    })
}

/// Parse `~/.omp/agent/models.yml` for the configured provider names.
/// `omp models` lists every built-in provider even when unauthenticated; we
/// only want the ones the user actually configured.
fn configured_providers() -> Vec<String> {
    let Some(home) = OhMyPi.config_home() else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read_to_string(home.join("models.yml")) else {
        return Vec::new();
    };
    let mut providers = Vec::new();
    let mut in_providers = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "providers:" {
            in_providers = true;
            continue;
        }
        if !in_providers {
            continue;
        }
        // Provider entries are indented exactly two spaces under `providers:`.
        if line.starts_with("  ") && !line.starts_with("    ") && trimmed.ends_with(':') {
            let name = trimmed.trim_end_matches(':').trim();
            if !name.is_empty() {
                providers.push(name.to_string());
            }
            continue;
        }
        // Deeper indentation or a new top-level key ends the providers section.
        if !line.starts_with("  ") {
            break;
        }
    }
    providers
}

/// `omp models` prints an ASCII table per provider group. Model ids are the
/// first column of rows inside the boxed table; provider headers (`kimi (4)`)
/// prefix the id on `--model` as `provider/id`. When `providers` is non-empty,
/// only those providers' sections are kept — `omp models` lists every built-in
/// provider even when unauthenticated.
async fn omp_model_list(bin: &std::path::Path, providers: &[String]) -> Option<Vec<ModelInfo>> {
    let mut cmd = Command::new(bin);
    cmd.args(["models"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    prepare_env(&mut cmd);
    cmd.env("NO_COLOR", "1");
    let out = tokio::time::timeout(MODELS_TIMEOUT, cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let parsed = parse_omp_model_list(&text, providers);
    Some(parsed)
}

fn fallback_models() -> Vec<ModelInfo> {
    vec![]
}

/// Parse the `omp models` table: provider headers are bare `name (count)`
/// lines; model rows look like `│ model-id  │ 200K │ …` (box-drawing), with
/// ids made of word chars plus `-._/`. Only providers whose section yielded at
/// least one row are trusted — an unauthenticated provider prints an empty or
/// error section. When `providers` is non-empty, only those providers are kept.
fn parse_omp_model_list(text: &str, providers: &[String]) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    let mut provider: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = parse_provider_header(line) {
            provider = Some(header);
            continue;
        }
        // Table rows carry the box-drawing vertical bar.
        let Some(row) = line.strip_prefix('│').or_else(|| line.strip_prefix('|')) else {
            continue;
        };
        let id = row.split(['│', '|']).next().unwrap_or("").trim();
        if id.is_empty()
            || id == "model"
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        {
            continue;
        }
        let Some(provider_name) = &provider else {
            continue;
        };
        if !providers.is_empty() && !providers.iter().any(|p| p == provider_name) {
            continue;
        }
        let qualified = format!("{provider_name}/{id}");
        models.push(ModelInfo::new(qualified).with_label(Some(id), None));
    }
    models
}

/// A provider header is a bare `name (N)` line outside the table borders.
fn parse_provider_header(line: &str) -> Option<String> {
    let (name, count) = line.rsplit_once('(')?;
    let count = count.strip_suffix(')')?.trim();
    if count.parse::<u32>().is_err() {
        return None;
    }
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return None;
    }
    Some(name.to_string())
}

fn first_turn_prompt(text: &str) -> String {
    format!(
        "Read and follow `{PLAYBOOK_REL}` before acting. It is the OpenResearch session playbook for this worktree.\n\n{text}"
    )
}

async fn send_prompt(child: &mut tokio::process::Child, prompt: &str) -> Result<()> {
    // Multiline argv is fine for omp, but stdin matches the other harnesses and
    // sidesteps any ARG_MAX concern on very long prepared inputs.
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let result = tokio::time::timeout(TURN_WATCHDOG, stdin.write_all(prompt.as_bytes()))
        .await
        .map_err(|_| anyhow!("omp timed out reading its prompt"))?;
    match result {
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(Into::into),
    }
}

async fn run_turn(ctx: &mut TurnCtx) -> Result<()> {
    let bin = find_omp().ok_or_else(|| {
        anyhow!("omp not found on PATH — install Oh My Pi and configure a provider key first")
    })?;
    let project = ctx.project.clone();
    let session_id = ctx.session_id.clone();
    let skills_dir = OhMyPi.session_skills_dir();
    let (repo, _playbook) =
        tokio::task::spawn_blocking(move || ensure_playbook(&project, &session_id, skills_dir))
            .await
            .map_err(|e| anyhow!("playbook task failed: {e}"))??;

    let resume = ctx.native_session_id.clone();
    let mut prompt = ctx.text.clone();
    if resume.is_none() {
        prompt = first_turn_prompt(&prompt);
    }

    let mut cmd = Command::new(&bin);
    cmd.args(["--print", "--mode", "json", "--no-title", "--cwd"])
        .arg(&repo);
    if let Some(model) = ctx.model.as_deref().filter(|model| !model.is_empty()) {
        cmd.args(["--model", model]);
    }
    // Plan mode and Ask are read-only: strip tools so the agent can only
    // answer. Auto/Full-access auto-approve; omp keeps no further sandbox.
    if ctx.plan_mode || ctx.permission_mode == Some(PermissionMode::Plan) {
        cmd.arg("--no-tools");
    } else {
        match ctx.permission_mode.unwrap_or(PermissionMode::Auto) {
            PermissionMode::Ask => {
                cmd.arg("--no-tools");
            }
            PermissionMode::Auto
            | PermissionMode::AcceptEdits
            | PermissionMode::Bypass
            | PermissionMode::Plan => {
                cmd.args(["--approval-mode", "yolo"]);
            }
        }
    }
    if let Some(native_id) = &resume {
        cmd.args(["--resume", native_id]);
    }
    let log_name = format!("oh-my-pi-{}", uuid::Uuid::new_v4());
    cmd.current_dir(&repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(harness_log(&log_name)?))
        .kill_on_drop(true);
    prepare_env(&mut cmd);
    cmd.env("NO_COLOR", "1");
    set_chat_session_env(&mut cmd, &ctx.session_id, "oh-my-pi", ctx.host.up_port());

    ctx.persist_delivery(DeliveryState::Unknown)?;
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            ctx.mark_delivery(DeliveryState::NotSent);
            return Err(anyhow!("Could not spawn {}: {}", bin.display(), error));
        }
    };
    send_prompt(&mut child, &prompt).await?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut state = TurnState::default();

    loop {
        match tokio::time::timeout(TURN_WATCHDOG, lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                ctx.mark_delivery(DeliveryState::Accepted);
                let terminal = apply_event(ctx, &mut state, &event);
                if let Some(sid) = state.native_session_id.as_deref() {
                    ctx.set_native_session_id(sid);
                }
                ctx.maybe_flush();
                if terminal {
                    break;
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(error)) => {
                return Err(anyhow!("omp stdout: {error}"));
            }
            Err(_) => {
                return Err(anyhow!(
                    "Oh My Pi went silent for {} minutes and was interrupted.",
                    TURN_WATCHDOG.as_secs() / 60
                ));
            }
        }
    }

    let status = child.wait().await?;
    let log_path = crate::store::data_dir().join(format!("agent-{log_name}.log"));
    if !state.saw_result {
        return Err(anyhow!("{}", omp_exit_detail(status, &log_path)));
    }
    if ctx.plan_mode {
        if let Some(card) = plan_card(&ctx.assistant.parts, &ctx.assistant.id, state.turn_errored) {
            ctx.upsert_part(card);
        }
    }
    if state.turn_errored {
        let message = state
            .error_detail
            .clone()
            .unwrap_or_else(|| "Oh My Pi reported a terminal turn error".into());
        ctx.mark_terminal_failure("oh-my-pi_terminal", message);
    } else if status.success() {
        let _ = std::fs::remove_file(log_path);
    }
    let _ = ctx.flush();
    Ok(())
}

fn read_log_tail(path: &std::path::Path, max: usize) -> String {
    use std::io::{Read, Seek, SeekFrom};

    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(max as u64)));
    let mut data = Vec::new();
    let _ = file.take(max as u64).read_to_end(&mut data);
    String::from_utf8_lossy(&data).into_owned()
}

fn omp_exit_detail(status: std::process::ExitStatus, log: &std::path::Path) -> String {
    let tail = read_log_tail(log, 8 * 1024);
    let line = tail
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty());
    line.map(str::to_string).unwrap_or_else(|| {
        format!(
            "Oh My Pi ended without a result ({status}); see {}",
            log.display()
        )
    })
}

#[derive(Default)]
struct TurnState {
    native_session_id: Option<String>,
    text_part_id: Option<String>,
    reasoning_part_id: Option<String>,
    text_seq: usize,
    turn_errored: bool,
    saw_result: bool,
    error_detail: Option<String>,
}

/// Fold one omp `--mode json` stream event into wire parts. Returns true on the
/// terminal event (`agent_end`).
///
/// omp stream shape (v3):
/// - `session` — `id` is the native session id (full UUID; `--resume` takes a prefix).
/// - `message_update.assistantMessageEvent` — `thinking_start/thinking_delta/thinking_end`
///   and `text_start/text_delta/text_end`, each with a `contentIndex` local to the
///   message. Interleaved thinking/text blocks get separate part ids.
/// - `tool_execution_start/update/end` — `toolCallId`, `toolName`, `args`,
///   `partialResult`/`result` (`{content: [{type:"text", text}], details}`, plus
///   `isError` on end).
/// - `agent_end` — terminal; `isTerminal` true.
fn apply_event(ctx: &mut TurnCtx, state: &mut TurnState, event: &Value) -> bool {
    match event.get("type").and_then(Value::as_str) {
        Some("session") => {
            if let Some(sid) = event.get("id").and_then(Value::as_str) {
                state.native_session_id = Some(sid.to_string());
            }
            false
        }
        Some("message_update") => {
            let Some(update) = event.get("assistantMessageEvent") else {
                return false;
            };
            match update.get("type").and_then(Value::as_str) {
                Some("thinking_start") => {
                    close_text_segment(state);
                    let index = update
                        .get("contentIndex")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let id = format!("reasoning-{index}");
                    state.reasoning_part_id = Some(id.clone());
                    ctx.upsert_part(WirePart::reasoning(id, ""));
                }
                Some("thinking_delta") => {
                    if let (Some(id), Some(delta)) = (
                        state.reasoning_part_id.clone(),
                        update.get("delta").and_then(Value::as_str),
                    ) {
                        ctx.append_part_text(&id, delta);
                    }
                }
                Some("thinking_end") => {
                    state.reasoning_part_id = None;
                }
                Some("text_start") => {
                    state.reasoning_part_id = None;
                    let index = update
                        .get("contentIndex")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let id = format!("text-{index}");
                    state.text_part_id = Some(id.clone());
                    ctx.upsert_part(WirePart::text(id, ""));
                }
                Some("text_delta") => {
                    if let (Some(id), Some(delta)) = (
                        state.text_part_id.clone(),
                        update.get("delta").and_then(Value::as_str),
                    ) {
                        ctx.append_part_text(&id, delta);
                    }
                }
                Some("text_end") => {
                    close_text_segment(state);
                }
                _ => {}
            }
            false
        }
        Some("tool_execution_start") => {
            state.reasoning_part_id = None;
            close_text_segment(state);
            let call_id = tool_call_id(state, event);
            let name = tool_name(event);
            let args = event.get("args").cloned();
            ctx.upsert_part(WirePart {
                id: call_id,
                kind: "tool".into(),
                text: None,
                tool: Some(name),
                state: Some(WireToolState {
                    status: "running".into(),
                    input: args,
                    output: None,
                    error: None,
                    title: None,
                }),
                prompt: None,
                phase: None,
                children: Vec::new(),
            });
            false
        }
        Some("tool_execution_end") => {
            let call_id = tool_call_id(state, event);
            let name = tool_name(event);
            let is_error = event
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let output = event.get("result").map(result_text).unwrap_or_default();
            if let Some(part) = find_part_mut(&mut ctx.assistant.parts, &call_id) {
                if let Some(part_state) = part.state.as_mut() {
                    part_state.status = if is_error { "error" } else { "completed" }.into();
                    if is_error {
                        part_state.error = Some(output);
                    } else {
                        part_state.output = Some(output);
                    }
                }
            } else {
                ctx.upsert_part(WirePart {
                    id: call_id,
                    kind: "tool".into(),
                    text: None,
                    tool: Some(name),
                    state: Some(WireToolState {
                        status: if is_error { "error" } else { "completed" }.into(),
                        input: event.get("args").cloned(),
                        output: (!is_error).then_some(output.clone()),
                        error: is_error.then_some(output),
                        title: None,
                    }),
                    prompt: None,
                    phase: None,
                    children: Vec::new(),
                });
            }
            false
        }
        // `turn_end`/`message_end` carry the full assistant message — already
        // streamed as deltas; ignore to avoid duplicating text.
        Some("agent_end") => {
            state.saw_result = true;
            let is_error = event
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if is_error {
                state.turn_errored = true;
                let detail = event
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Oh My Pi reported an error")
                    .to_string();
                state.error_detail = Some(detail.clone());
                ctx.push_error(detail);
            } else {
                ctx.mark_final_text_tail();
            }
            true
        }
        _ => false,
    }
}

fn tool_call_id(state: &mut TurnState, event: &Value) -> String {
    event
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            state.text_seq += 1;
            format!("tool-{}", state.text_seq)
        })
}

fn tool_name(event: &Value) -> String {
    event
        .get("toolName")
        .and_then(Value::as_str)
        .map(title_case)
        .unwrap_or_else(|| "Tool".into())
}

/// omp tool results are `{content: [{type:"text", text}], details}`; the text
/// blocks are the displayable output.
fn result_text(result: &Value) -> String {
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        let text = content
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return text;
        }
    }
    result
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| serde_json::to_string(result).unwrap_or_default())
}

fn close_text_segment(state: &mut TurnState) {
    state.text_part_id = None;
}

fn plan_card(parts: &[WirePart], assistant_id: &str, errored: bool) -> Option<WirePart> {
    let last_text = parts.iter().rev().find_map(|part| {
        (part.kind == "text")
            .then_some(part.text.as_deref())
            .flatten()
            .filter(|text| !text.trim().is_empty())
    })?;
    if !super::should_synthesize_plan(true, false, errored, last_text) {
        return None;
    }
    Some(WirePart::prompt(
        format!("plan-synth-{assistant_id}"),
        WirePrompt {
            kind: "plan".into(),
            plan: Some(last_text.to_string()),
            synthesized: true,
            ..Default::default()
        },
    ))
}

async fn omp_one_shot(bin: &std::path::Path, request: OneShot<'_>) -> Option<String> {
    let message = format!("{}\n\n{}", request.system, request.prompt);
    let mut cmd = Command::new(bin);
    cmd.args([
        "--print",
        "--mode",
        "text",
        "--no-tools",
        "--no-title",
        "--no-session",
    ]);
    if let Some(model) = request.model.filter(|model| !model.is_empty()) {
        cmd.args(["--model", model]);
    } else if matches!(request.quality, OneShotQuality::Cheap) {
        // omp's fast role; falls back to the default model when unset.
        cmd.env(
            "PI_SMOL_MODEL",
            std::env::var("PI_SMOL_MODEL").unwrap_or_default(),
        );
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .current_dir(std::env::temp_dir());
    prepare_env(&mut cmd);
    cmd.env("NO_COLOR", "1");
    let out = tokio::time::timeout(request.timeout, async {
        let mut child = cmd.spawn().ok()?;
        send_prompt(&mut child, &message).await.ok()?;
        child.wait_with_output().await.ok()
    })
    .await
    .ok()??;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_models_table() {
        let text = "\
anthropic (2)
┌──────────────────┬─────────┐
│ model            │ context │
├──────────────────┼─────────┤
│ claude-opus-4-6  │      1M │
│ claude-haiku-4-5 │    200K │
└──────────────────┴─────────┘

kimi (1)
┌───────┬─────────┐
│ model │ context │
├───────┼─────────┤
│ k3    │      1M │
└───────┴─────────┘
";
        let models = parse_omp_model_list(text, &[]);
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "anthropic/claude-opus-4-6",
                "anthropic/claude-haiku-4-5",
                "kimi/k3"
            ]
        );
        assert_eq!(models[0].display_name.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn folds_stream_events() {
        let mut ctx = TurnCtx::test_stub();
        let mut state = TurnState::default();
        let events = [
            r#"{"type":"session","id":"abc-123"}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_start","contentIndex":0}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"hmm"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_end","contentIndex":0}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_start","contentIndex":1}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"Hello"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_end","contentIndex":1}}"#,
            r#"{"type":"tool_execution_start","toolCallId":"t1","toolName":"write","args":{"path":"/tmp/x"}}"#,
            r#"{"type":"tool_execution_end","toolCallId":"t1","toolName":"write","result":{"content":[{"type":"text","text":"done"}]}}"#,
            r#"{"type":"agent_end","isTerminal":true}"#,
        ];
        let mut terminal = false;
        for raw in events {
            let event: Value = serde_json::from_str(raw).unwrap();
            terminal = apply_event(&mut ctx, &mut state, &event);
        }
        assert!(terminal);
        assert!(state.saw_result);
        assert_eq!(state.native_session_id.as_deref(), Some("abc-123"));
        let reasoning = ctx
            .assistant
            .parts
            .iter()
            .find(|p| p.kind == "reasoning")
            .unwrap();
        assert_eq!(reasoning.text.as_deref(), Some("hmm"));
        let text = ctx
            .assistant
            .parts
            .iter()
            .find(|p| p.kind == "text")
            .unwrap();
        assert_eq!(text.text.as_deref(), Some("Hello"));
        let tool = ctx.assistant.parts.iter().find(|p| p.id == "t1").unwrap();
        let tool_state = tool.state.as_ref().unwrap();
        assert_eq!(tool_state.status, "completed");
        assert_eq!(tool_state.output.as_deref(), Some("done"));
    }

    #[test]
    fn terminal_error_marks_turn() {
        let mut ctx = TurnCtx::test_stub();
        let mut state = TurnState::default();
        let event: Value =
            serde_json::from_str(r#"{"type":"agent_end","isError":true,"error":"boom"}"#).unwrap();
        assert!(apply_event(&mut ctx, &mut state, &event));
        assert!(state.turn_errored);
        assert_eq!(state.error_detail.as_deref(), Some("boom"));
    }
}
