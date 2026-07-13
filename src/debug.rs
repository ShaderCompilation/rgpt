use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

/// When enabled via `--debug`, records every request/response exchanged with
/// the model backend to a timestamped log file in the current directory:
/// the exact outgoing request body, every raw line received back (so
/// reasoning/thinking content is visible even though the app doesn't parse
/// it), and the completion the app derived from it. Meant for after-the-fact
/// inspection of a session, not for normal use.
pub struct DebugLog {
    file: Mutex<File>,
    start: Instant,
}

impl DebugLog {
    pub fn new() -> Result<Self> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = std::env::current_dir()
            .unwrap_or_default()
            .join(format!("rgpt-debug-{ts}.log"));
        let file = File::create(&path)
            .with_context(|| format!("creating debug log file {}", path.display()))?;
        eprintln!("debug: logging to {}", path.display());
        Ok(Self {
            file: Mutex::new(file),
            start: Instant::now(),
        })
    }

    fn write(&self, text: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = write!(file, "{text}");
            let _ = file.flush();
        }
    }

    /// A titled, multi-line block (request bodies, parsed completions, errors).
    pub fn section(&self, title: &str, body: &str) {
        self.write(&format!(
            "[+{:>8.3}s] === {title} ===\n{body}\n\n",
            self.start.elapsed().as_secs_f64()
        ));
    }

    /// A single raw line as it came off the wire (one SSE/NDJSON chunk).
    pub fn line(&self, title: &str, body: &str) {
        self.write(&format!(
            "[+{:>8.3}s] {title}: {body}\n",
            self.start.elapsed().as_secs_f64()
        ));
    }
}
