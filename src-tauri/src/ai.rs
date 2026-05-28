use crate::error::{AppError, Result};
use crate::skills;
use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::{CStr, CString};
use std::fs;
use std::io::Write;
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const KEYRING_SERVICE: &str = "com.skillloom.desktop";
const API_KEY_ACCOUNT: &str = "ai_api_key";
const DEFAULT_SUMMARY_MODEL: &str = "claude-haiku-4-5-20251001";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;

#[repr(C)]
struct Sqlite3 {
    _private: [u8; 0],
}

#[repr(C)]
struct Sqlite3Stmt {
    _private: [u8; 0],
}

type SqliteCallback =
    Option<unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int>;

#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open(filename: *const c_char, db: *mut *mut Sqlite3) -> c_int;
    fn sqlite3_close(db: *mut Sqlite3) -> c_int;
    fn sqlite3_errmsg(db: *mut Sqlite3) -> *const c_char;
    fn sqlite3_exec(
        db: *mut Sqlite3,
        sql: *const c_char,
        callback: SqliteCallback,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_free(ptr: *mut c_void);
    fn sqlite3_prepare_v2(
        db: *mut Sqlite3,
        sql: *const c_char,
        n_byte: c_int,
        stmt: *mut *mut Sqlite3Stmt,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut Sqlite3Stmt,
        index: c_int,
        value: *const c_char,
        n_byte: c_int,
        destructor: *mut c_void,
    ) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut Sqlite3Stmt, index: c_int, value: i64) -> c_int;
    fn sqlite3_step(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_column_text(stmt: *mut Sqlite3Stmt, index: c_int) -> *const c_char;
    fn sqlite3_finalize(stmt: *mut Sqlite3Stmt) -> c_int;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub configured: bool,
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

fn sqlite_transient() -> *mut c_void {
    -1_isize as *mut c_void
}

fn sqlite_message(raw: *mut Sqlite3) -> String {
    if raw.is_null() {
        return "sqlite error".into();
    }

    let message = unsafe { sqlite3_errmsg(raw) };
    if message.is_null() {
        "sqlite error".into()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    }
}

fn sqlite_string(value: &str, context: &str) -> Result<CString> {
    CString::new(value).map_err(|_| AppError::Database(format!("{context} contains a NUL byte")))
}

struct CacheDb {
    raw: *mut Sqlite3,
}

impl CacheDb {
    fn open(path: &Path) -> Result<Self> {
        let path = sqlite_string(&path.to_string_lossy(), "cache path")?;
        let mut raw = ptr::null_mut();
        let code = unsafe { sqlite3_open(path.as_ptr(), &mut raw) };
        if code != SQLITE_OK {
            let message = sqlite_message(raw);
            if !raw.is_null() {
                unsafe { sqlite3_close(raw) };
            }
            return Err(AppError::Database(message));
        }
        Ok(Self { raw })
    }

    fn exec(&self, sql: &str) -> Result<()> {
        let sql = sqlite_string(sql, "sql")?;
        let mut error = ptr::null_mut();
        let code =
            unsafe { sqlite3_exec(self.raw, sql.as_ptr(), None, ptr::null_mut(), &mut error) };
        if code == SQLITE_OK {
            return Ok(());
        }

        let message = if error.is_null() {
            sqlite_message(self.raw)
        } else {
            let message = unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned();
            unsafe { sqlite3_free(error.cast()) };
            message
        };
        Err(AppError::Database(message))
    }

    fn prepare(&self, sql: &str) -> Result<Statement> {
        let sql = sqlite_string(sql, "sql")?;
        let mut raw = ptr::null_mut();
        let code =
            unsafe { sqlite3_prepare_v2(self.raw, sql.as_ptr(), -1, &mut raw, ptr::null_mut()) };
        if code != SQLITE_OK {
            return Err(AppError::Database(sqlite_message(self.raw)));
        }
        Ok(Statement { raw, db: self.raw })
    }
}

impl Drop for CacheDb {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sqlite3_close(self.raw) };
        }
    }
}

struct Statement {
    raw: *mut Sqlite3Stmt,
    db: *mut Sqlite3,
}

impl Statement {
    fn bind_text(&self, index: c_int, value: &str) -> Result<()> {
        let value = sqlite_string(value, "sqlite parameter")?;
        let code =
            unsafe { sqlite3_bind_text(self.raw, index, value.as_ptr(), -1, sqlite_transient()) };
        if code == SQLITE_OK {
            Ok(())
        } else {
            Err(AppError::Database(sqlite_message(self.db)))
        }
    }

    fn bind_int64(&self, index: c_int, value: i64) -> Result<()> {
        let code = unsafe { sqlite3_bind_int64(self.raw, index, value) };
        if code == SQLITE_OK {
            Ok(())
        } else {
            Err(AppError::Database(sqlite_message(self.db)))
        }
    }

