use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::api;
use crate::app::AgentEvent;
use crate::config::Config;
use crate::debug;

// ── Paths ──────────────────────────────────────────────────────────────────

pub(crate) fn project_memory_dir(project_name: &str) -> PathBuf {
    crate::session::memory_dir().join(project_name)
}

pub(crate) fn memory_md_path(project_name: &str) -> PathBuf {
    project_memory_dir(project_name).join("memory.md")
}

fn project_sessions_dir(project_name: &str) -> PathBuf {
    crate::session::memory_dir().join(project_name).join("sessions")
}

// ── Topic block parsing ────────────────────────────────────────────────────

const RELEVANCE_THRESHOLD: f64 = 0.10;

struct TopicBlock {
    title: String,
    content: String,
    start_byte: usize,
    end_byte: usize,
}

fn parse_topic_blocks(markdown: &str) -> Vec<TopicBlock> {
    let mut blocks = Vec::new();
    let mut current_title = String::new();
    let mut current_start = None;
    let mut pos = 0;

    for line in markdown.lines() {
        let line_start = pos;
        let line_len = line.len() + 1; // +1 for newline
        pos += line_len;

        if let Some(stripped) = line.strip_prefix("## Topic:") {
            if let Some(start) = current_start {
                blocks.push(TopicBlock {
                    title: std::mem::take(&mut current_title),
                    content: String::new(),
                    start_byte: start,
                    end_byte: line_start.saturating_sub(1),
                });
            }
            current_title = stripped.trim().to_string();
            current_start = Some(line_start);
        }
    }

    if let Some(start) = current_start {
        blocks.push(TopicBlock {
            title: current_title,
            content: String::new(),
            start_byte: start,
            end_byte: markdown.len(),
        });
    }

    // Fill in content for each block
    if !blocks.is_empty() {
        for i in 0..blocks.len() {
            let end = if i + 1 < blocks.len() {
                blocks[i + 1].start_byte
            } else {
                markdown.len()
            };
            let raw = &markdown[blocks[i].start_byte..end];
            // Extract content after the ## Topic: header line
            let body = raw
                .split_once('\n')
                .map(|(_, rest)| rest)
                .unwrap_or("")
                .trim_end_matches(['\n', '-']);
            blocks[i].content = body.trim().to_string();
            blocks[i].end_byte = end;
        }
    }

    blocks
}

fn extract_keywords(query: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    query
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .filter_map(|w| {
            let cleaned: String = w
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if cleaned.len() > 2 && seen.insert(cleaned.clone()) {
                Some(cleaned)
            } else {
                None
            }
        })
        .collect()
}

fn score_relevance(block: &TopicBlock, keywords: &[String]) -> f64 {
    if keywords.is_empty() {
        return 0.0;
    }
    let title_lower = block.title.to_lowercase();
    let content_lower = block.content.to_lowercase();
    let mut score = 0.0;
    for kw in keywords {
        if title_lower.contains(kw.as_str()) {
            score += 2.0;
        } else if content_lower.contains(kw.as_str()) {
            score += 1.0;
        }
    }
    score / (keywords.len() as f64 * 2.0)
}

// ── Memory search ───────────────────────────────────────────────────────────

pub(crate) fn search_memory(project_name: &str, query: &str) -> Option<String> {
    let markdown = load_memory_md(project_name)?;
    let blocks = parse_topic_blocks(&markdown);
    if blocks.is_empty() {
        return None;
    }
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return None;
    }
    let mut scored: Vec<(f64, &TopicBlock)> = blocks
        .iter()
        .map(|b| (score_relevance(b, &keywords), b))
        .filter(|(s, _)| *s >= RELEVANCE_THRESHOLD)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    if scored.is_empty() {
        return None;
    }

    let mut result = String::from("[Relevant memory from past sessions:]\n\n");
    for (_, block) in scored.iter().take(5) {
        result.push_str(&format!("## Topic: {}\n{}\n\n", block.title, block.content));
    }
    Some(result)
}

// ── Memory I/O ──────────────────────────────────────────────────────────────

pub(crate) fn load_memory_md(project_name: &str) -> Option<String> {
    let path = memory_md_path(project_name);
    if !path.exists() {
        return None;
    }
    fs::read_to_string(&path).ok()
}

