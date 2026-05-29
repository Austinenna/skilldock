use crate::config;
use crate::error::{AppError, Result};
use crate::skills;
use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const KEYRING_SERVICE: &str = "com.skillloom.desktop";
const API_KEY_ACCOUNT: &str = "ai_api_key";
const DEFAULT_ANTHROPIC_ENDPOINT: &str = "https://api.minimaxi.com/anthropic/v1/messages";
const DEFAULT_ANTHROPIC_MODEL: &str = "MiniMax-M2.7";
const DEFAULT_CHAT_ENDPOINT: &str = "https://token-plan-sgp.xiaomimimo.com/v1/chat/completions";
const DEFAULT_CHAT_MODEL: &str = "mimo-v2.5-pro";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const SQLITE_BIN: &str = "/usr/bin/sqlite3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiProvider {
    Anthropic,
    ChatCompletions,
}

impl AiProvider {
    fn from_config(value: &str) -> Self {
        match value {
            "chat" | "chat-completions" | "openai" | "openai-compatible" => Self::ChatCompletions,
            _ => Self::Anthropic,
        }
    }

    fn default_endpoint(self) -> &'static str {
        match self {
            Self::Anthropic => DEFAULT_ANTHROPIC_ENDPOINT,
            Self::ChatCompletions => DEFAULT_CHAT_ENDPOINT,
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => DEFAULT_ANTHROPIC_MODEL,
            Self::ChatCompletions => DEFAULT_CHAT_MODEL,
        }
    }

    fn cache_model_label(self, model: &str, endpoint: &str) -> String {
        let endpoint_hash = content_hash(endpoint);
        match self {
            Self::Anthropic => format!("anthropic:{model}:{endpoint_hash}"),
            Self::ChatCompletions => format!("chat:{model}:{endpoint_hash}"),
        }
    }

    fn as_config_value(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::ChatCompletions => "chat",
        }
    }
}

