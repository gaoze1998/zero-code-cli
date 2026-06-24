use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

static LOG_WRITER: Mutex<Option<BufWriter<File>>> = Mutex::new(None);

/// Initialize the debug log file. Truncates on each run.
pub fn init(log_path: &Path) -> io::Result<()> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(log_path)?;
    let mut writer = LOG_WRITER.lock().unwrap();
    *writer = Some(BufWriter::new(file));
    Ok(())
}

/// Write a debug entry to the log file. Flushes immediately.
pub fn write_debug(file: &str, line: u32, msg: &str) {
    if let Ok(mut guard) = LOG_WRITER.lock()
        && let Some(ref mut w) = *guard
    {
        let _ = writeln!(w, "[DEBUG] {}:{} {}", file, line, msg);
        let _ = w.flush();
    }
}

/// Debug macro — drop-in replacement for `eprintln!("[DEBUG] ...")`.
/// Writes to the log file instead of stderr.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::logger::write_debug(file!(), line!(), &format!($($arg)*))
    };
}