pub(crate) fn save_memory_md(project_name: &str, content: &str) -> Result<(), String> {
    let dir = project_memory_dir(project_name);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create memory dir: {}", e))?;
    let path = memory_md_path(project_name);
    fs::write(&path, content).map_err(|e| format!("Failed to write memory.md: {}", e))?;
    debug!("Long-term memory saved: {}", path.display());
    Ok(())
}

// ── Session expiry check ────────────────────────────────────────────────────

pub(crate) fn check_expired_sessions(
    project_name: &str,
    max_age_days: u64,
) -> (bool, Vec<String>) {
    let dir = project_sessions_dir(project_name);
    if !dir.exists() {
        return (false, Vec::new());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = now.saturating_sub(max_age_days * 86400);

    let mut expired = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return (false, Vec::new()),
    };

    for entry in entries.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.ends_with(".json") {
            continue;
        }
        // Extract unix timestamp from filename: <name>_<ts>.json
        let stem = fname.strip_suffix(".json").unwrap_or(&fname);
        if let Some(ts_str) = stem.rsplit('_').next()
            && let Ok(ts) = ts_str.parse::<u64>()
            && ts < cutoff
        {
            expired.push(fname);
        }
    }

    let any = !expired.is_empty();
    if any {
        debug!(
            "Found {} expired session(s) for project {}",
            expired.len(),
            project_name
        );
    }
    (any, expired)
}

// ── Session text collection ─────────────────────────────────────────────────

const MAX_EXCHANGES_PER_SESSION: usize = 50;

pub(crate) fn collect_all_session_texts(project_name: &str) -> Result<String, String> {
    let dir = project_sessions_dir(project_name);
    if !dir.exists() {
        return Ok(String::new());
    }

    let mut all_text = String::new();
    let sessions = crate::session::list_sessions(project_name)?;

    for info in &sessions {
        let data = crate::session::load_session(project_name, &info.filename)?;
        let mut session_text = format!(
            "=== Session: {} (created: {}, updated: {}) ===\n",
            data.session_name, data.created_at, data.updated_at
        );

        let all_messages: Vec<&crate::app::Message> = data
            .plan_messages
            .iter()
            .chain(data.build_messages.iter())
            .collect();

        // Skip the first 2 messages (welcome + mode-specific system prompt)
        let relevant: Vec<&&crate::app::Message> = all_messages
            .iter()
            .skip(2)
            .filter(|m| {
                matches!(
                    m.role,
                    crate::app::MessageRole::User | crate::app::MessageRole::Agent
                )
            })
            .collect();

        // Limit to last N exchanges
        let start = if relevant.len() > MAX_EXCHANGES_PER_SESSION * 2 {
            relevant.len() - MAX_EXCHANGES_PER_SESSION * 2
        } else {
            0
        };

        for msg in &relevant[start..] {
            let role = match msg.role {
                crate::app::MessageRole::User => "User",
                crate::app::MessageRole::Agent => "Agent",
                _ => continue,
            };
            let content = if msg.content.len() > 2000 {
                format!("{}...", &msg.content[..2000])
            } else {
                msg.content.clone()
            };
            session_text.push_str(&format!("[{}]: {}\n", role, content));
        }
        session_text.push('\n');
        all_text.push_str(&session_text);
    }

    Ok(all_text)
}

// ── Size limit enforcement ──────────────────────────────────────────────────

const MAX_MEMORY_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

