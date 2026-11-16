//! 영구 누적 로그 (system_accumulated.log).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;

use crate::config::LOGS_ROOT;

pub fn log_file() -> PathBuf {
    PathBuf::from(LOGS_ROOT).join("system_accumulated.log")
}

pub fn append(level: &str, message: &str) -> String {
    let entry = format!(
        "[{}] [{:<7}] {}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level,
        message
    );
    let path = log_file();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{entry}");
    }
    entry
}

/// 최근 max줄을 읽는다 (UTF-8 실패 시 CP949로 해석).
pub fn read_recent(max: usize) -> Vec<String> {
    let bytes = match fs::read(log_file()) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let text = crate::batch::decode_text(&bytes);
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let start = lines.len().saturating_sub(max);
    lines[start..].to_vec()
}
