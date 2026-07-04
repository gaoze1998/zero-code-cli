use serde_json::Value;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

pub fn exec_bash(args: &Value) -> (String, bool) {
    let command = match args["command"].as_str() {
        Some(c) => c,
        None => return ("Missing required parameter: command".into(), true),
    };

    let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(30_000);
    let timeout_ms = timeout_ms.min(120_000); // max 120s
    let timeout = Duration::from_millis(timeout_ms);

    let child = match Command::new("cmd")
        .args(["/C", command])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (format!("Failed to spawn command: {}", e), true),
    };

    let pid = child.id();

    // Wait for the child on a separate thread so we can enforce a timeout
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let output = match rx.recv_timeout(timeout) {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return (format!("Failed to wait on process: {}", e), true),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Kill the whole process tree (cmd + any children it spawned)
            let pid_str = pid.to_string();
            let _ = Command::new("taskkill")
                .args(["/PID", &pid_str, "/T", "/F"])
                .output();
            let _ = rx.recv_timeout(Duration::from_secs(3));
            return (format!("Command timed out after {}ms", timeout_ms), true);
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            return ("Process wait thread panicked".into(), true);
        }
    };

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

pub fn exec_grep(args: &Value) -> (String, bool) {
    let pattern = match args["pattern"].as_str() {
        Some(p) => p,
        None => return ("Missing required parameter: pattern".into(), true),
    };
    let path = match args["path"].as_str() {
        Some(p) => p,
        None => return ("Missing required parameter: path".into(), true),
    };

    // ripgrep (rg) is used on Windows. By default rg respects .gitignore and
    // skips hidden files, which is the desired behavior for code search.
    // Exit codes mirror grep: 0=matches found, 1=no matches, 2=error.
    let output = match Command::new("rg")
        .args(["-n", "--no-heading", "--color", "never", pattern, path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => return (format!("Failed to run rg: {}", e), true),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        return (format!("rg error: {}", stderr), true);
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
    // rg returns 0=matches found, 1=no matches, 2=error
    (result, is_error)
}