pub(crate) fn enforce_size_limit(path: &PathBuf, max_bytes: u64) -> Result<(), String> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };

    if metadata.len() <= max_bytes {
        return Ok(());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read memory.md for truncation: {}", e))?;

    // Split on "## Topic:" to get individual blocks plus frontmatter
    // First element is everything before the first "## Topic:"
    let mut parts: Vec<&str> = content.splitn(2, "## Topic:").collect();
    let frontmatter = parts[0].to_string();

    // Find all topic blocks
    let mut topics: Vec<String> = Vec::new();
    let remaining = if parts.len() > 1 {
        parts.swap_remove(1)
    } else {
        return Ok(()); // no topics, nothing to truncate
    };

    for part in remaining.split("## Topic:") {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            topics.push(format!("## Topic:{}", part));
        }
    }

    if topics.is_empty() {
        return Ok(());
    }

    // Remove oldest topics (from the front) until under size limit
    // Reserve ~100 bytes for the truncation notice that will be added
    let frontmatter_size = frontmatter.len() as u64;
    let notice_reserve: u64 = 100;
    let effective_limit = max_bytes.saturating_sub(notice_reserve);
    let mut removed = 0u64;

    while !topics.is_empty() {
        let total: u64 = frontmatter_size + topics.iter().map(|t| t.len() as u64).sum::<u64>();
        if total <= effective_limit {
            break;
        }
        topics.remove(0);
        removed += 1;
    }

    if removed == 0 {
        return Ok(());
    }

    let notice = format!(
        "<!-- WARNING: Truncated to stay under {}MB. {} oldest topic(s) removed. -->\n\n",
        max_bytes / (1024 * 1024),
        removed
    );
    let truncated = format!("{}{}{}", frontmatter, notice, topics.join(""));

    fs::write(path, truncated)
        .map_err(|e| format!("Failed to write truncated memory.md: {}", e))?;

    debug!(
        "Truncated memory.md: removed {} oldest blocks, saved under {}MB",
        removed,
        max_bytes / (1024 * 1024)
    );
    Ok(())
}

// ── Summarization ───────────────────────────────────────────────────────────

const SUMMARIZE_SYSTEM_PROMPT: &str = "\
You are a knowledge summarization engine. Your task is to analyze conversation histories \
from multiple programming/development sessions and produce a structured long-term memory \
document in markdown format.

Instructions:
1. Read through all provided session transcripts carefully.
2. Identify distinct topics discussed across sessions (e.g., specific features, bugs fixed, \
   architectural decisions, library usage patterns, configuration details, project conventions).
3. For each topic, write a concise but information-dense summary (2-5 paragraphs). Include:
   - What was decided or implemented
   - Specific code patterns, file paths, or configuration values mentioned
   - Rationale behind decisions
   - Any caveats or gotchas discovered
4. Group related information into the same topic block rather than creating many small blocks.
5. At the end of each topic block, add a \"### Cross-References\" section linking to other \
   related topics. Use the format: \"- [Topic: Name](#topic-name) -- relationship description\"