    fn step(&self) -> Result<c_int> {
        let code = unsafe { sqlite3_step(self.raw) };
        match code {
            SQLITE_ROW | SQLITE_DONE => Ok(code),
            _ => Err(AppError::Database(sqlite_message(self.db))),
        }
    }

    fn column_text(&self, index: c_int) -> String {
        let value = unsafe { sqlite3_column_text(self.raw, index) };
        if value.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned()
        }
    }
}

impl Drop for Statement {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sqlite3_finalize(self.raw) };
        }
    }
}

fn open_cache(app: &tauri::AppHandle) -> Result<CacheDb> {
    let conn = CacheDb::open(&cache_path(app)?)?;
    conn.exec(
        "CREATE TABLE IF NOT EXISTS ai_summary (
            skill_id TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            summary TEXT NOT NULL,
            model TEXT NOT NULL,
            generated_at INTEGER NOT NULL,
            PRIMARY KEY (skill_id, content_hash)
        )",
    )?;
    Ok(conn)
}

pub fn delete_cached_summaries(app: &tauri::AppHandle, skill_id: &str) -> Result<()> {
    let path = cache_dir(app)?.join("cache.db");
    if !path.exists() {
        return Ok(());
    }

    let conn = CacheDb::open(&path)?;
    let stmt = conn.prepare("DELETE FROM ai_summary WHERE skill_id = ?1")?;
    stmt.bind_text(1, skill_id)?;
    stmt.step()?;
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

fn cached_summary(conn: &CacheDb, skill_id: &str, hash: &str) -> Result<Option<String>> {
    let stmt =
        conn.prepare("SELECT summary FROM ai_summary WHERE skill_id = ?1 AND content_hash = ?2")?;
    stmt.bind_text(1, skill_id)?;
    stmt.bind_text(2, hash)?;
    if stmt.step()? == SQLITE_ROW {
        Ok(Some(stmt.column_text(0)))
    } else {
        Ok(None)
    }
}

fn store_summary(
    conn: &CacheDb,
    skill_id: &str,
    hash: &str,
    summary: &str,
    model: &str,
) -> Result<()> {
    let generated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    let stmt = conn.prepare(
        "INSERT OR REPLACE INTO ai_summary
            (skill_id, content_hash, summary, model, generated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    stmt.bind_text(1, skill_id)?;
    stmt.bind_text(2, hash)?;
    stmt.bind_text(3, summary)?;
    stmt.bind_text(4, model)?;
    stmt.bind_int64(5, generated_at)?;
    stmt.step()?;
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

fn request_summary(api_key: &str, skill_id: &str, skill_md: &str) -> Result<String> {
    let request = AnthropicRequest {
        model: DEFAULT_SUMMARY_MODEL.into(),
        max_tokens: 180,
        system: "You write concise, accurate summaries of AI agent skills.".into(),
        messages: vec![AnthropicMessage {
            role: "user".into(),
            content: summary_prompt(skill_id, skill_md),
        }],
    };
    let body = serde_json::to_vec(&request)?;
    let temp_path = std::env::temp_dir().join(format!(
        "skillloom-summary-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ));
    fs::write(&temp_path, body)?;

    let api_key = curl_config_value(api_key, "api key")?;
    let body_path = curl_config_value(&temp_path.to_string_lossy(), "request path")?;
    let config = format!(
        "url = \"https://api.anthropic.com/v1/messages\"\n\
         request = \"POST\"\n\
         header = \"content-type: application/json\"\n\
         header = \"anthropic-version: {ANTHROPIC_VERSION}\"\n\
         header = \"x-api-key: {api_key}\"\n\
         data-binary = \"@{body_path}\"\n"
    );

    let mut child = Command::new("/usr/bin/curl")
        .arg("--silent")
        .arg("--show-error")
        .arg("--fail-with-body")
        .arg("--max-time")
        .arg("45")
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

    let response = serde_json::from_slice::<AnthropicResponse>(&output.stdout)?;

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
pub fn generate_summary(app: tauri::AppHandle, skill_id: String, force: bool) -> Result<String> {
    let detail = skills::get_skill_detail(skill_id.clone())?;
    let hash = content_hash(&detail.skill_md);
    let conn = open_cache(&app)?;

    if !force {
        if let Some(summary) = cached_summary(&conn, &skill_id, &hash)? {
            return Ok(summary);
        }
    }

    let Some(api_key) = read_api_key()? else {
        return Ok(detail.skill.tagline);
    };

    let summary = request_summary(&api_key, &skill_id, &detail.skill_md)?;
    store_summary(&conn, &skill_id, &hash, &summary, DEFAULT_SUMMARY_MODEL)?;
    Ok(summary)
}
