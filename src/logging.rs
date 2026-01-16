use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;

fn log_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("lazypost").join("error.log"))
}

pub fn log_error(context: &str, error: &str) {
    if let Some(path) = log_path() {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] {}: {}", timestamp, context, error);
        }
    }
}