6. Use the following exact format for each topic block (separate blocks with \"---\"):

## Topic: <Descriptive Title>
**Last Updated**: <current datetime>
**Source Sessions**: <number of sessions this topic appeared in>

<summary content>

### Cross-References
- [Topic: Other](#topic-other) -- brief relationship note

7. Do NOT include a top-level \"# Long-Term Memory\" header -- that will be added automatically.
8. Write in English. Be specific and concrete, not vague.
9. If the existing memory already covers a topic, update and enrich it rather than duplicating.";

pub async fn run_summarization(
    config: &Config,
    project_name: &str,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
) {
    // 1. Collect session texts
    let session_texts = match collect_all_session_texts(project_name) {
        Ok(t) => t,
        Err(e) => {
            debug!("Summarization failed: could not collect sessions: {}", e);
            if let Some(tx) = event_tx {
                let _ = tx.send(AgentEvent::Token(format!(
                    "Summarization failed: could not read session files: {}\n",
                    e
                )));
                let _ = tx.send(AgentEvent::Error(format!("Summarization failed: {}", e)));
            }
            return;
        }
    };

    if session_texts.trim().is_empty() {
        debug!("Summarization skipped: no session texts to summarize");
        if let Some(tx) = event_tx {
            let _ = tx.send(AgentEvent::Token(
                "No session data to summarize.\n".to_string(),
            ));
            let _ = tx.send(AgentEvent::Done);
        }
        return;
    }

    if let Some(tx) = &event_tx {
        let _ = tx.send(AgentEvent::Token(
            "Collecting session data for summarization...\n".to_string(),
        ));
    }

    // 2. Load existing memory
    let existing_memory = load_memory_md(project_name);

    // 3. Build messages for the API
    let mut api_messages: Vec<api::ChatMessage> = Vec::new();

    // System prompt
    api_messages.push(api::ChatMessage {
        role: "system".to_string(),
        content: Some(SUMMARIZE_SYSTEM_PROMPT.to_string()),
        tool_calls: None,
        tool_call_id: None,
    });

    // Session data
    let mut user_prompt = format!(
        "Here are the conversation transcripts from past sessions. \
         Summarize them into a long-term memory document.\n\n{}",
        session_texts
    );
    if user_prompt.len() > 120_000 {
        user_prompt.truncate(120_000);
        user_prompt.push_str("\n\n[... content truncated due to length ...]");
    }
    api_messages.push(api::ChatMessage {
        role: "user".to_string(),
        content: Some(user_prompt),
        tool_calls: None,
        tool_call_id: None,
    });

    // Existing memory as additional context
    if let Some(ref existing) = existing_memory {
        let truncated: String = if existing.len() > 50_000 {
            format!("{}...\n\n[... existing memory truncated ...]", &existing[..50_000])
        } else {
            existing.clone()
        };
        api_messages.push(api::ChatMessage {
            role: "user".to_string(),
            content: Some(format!(
                "Here is the existing long-term memory for this project. \
                 Integrate any new knowledge from the session transcripts above. \
                 Update existing topics if new information is available. \
                 Add new topics for entirely new subjects.\n\n{}",
                truncated
            )),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    if let Some(tx) = &event_tx {
        let _ = tx.send(AgentEvent::Token(
            "Calling API to generate long-term memory summary...\n".to_string(),
        ));
    }

    // 4. Call the API
    let summary = match api::chat_sync(config, &api_messages, 8192).await {
        Ok(s) => s,
        Err(e) => {
            debug!("Summarization API call failed: {}", e);
            if let Some(tx) = event_tx {
                let _ = tx.send(AgentEvent::Token(format!(
                    "Summarization API call failed: {}\n",
                    e
                )));
                let _ = tx.send(AgentEvent::Error(format!("Summarization failed: {}", e)));
            }
            return;
        }
    };

    if let Some(tx) = &event_tx {
        let _ = tx.send(AgentEvent::Token(
            "Summary generated. Writing memory.md...\n".to_string(),
        ));
    }

    // 5. Format and save
    let now = crate::session::now_readable();
    let header = format!(
        "# Long-Term Memory: {}\n\
         # Generated: {}\n\
         #\n\
         # This file is automatically generated. It summarizes accumulated knowledge\n\
         # from past sessions into topic-based knowledge blocks.\n\n\
         ---\n\n",
        project_name, now
    );
    let full_content = format!("{}{}", header, summary.trim());

    if let Err(e) = save_memory_md(project_name, &full_content) {
        debug!("Failed to save memory.md: {}", e);
        if let Some(tx) = event_tx {
            let _ = tx.send(AgentEvent::Token(format!(
                "Failed to save memory.md: {}\n",
                e
            )));
            let _ = tx.send(AgentEvent::Error(format!("Failed to save memory: {}", e)));
        }
        return;
    }

    // 6. Enforce size limit
    let md_path = memory_md_path(project_name);
    let _ = enforce_size_limit(&md_path, MAX_MEMORY_BYTES);

    // 7. Delete session files (only after successful save)
    let deleted = match delete_all_session_files(project_name) {
        Ok(n) => n,
        Err(e) => {
            debug!("Failed to delete some session files: {}", e);
            0
        }
    };

    let done_msg = format!(
        "Long-term memory updated. Deleted {} session file(s).\n",
        deleted
    );
    debug!("{}", done_msg.trim());
    if let Some(tx) = event_tx {
        let _ = tx.send(AgentEvent::Token(done_msg));
        let _ = tx.send(AgentEvent::Done);
    }
}

fn delete_all_session_files(project_name: &str) -> Result<usize, String> {
    let dir = project_sessions_dir(project_name);
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(&dir).map_err(|e| format!("Failed to read sessions dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete session file: {}", e))?;
            count += 1;
        }
    }
    debug!("Deleted {} session files for project {}", count, project_name);
    Ok(count)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords_filters_short_words() {
        let kw = extract_keywords("a an file read path");
        assert!(kw.contains(&"file".to_string()));
        assert!(kw.contains(&"read".to_string()));
        assert!(kw.contains(&"path".to_string()));
        assert!(!kw.contains(&"a".to_string()));
        assert!(!kw.contains(&"an".to_string()));
        // "the" is 3 chars so it passes, verify it's included
    }

    #[test]
    fn test_extract_keywords_deduplicates() {
        let kw = extract_keywords("file file FILE Read read");
        let file_count = kw.iter().filter(|w| *w == "file").count();
        let read_count = kw.iter().filter(|w| *w == "read").count();
        assert_eq!(file_count, 1);
        assert_eq!(read_count, 1);
    }

    #[test]
    fn test_parse_topic_blocks_basic() {
        let md = "\
# Header stuff
---

## Topic: Auth System
**Last Updated**: 2026-01-01
**Source Sessions**: 3

We decided to use JWT tokens for authentication.

### Cross-References
- [Topic: API Design](#topic-api-design) -- related

---

## Topic: API Design
**Last Updated**: 2026-02-01
**Source Sessions**: 2

RESTful API with versioning.

### Cross-References
- [Topic: Auth System](#topic-auth-system) -- related
";
        let blocks = parse_topic_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].title, "Auth System");
        assert_eq!(blocks[1].title, "API Design");
        assert!(blocks[0].content.contains("JWT tokens"));
        assert!(blocks[1].content.contains("RESTful API"));
    }

    #[test]
    fn test_parse_topic_blocks_empty() {
        let blocks = parse_topic_blocks("Just some text\nNo topic headers\n");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_score_relevance_title_match() {
        let block = TopicBlock {
            title: "Authentication System Design".to_string(),
            content: "Some content about other things".to_string(),
            start_byte: 0,
            end_byte: 100,
        };
        let kw = vec!["authentication".to_string()];
        let score = score_relevance(&block, &kw);
        assert!(score > 0.5); // Title match is 2x weighted
    }

    #[test]
    fn test_score_relevance_no_match() {
        let block = TopicBlock {
            title: "Database Schema".to_string(),
            content: "We use PostgreSQL with migrations".to_string(),
            start_byte: 0,
            end_byte: 100,
        };
        let kw = vec!["frontend".to_string(), "react".to_string()];
        let score = score_relevance(&block, &kw);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_score_relevance_content_match() {
        let block = TopicBlock {
            title: "Database Schema".to_string(),
            content: "We decided to use PostgreSQL for authentication storage".to_string(),
            start_byte: 0,
            end_byte: 100,
        };
        let kw = vec!["postgresql".to_string()];
        let score = score_relevance(&block, &kw);
        assert_eq!(score, 0.5); // Content match is 1x weighted, 1/(1*2) = 0.5
    }

    #[test]
    fn test_search_memory_no_file() {
        let result = search_memory("__nonexistent_project__", "test query");
        assert!(result.is_none());
    }

    #[test]
    fn test_enforce_size_limit_below_threshold() {
        let dir = std::env::temp_dir().join("zero-test-size-limit");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.md");
        let content = "## Topic: Small\nSome content\n";
        fs::write(&path, content).unwrap();

        let result = enforce_size_limit(&path, MAX_MEMORY_BYTES);
        assert!(result.is_ok());

        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(after, content); // unchanged

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_enforce_size_limit_truncates() {
        let dir = std::env::temp_dir().join("zero-test-size-truncate");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.md");

        // Create content with 3 topic blocks
        let mut content = String::new();
        content.push_str("# Header\n---\n\n");
        content.push_str("## Topic: First\n**Last Updated**: 2020-01-01\n**Source Sessions**: 1\n\nOldest content here.\n\n### Cross-References\n\n---\n\n");
        content.push_str("## Topic: Second\n**Last Updated**: 2021-01-01\n**Source Sessions**: 2\n\nMiddle content here.\n\n### Cross-References\n\n---\n\n");
        content.push_str("## Topic: Third\n**Last Updated**: 2022-01-01\n**Source Sessions**: 3\n\nNewest content here.\n\n### Cross-References\n\n");
        let total_size = content.len() as u64;
        fs::write(&path, &content).unwrap();

        // Reduce to limit that removes oldest 2 of 3 blocks
        let max_bytes = total_size * 3 / 4 + 50;
        let result = enforce_size_limit(&path, max_bytes);
        assert!(result.is_ok());

        let after = fs::read_to_string(&path).unwrap();
        let after_size = after.len() as u64;
        assert!(after.contains("WARNING"), "Should have truncation notice");
        assert!(after_size <= max_bytes, "{} <= {}", after_size, max_bytes);
        assert!(!after.contains("## Topic: First"), "Oldest block should be removed");
        assert!(after.contains("## Topic:"), "Should have at least one remaining topic");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_check_expired_sessions_no_dir() {
        let (any, files) = check_expired_sessions("__nonexistent_project_check__", 7);
        assert!(!any);
        assert!(files.is_empty());
    }
}
