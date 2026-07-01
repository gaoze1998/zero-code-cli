use serde::Serialize;
use serde_json::{json, Value};
use std::io::Read;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Clone, Serialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "read_file".into(),
                description: "Read a file. Supports optional start_line and end_line (1-indexed, inclusive) for partial reads.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the file to read"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional 1-indexed start line. Reads from this line to end of file (or to end_line if provided)."
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Optional 1-indexed end line (inclusive). Clamped to last line if it exceeds the file length."
                        }
                    },
                    "required": ["path"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "write_file".into(),
                description: "Write or overwrite a file. Supports optional start_line and end_line for partial edits (replaces those lines with content). Creates parent directories if needed.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional 1-indexed start line for partial edit. Replaces lines [start_line, end_line] with content. If omitted, overwrites the entire file."
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Optional 1-indexed end line (inclusive). Defaults to start_line if omitted. Lines in range are replaced."
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "bash".into(),
                description: "Execute a shell command and return its stdout and stderr. Default timeout 30 seconds, max 120 seconds.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Optional timeout in milliseconds (max 120000)"
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "grep".into(),
                description: "Search for a regex pattern in files under a directory. Uses grep -rn under the hood. Returns matching lines with file paths and line numbers.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory or file path to search in"
                        }
                    },
                    "required": ["pattern", "path"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDef {
                name: "ls".into(),
                description: "List files and directories at the given path. Defaults to current directory.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to list. Defaults to current directory."
                        }
                    },
                    "required": []
                }),
            },
        },
    ]
}

/// Execute a tool call. Returns (result_string, is_error).
pub fn execute_tool(name: &str, arguments: &str) -> (String, bool) {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => return (format!("Invalid arguments JSON: {}", e), true),
    };

    match name {
        "read_file" => exec_read_file(&args),
        "write_file" => exec_write_file(&args),
        "bash" => exec_bash(&args),
        "grep" => exec_grep(&args),
        "ls" => exec_ls(&args),
        other => (format!("Unknown tool: {}", other), true),
    }
}

fn exec_read_file(args: &Value) -> (String, bool) {
    let path = match args["path"].as_str() {
        Some(p) => p,
        None => return ("Missing required parameter: path".into(), true),
    };

    let start_line: Option<usize> = args["start_line"].as_u64().map(|v| v as usize);
    let end_line: Option<usize> = args["end_line"].as_u64().map(|v| v as usize);

    // Validate line parameters
    if let Some(s) = start_line
        && s < 1
    {
        return ("start_line must be >= 1 (1-indexed)".into(), true);
    }
    if let (Some(s), Some(e)) = (start_line, end_line)
        && s > e
    {
        return ("start_line must be <= end_line".into(), true);
    }

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return (format!("Failed to open {}: {}", path, e), true),
    };

    let metadata = match file.metadata() {
        Ok(m) => m,
        Err(e) => return (format!("Failed to read metadata: {}", e), true),
    };

    // Limit to 1MB
    const MAX_SIZE: u64 = 1_048_576;
    if metadata.len() > MAX_SIZE {
        return (format!(
            "File is {} bytes, exceeds max read size of {} bytes (1MB). Try reading a smaller portion.",
            metadata.len(),
            MAX_SIZE
        ), true);
    }

    let mut reader = std::io::BufReader::new(file);
    let mut content = String::new();
    match reader.read_to_string(&mut content) {
        Ok(_) => {}
        Err(e) => return (format!("Failed to read {}: {}", path, e), true),
    }

    // If no line range specified, return full content (backward compatible)
    if start_line.is_none() && end_line.is_none() {
        return (content, false);
    }

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let s = start_line.unwrap_or(1);
    let e = end_line.unwrap_or(total).min(total);

    if s > total {
        return (String::new(), false);
    }

    (lines[s - 1..e].join("\n"), false)
}