struct AiRequestConfig {
    provider: AiProvider,
    endpoint: String,
    model: String,
    model_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTestResult {
    pub provider: String,
    pub model: String,
    pub response: String,
}

fn api_key_entry() -> Result<Entry> {
    Ok(Entry::new(KEYRING_SERVICE, API_KEY_ACCOUNT)?)
}

pub fn read_api_key() -> Result<Option<String>> {
    match api_key_entry()?.get_password() {
        Ok(key) if !key.trim().is_empty() => Ok(Some(key)),
        Ok(_) => Ok(None),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn cache_path(app: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = cache_dir(app)?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("cache.db"))
}

fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf> {
    app.path().app_data_dir().map_err(|_| AppError::NoHomeDir)
}

fn sql_literal(value: &str) -> Result<String> {
    if value.contains('\0') {
        return Err(AppError::Database(
            "sqlite parameter contains a NUL byte".into(),
        ));
    }
    Ok(format!("'{}'", value.replace('\'', "''")))
}

fn run_sqlite(path: &PathBuf, script: &str) -> Result<String> {
    let mut child = Command::new(SQLITE_BIN)
        .arg("-batch")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(script.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(AppError::Database(if message.is_empty() {
            "sqlite command failed".into()
        } else {
            message
        }))
    }
}

fn open_cache(app: &tauri::AppHandle) -> Result<PathBuf> {
    let path = cache_path(app)?;
    run_sqlite(
        &path,
        "CREATE TABLE IF NOT EXISTS ai_summary (
            skill_id TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            summary TEXT NOT NULL,
            model TEXT NOT NULL,
            generated_at INTEGER NOT NULL,
            PRIMARY KEY (skill_id, content_hash)
        );",
    )?;
    Ok(path)
}

pub fn delete_cached_summaries(app: &tauri::AppHandle, skill_id: &str) -> Result<()> {
    let path = cache_dir(app)?.join("cache.db");
    if !path.exists() {
        return Ok(());
    }

    run_sqlite(
        &path,
        &format!(
            "DELETE FROM ai_summary WHERE skill_id = {};",
            sql_literal(skill_id)?
        ),
    )?;
    Ok(())
}

fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

#[derive(Debug, Deserialize)]
struct CachedSummaryRow {
    summary: String,
}

fn cached_summary(
    path: &PathBuf,
    skill_id: &str,
    hash: &str,
    model_label: &str,
) -> Result<Option<String>> {
    let output = run_sqlite(
        path,
        &format!(
            ".mode json\nSELECT summary FROM ai_summary WHERE skill_id = {} AND content_hash = {} AND model = {} LIMIT 1;",
            sql_literal(skill_id)?,
            sql_literal(hash)?,
            sql_literal(model_label)?,
        ),
    )?;
    let output = output.trim();
    if output.is_empty() {
        return Ok(None);
    }

    let rows: Vec<CachedSummaryRow> = serde_json::from_str(output)?;
    Ok(rows.into_iter().next().map(|row| row.summary))
}

fn store_summary(
    path: &PathBuf,
    skill_id: &str,
    hash: &str,
    summary: &str,
    model: &str,
) -> Result<()> {
    let generated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    run_sqlite(
        path,
        &format!(
            "INSERT OR REPLACE INTO ai_summary
            (skill_id, content_hash, summary, model, generated_at)
            VALUES ({}, {}, {}, {}, {});",
            sql_literal(skill_id)?,
            sql_literal(hash)?,
            sql_literal(summary)?,
            sql_literal(model)?,
            generated_at,
        ),
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_completion_tokens: u32,
    temperature: f32,
    top_p: f32,
    stream: bool,
    stop: Option<Vec<String>>,
    frequency_penalty: f32,
    presence_penalty: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

fn summary_prompt(skill_id: &str, skill_md: &str) -> String {
    format!(
        "Summarize this SkillLoom skill for a local skill manager UI.\n\
         Skill id: {skill_id}\n\n\
         Requirements:\n\
         - Write 2 concise sentences.\n\
         - Explain what the skill is for and when to use it.\n\
         - Mention notable prerequisites only if they are explicit.\n\
         - Do not use markdown bullets.\n\n\
         SKILL.md:\n{skill_md}"
    )
}

fn curl_config_value(value: &str, label: &str) -> Result<String> {
    if value.chars().any(|ch| ch == '\n' || ch == '\r') {
        return Err(AppError::Ai(format!("{label} contains a newline")));
    }
    Ok(value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn configured_value(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_ai_config(config: &config::Config) -> AiRequestConfig {
    let provider = AiProvider::from_config(&config.ai_provider);
    let endpoint = configured_value(&config.ai_endpoint, provider.default_endpoint());
    let model = configured_value(&config.ai_model, provider.default_model());
    let model_label = provider.cache_model_label(&model, &endpoint);
    AiRequestConfig {
        provider,
        endpoint,
        model,
        model_label,
    }
}

fn request_with_curl(endpoint: &str, headers: &[(&str, String)], body: &[u8]) -> Result<Vec<u8>> {
    let temp_path = std::env::temp_dir().join(format!(
        "skillloom-ai-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ));
    fs::write(&temp_path, body)?;

    let endpoint = curl_config_value(endpoint, "ai endpoint")?;
    let body_path = curl_config_value(&temp_path.to_string_lossy(), "request path")?;
    let mut config = format!(
        "url = \"{endpoint}\"\n\
         request = \"POST\"\n\
         data-binary = \"@{body_path}\"\n"
    );
    for (name, value) in headers {
        let value = curl_config_value(value, name)?;
        config.push_str(&format!("header = \"{name}: {value}\"\n"));
    }

    let mut child = Command::new("/usr/bin/curl")
        .arg("--silent")
        .arg("--show-error")
        .arg("--fail-with-body")
        .arg("--max-time")
        .arg("120")
        .arg("--config")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(config.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let _ = fs::remove_file(&temp_path);
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let body = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(AppError::Ai(if body.is_empty() {
            message
        } else if message.is_empty() {
            body
        } else {
            format!("{message}: {body}")
        }));
    }

    Ok(output.stdout)
}

fn text_from_anthropic_response(body: &[u8]) -> Result<String> {
    let response = serde_json::from_slice::<AnthropicResponse>(body)?;
    response
        .content
        .into_iter()
        .find_map(|block| {
            if block.kind == "text" {
                block.text.and_then(|text| {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
            } else {
                None
            }
        })
        .ok_or_else(|| AppError::Ai("summary response did not contain text".into()))
}

fn text_from_chat_response(body: &[u8]) -> Result<String> {
    let response = serde_json::from_slice::<ChatCompletionsResponse>(body)?;
    response
        .choices
        .into_iter()
        .find_map(|choice| {
            let trimmed = choice.message.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .ok_or_else(|| AppError::Ai("chat completion response did not contain text".into()))
}

fn request_anthropic_prompt(
    api_key: &str,
    endpoint: &str,
    model: &str,
    prompt: String,
    max_tokens: u32,
) -> Result<String> {
    let request = AnthropicRequest {
        model: model.into(),
        max_tokens,
        system: "You write concise, accurate responses for SkillLoom.".into(),
        messages: vec![AnthropicMessage {
            role: "user".into(),
            content: prompt,
        }],
    };
    let body = serde_json::to_vec(&request)?;
    let response = request_with_curl(
        endpoint,
        &[
            ("content-type", "application/json".into()),
            ("anthropic-version", ANTHROPIC_VERSION.into()),
            ("x-api-key", api_key.into()),
        ],
        &body,
    )?;
    text_from_anthropic_response(&response)
}

fn request_chat_prompt(
    api_key: &str,
    endpoint: &str,
    model: &str,
    prompt: String,
    max_tokens: u32,
) -> Result<String> {
    let request = ChatCompletionsRequest {
        model: model.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompt,
        }],
        max_completion_tokens: max_tokens,
        temperature: 0.3,
        top_p: 0.95,
        stream: false,
        stop: None,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
    };
    let body = serde_json::to_vec(&request)?;
    let response = request_with_curl(
        endpoint,
        &[
            ("content-type", "application/json".into()),
            ("api-key", api_key.into()),
        ],
        &body,
    )?;
    text_from_chat_response(&response)
}

fn request_anthropic_summary(
    api_key: &str,
    endpoint: &str,
    model: &str,
    skill_id: &str,
    skill_md: &str,
) -> Result<String> {
    request_anthropic_prompt(
        api_key,
        endpoint,
        model,
        summary_prompt(skill_id, skill_md),
        180,
    )
}

fn request_chat_summary(
    api_key: &str,
    endpoint: &str,
    model: &str,
    skill_id: &str,
    skill_md: &str,
) -> Result<String> {
    request_chat_prompt(
        api_key,
        endpoint,
        model,
        summary_prompt(skill_id, skill_md),
        180,
    )
}

fn request_summary(
    api_key: &str,
    config: &AiRequestConfig,
    skill_id: &str,
    skill_md: &str,
) -> Result<String> {
    match config.provider {
        AiProvider::Anthropic => {
            request_anthropic_summary(api_key, &config.endpoint, &config.model, skill_id, skill_md)
        }
        AiProvider::ChatCompletions => {
            request_chat_summary(api_key, &config.endpoint, &config.model, skill_id, skill_md)
        }
    }
}

fn request_test_message(api_key: &str, config: &AiRequestConfig) -> Result<String> {
    let prompt = "请只用一句中文回复：连接成功。".to_string();
    match config.provider {
        AiProvider::Anthropic => {
            request_anthropic_prompt(api_key, &config.endpoint, &config.model, prompt, 96)
        }
        AiProvider::ChatCompletions => {
            request_chat_prompt(api_key, &config.endpoint, &config.model, prompt, 96)
        }
    }
}

#[tauri::command]
pub fn get_api_key_status() -> Result<ApiKeyStatus> {
    Ok(ApiKeyStatus {
        configured: read_api_key()?.is_some(),
    })
}

#[tauri::command]
pub fn set_api_key(key: String) -> Result<ApiKeyStatus> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        clear_api_key()?;
    } else {
        api_key_entry()?.set_password(trimmed)?;
    }
    get_api_key_status()
}

#[tauri::command]
pub fn clear_api_key() -> Result<ApiKeyStatus> {
    match api_key_entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => get_api_key_status(),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub fn test_ai_config(app: tauri::AppHandle) -> Result<AiTestResult> {
    let config = config::read_config(&app)?;
    let ai_config = resolve_ai_config(&config);
    let Some(api_key) = read_api_key()? else {
        return Err(AppError::Ai(
            "API key is not configured. Save one in Settings first.".into(),
        ));
    };
    let response = request_test_message(&api_key, &ai_config)?;
    Ok(AiTestResult {
        provider: ai_config.provider.as_config_value().into(),
        model: ai_config.model,
        response,
    })
}

#[tauri::command]
pub fn generate_summary(app: tauri::AppHandle, skill_id: String, force: bool) -> Result<String> {
    let detail = skills::get_skill_detail(skill_id.clone())?;
    let hash = content_hash(&detail.skill_md);
    let conn = open_cache(&app)?;
    let config = config::read_config(&app)?;
    let ai_config = resolve_ai_config(&config);

    if !force {
        if let Some(summary) = cached_summary(&conn, &skill_id, &hash, &ai_config.model_label)? {
            return Ok(summary);
        }
    }

    let Some(api_key) = read_api_key()? else {
        if force {
            return Err(AppError::Ai(
                "API key is not configured. Save one in Settings first.".into(),
            ));
        }
        return Ok(detail.skill.tagline);
    };

    let summary = request_summary(&api_key, &ai_config, &skill_id, &detail.skill_md)?;
    store_summary(&conn, &skill_id, &hash, &summary, &ai_config.model_label)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::{sql_literal, text_from_anthropic_response, text_from_chat_response};

    #[test]
    fn quotes_sqlite_string_literals() {
        assert_eq!(sql_literal("simple").unwrap(), "'simple'");
        assert_eq!(sql_literal("it's fine").unwrap(), "'it''s fine'");
    }

    #[test]
    fn rejects_nul_in_sqlite_literals() {
        assert!(sql_literal("bad\0value").is_err());
    }

    #[test]
    fn parses_anthropic_text_response() {
        let body = br#"{"content":[{"type":"text","text":"  connection ok  "}]}"#;
        assert_eq!(text_from_anthropic_response(body).unwrap(), "connection ok");
    }

    #[test]
    fn parses_chat_completion_text_response() {
        let body =
            br#"{"choices":[{"message":{"role":"assistant","content":"  connection ok  "}}]}"#;
        assert_eq!(text_from_chat_response(body).unwrap(), "connection ok");
    }
}
