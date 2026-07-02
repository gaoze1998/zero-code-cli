use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::{Message, MessageRole, Mode};
use crate::debug;

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    pub session_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub plan_messages: Vec<Message>,
    pub build_messages: Vec<Message>,
    pub plan_artifact: Option<String>,
    pub current_mode: Mode,
}

pub struct SessionInfo {
    pub filename: String,
    pub session_name: String,
    pub updated_at: String,
}

fn memory_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".zero-code-cli")
        .join("memory")
}

fn project_sessions_dir(project_name: &str) -> PathBuf {
    memory_dir().join(project_name).join("sessions")
}

pub fn make_session_filename(session_name: &str) -> String {
    let ts = unix_timestamp();
    format!("{}_{}.json", session_name, ts)
}

pub fn save_session(project_name: &str, filename: &str, data: &SessionData) -> Result<(), String> {
    let dir = project_sessions_dir(project_name);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create sessions dir: {}", e))?;
    let path = dir.join(filename);
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write session file: {}", e))?;
    debug!("Session saved: {}", path.display());
    Ok(())
}

pub fn load_session(project_name: &str, filename: &str) -> Result<SessionData, String> {
    let path = project_sessions_dir(project_name).join(filename);
    let json =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read session file: {}", e))?;
    let data: SessionData =
        serde_json::from_str(&json).map_err(|e| format!("Failed to deserialize session: {}", e))?;
    Ok(data)
}

pub fn list_sessions(project_name: &str) -> Result<Vec<SessionInfo>, String> {
    let dir = project_sessions_dir(project_name);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut infos = Vec::new();
    for entry in
        fs::read_dir(&dir).map_err(|e| format!("Failed to read sessions dir: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".json") {
            continue;
        }
        match fs::read_to_string(entry.path()) {
            Ok(json) => {
                if let Ok(data) = serde_json::from_str::<SessionData>(&json) {
                    infos.push(SessionInfo {
                        filename,
                        session_name: data.session_name,
                        updated_at: data.updated_at,
                    });
                }
            }
            Err(_) => continue,
        }
    }
    infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(infos)
}

pub fn generate_session_name(
    plan_messages: &[Message],
    build_messages: &[Message],
) -> String {
    let first_user = plan_messages
        .iter()
        .chain(build_messages.iter())
        .find(|m| matches!(m.role, MessageRole::User));

    match first_user {
        Some(msg) => sanitize_filename(&msg.content, 40),
        None => "session".to_string(),
    }
}

pub fn now_readable() -> String {
    let secs = unix_timestamp();
    let (y, m, d, hh, mm, ss) = secs_to_datetime(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, hh, mm, ss)
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sanitize_filename(name: &str, max_len: usize) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_whitespace() => '_',
            c => c,
        })
        .take(max_len)
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() { "session" } else { trimmed }.to_string()
}

fn days_to_date(days: i64) -> (i64, i64, i64) {
    // Howard Hinnant's algorithm: days since civil epoch to year/month/day
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn secs_to_datetime(secs: u64) -> (i64, i64, i64, u64, u64, u64) {
    let days = (secs / 86400) as i64;
    let tod = secs % 86400;
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    let (y, m, d) = days_to_date(days);
    (y, m, d, hh, mm, ss)
}