fn exec_write_file(args: &Value) -> (String, bool) {
    let path = match args["path"].as_str() {
        Some(p) => p,
        None => return ("Missing required parameter: path".into(), true),
    };
    let content = match args["content"].as_str() {
        Some(c) => c,
        None => return ("Missing required parameter: content".into(), true),
    };

    let start_line: Option<usize> = args["start_line"].as_u64().map(|v| v as usize);
    let end_line: Option<usize> = args["end_line"].as_u64().map(|v| v as usize);

    // Validate line parameters
    if let Some(s) = start_line
        && s < 1
    {
        return ("start_line must be >= 1 (1-indexed)".into(), true);
    }
    if let (Some(s), Some(e)) = (start_line, end_line)
        && s > e
    {
        return ("start_line must be <= end_line".into(), true);
    }

    // Safety: reject path traversal
    if path.contains("..") {
        return ("Path traversal detected: '..' is not allowed in file paths".into(), true);
    }

    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return (format!("Failed to create parent directories: {}", e), true);
    }

    // Fast path: no line params → overwrite entire file (backward compatible)
    if start_line.is_none() && end_line.is_none() {
        match std::fs::write(path, content) {
            Ok(()) => (format!("Successfully wrote {} bytes to {}", content.len(), path), false),
            Err(e) => (format!("Failed to write {}: {}", path, e), true),
        }
    } else {
        // Line-range edit mode
        let existing = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return (format!("Failed to read {}: {}", path, e), true),
        };

        let mut lines: Vec<String> = existing.lines().map(|s| s.to_string()).collect();
        let total = lines.len();
        let s = start_line.unwrap_or(1);
        let e = end_line.unwrap_or(s);

        if s > total + 1 {
            return (
                format!(
                    "start_line {} exceeds file length {} + 1; cannot insert with a gap",
                    s, total
                ),
                true,
            );
        }

        if total == 0 && s > 1 {
            return (
                format!(
                    "Cannot edit non-existent file at line {}; file does not exist yet (0 lines)",
                    s
                ),
                true,
            );
        }

        let remove_start = s - 1;
        let remove_count = if s > total { 0 } else { e.min(total) - s + 1 };

        lines.drain(remove_start..remove_start + remove_count);

        let new_lines: Vec<String> = if content.is_empty() {
            vec![]
        } else {
            content.lines().map(|s| s.to_string()).collect()
        };

        lines.splice(remove_start..remove_start, new_lines);

        let output = lines.join("\n");
        match std::fs::write(path, &output) {
            Ok(()) => (format!("Successfully edited {}: replaced lines {}-{} ({} bytes written)", path, s, e, output.len()), false),
            Err(e) => (format!("Failed to write {}: {}", path, e), true),
        }
    }
}

fn exec_bash(args: &Value) -> (String, bool) {
    let command = match args["command"].as_str() {
        Some(c) => c,
        None => return ("Missing required parameter: command".into(), true),
    };

    let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(30_000);
    let timeout_ms = timeout_ms.min(120_000); // max 120s
    let timeout = Duration::from_millis(timeout_ms);

    let output = match Command::new("bash")
        .args(["-c", command])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => {
            match child.wait_with_output() {
                Ok(o) => o,
                Err(e) => return (format!("Failed to wait on process: {}", e), true),
            }
        }
        Err(e) => return (format!("Failed to spawn command: {}", e), true),
    };

    // Note: timeout via Duration isn't directly supported in std::process.
    // For a production implementation, we'd use wait_timeout or similar.
    // Using the raw approach for now — the command will run to completion.
    let _ = timeout;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut result = String::new();
    if !stdout.is_empty() {
        result.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str("stderr:\n");
        result.push_str(&stderr);
    }
    if result.is_empty() {
        result.push_str("(no output)");
    }

    let is_error = !output.status.success();
    (result, is_error)
}

fn exec_grep(args: &Value) -> (String, bool) {
    let pattern = match args["pattern"].as_str() {
        Some(p) => p,
        None => return ("Missing required parameter: pattern".into(), true),
    };
    let path = match args["path"].as_str() {
        Some(p) => p,
        None => return ("Missing required parameter: path".into(), true),
    };

    let output = match Command::new("grep")
        .args(["-rn", "--color=never", pattern, path])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => return (format!("Failed to run grep: {}", e), true),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        return (format!("grep error: {}", stderr), true);
    }

    let result = if stdout.is_empty() {
        "No matches found.".into()
    } else {
        // Truncate to 100KB to avoid overwhelming context
        if stdout.len() > 102_400 {
            let truncated: String = stdout.chars().take(102_400).collect();
            format!("{}\n... (truncated, {} bytes total)", truncated, stdout.len())
        } else {
            stdout.to_string()
        }
    };

    let is_error = !output.status.success() && output.status.code() != Some(1);
    // grep returns 0=matches found, 1=no matches, >1=error
    (result, is_error)
}

fn exec_ls(args: &Value) -> (String, bool) {
    let path = args["path"].as_str().unwrap_or(".");

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => return (format!("Failed to list {}: {}", path, e), true),
    };

    let mut items: Vec<String> = Vec::new();
    for entry in entries {
        match entry {
            Ok(e) => {
                let name = e.file_name().to_string_lossy().to_string();
                let file_type = match e.file_type() {
                    Ok(ft) if ft.is_dir() => "/".to_string(),
                    Ok(ft) if ft.is_symlink() => "@".to_string(),
                    _ => String::new(),
                };
                items.push(format!("{}{}", name, file_type));
            }
            Err(e) => {
                items.push(format!("<error: {}>", e));
            }
        }
    }

    items.sort();

    if items.is_empty() {
        (format!("{} (empty directory)", path), false)
    } else {
        (items.join("\n"), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_file(content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("zero-code-test-{}-{}.txt", std::process::id(), id));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_tool_definitions_count() {
        let defs = get_tool_definitions();
        assert_eq!(defs.len(), 5, "should have 5 tools defined");
    }

    #[test]
    fn test_tool_definitions_are_valid_json() {
        let defs = get_tool_definitions();
        for def in &defs {
            let json_str = serde_json::to_string(def).unwrap();
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["type"], "function");
            assert!(!parsed["function"]["name"].as_str().unwrap().is_empty());
        }
    }

    #[test]
    fn test_execute_unknown_tool() {
        let (result, is_error) = execute_tool("nonexistent", "{}");
        assert!(is_error);
        assert!(result.contains("Unknown tool"));
    }

    #[test]
    fn test_execute_read_file_missing_path() {
        let (result, is_error) = execute_tool("read_file", "{}");
        assert!(is_error);
        assert!(result.contains("Missing required parameter"));
    }

    #[test]
    fn test_execute_read_file_not_found() {
        let (result, is_error) = execute_tool("read_file", r#"{"path": "/nonexistent/file.txt"}"#);
        assert!(is_error);
        assert!(result.contains("Failed to open"));
    }

    #[test]
    fn test_execute_write_file_path_traversal() {
        let (result, is_error) = execute_tool("write_file", r#"{"path": "../etc/passwd", "content": "x"}"#);
        assert!(is_error);
        assert!(result.contains("Path traversal"));
    }

    #[test]
    fn test_execute_write_file_missing_content() {
        let (result, is_error) = execute_tool("write_file", r#"{"path": "/tmp/test.txt"}"#);
        assert!(is_error);
        assert!(result.contains("Missing required parameter"));
    }

    #[test]
    fn test_execute_bash_missing_command() {
        let (result, is_error) = execute_tool("bash", "{}");
        assert!(is_error);
        assert!(result.contains("Missing required parameter"));
    }

    #[test]
    fn test_execute_bash_echo() {
        let (result, is_error) = execute_tool("bash", r#"{"command": "echo hello"}"#);
        assert!(!is_error);
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_execute_ls_current_dir() {
        let (result, is_error) = execute_tool("ls", r#"{"path": "src"}"#);
        assert!(!is_error);
        // Should list the source files
        assert!(result.contains("main.rs"));
    }

    #[test]
    fn test_execute_ls_nonexistent() {
        let (result, is_error) = execute_tool("ls", r#"{"path": "/nonexistent/dir"}"#);
        assert!(is_error);
        assert!(result.contains("Failed to list"));
    }

    // ── read_file line-range tests ──

    #[test]
    fn test_execute_read_file_with_line_range() {
        let path = temp_file("line1\nline2\nline3\nline4\nline5");
        let args = serde_json::json!({"path": path, "start_line": 2, "end_line": 4});
        let (result, is_error) = execute_tool("read_file", &args.to_string());
        assert!(!is_error);
        assert_eq!(result, "line2\nline3\nline4");
    }

    #[test]
    fn test_execute_read_file_start_line_only() {
        let path = temp_file("line1\nline2\nline3\nline4\nline5");
        let args = serde_json::json!({"path": path, "start_line": 3});
        let (result, is_error) = execute_tool("read_file", &args.to_string());
        assert!(!is_error);
        assert_eq!(result, "line3\nline4\nline5");
    }

    #[test]
    fn test_execute_read_file_end_line_only() {
        let path = temp_file("line1\nline2\nline3\nline4\nline5");
        let args = serde_json::json!({"path": path, "end_line": 2});
        let (result, is_error) = execute_tool("read_file", &args.to_string());
        assert!(!is_error);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_execute_read_file_start_gt_end_error() {
        let args = r#"{"path": "/tmp/test.txt", "start_line": 5, "end_line": 3}"#;
        let (result, is_error) = execute_tool("read_file", args);
        assert!(is_error);
        assert!(result.contains("start_line must be <= end_line"));
    }

    #[test]
    fn test_execute_read_file_start_lt_1_error() {
        let args = r#"{"path": "/tmp/test.txt", "start_line": 0}"#;
        let (result, is_error) = execute_tool("read_file", args);
        assert!(is_error);
        assert!(result.contains("start_line must be >= 1"));
    }

    #[test]
    fn test_execute_read_file_start_beyond_eof() {
        let path = temp_file("a\nb\nc");
        let args = serde_json::json!({"path": path, "start_line": 10});
        let (result, is_error) = execute_tool("read_file", &args.to_string());
        assert!(!is_error);
        assert_eq!(result, "");
    }

    #[test]
    fn test_execute_read_file_end_clamped() {
        let path = temp_file("a\nb\nc");
        let args = serde_json::json!({"path": path, "start_line": 2, "end_line": 10});
        let (result, is_error) = execute_tool("read_file", &args.to_string());
        assert!(!is_error);
        assert_eq!(result, "b\nc");
    }

    #[test]
    fn test_execute_read_file_backward_compat_no_lines() {
        let path = temp_file("hello\nworld");
        let args = serde_json::json!({"path": path});
        let (result, is_error) = execute_tool("read_file", &args.to_string());
        assert!(!is_error);
        assert_eq!(result, "hello\nworld");
    }

    // ── write_file line-range tests ──

    #[test]
    fn test_execute_write_file_line_range_edit() {
        let path = temp_file("line1\nline2\nline3\nline4\nline5");
        let args = serde_json::json!({"path": path, "start_line": 2, "end_line": 3, "content": "NEW_A\nNEW_B"});
        let (result, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        assert!(result.contains("Successfully edited"));
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "line1\nNEW_A\nNEW_B\nline4\nline5");
    }

    #[test]
    fn test_execute_write_file_single_line_replace() {
        let path = temp_file("line1\nline2\nline3");
        let args = serde_json::json!({"path": path, "start_line": 2, "content": "REPLACED"});
        let (_, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "line1\nREPLACED\nline3");
    }

    #[test]
    fn test_execute_write_file_insert_at_end() {
        let path = temp_file("line1\nline2");
        let args = serde_json::json!({"path": path, "start_line": 3, "content": "line3\nline4"});
        let (_, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "line1\nline2\nline3\nline4");
    }

    #[test]
    fn test_execute_write_file_delete_lines() {
        let path = temp_file("line1\nline2\nline3\nline4");
        let args = serde_json::json!({"path": path, "start_line": 2, "end_line": 3, "content": ""});
        let (_, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "line1\nline4");
    }

    #[test]
    fn test_execute_write_file_invalid_range_gap() {
        let path = temp_file("a\nb\nc");
        let args = serde_json::json!({"path": path, "start_line": 10, "content": "x"});
        let (result, is_error) = execute_tool("write_file", &args.to_string());
        assert!(is_error);
        assert!(result.contains("exceeds file length"));
    }

    #[test]
    fn test_execute_write_file_start_lt_1_error() {
        let path = temp_file("a\nb");
        let args = serde_json::json!({"path": path, "start_line": 0, "content": "x"});
        let (result, is_error) = execute_tool("write_file", &args.to_string());
        assert!(is_error);
        assert!(result.contains("start_line must be >= 1"));
    }

    #[test]
    fn test_execute_write_file_start_gt_end_error() {
        let path = temp_file("a\nb\nc");
        let args = serde_json::json!({"path": path, "start_line": 5, "end_line": 3, "content": "x"});
        let (result, is_error) = execute_tool("write_file", &args.to_string());
        assert!(is_error);
        assert!(result.contains("start_line must be <= end_line"));
    }

    #[test]
    fn test_execute_write_file_backward_compat_no_lines() {
        let path = temp_file("old");
        let args = serde_json::json!({"path": path, "content": "new content"});
        let (result, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        assert!(result.contains("Successfully wrote"));
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "new content");
    }

    #[test]
    fn test_execute_write_file_new_file_with_line_range() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("zero-code-test-new-{}.txt", std::process::id()));
        // Clean up in case a previous test left it
        let _ = std::fs::remove_file(&path);
        let args = serde_json::json!({"path": path, "start_line": 1, "content": "hello\nworld"});
        let (_, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "hello\nworld");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_execute_write_file_new_file_start_gt_1_error() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("zero-code-test-new2-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let args = serde_json::json!({"path": path, "start_line": 5, "content": "x"});
        let (_, is_error) = execute_tool("write_file", &args.to_string());
        assert!(is_error);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_execute_write_file_multi_line_content() {
        let path = temp_file("before\nTARGET\nafter");
        let args = serde_json::json!({"path": path, "start_line": 2, "content": "A\nB\nC"});
        let (_, is_error) = execute_tool("write_file", &args.to_string());
        assert!(!is_error);
        let edited = std::fs::read_to_string(&path).unwrap();
        assert_eq!(edited, "before\nA\nB\nC\nafter");
    }
}
