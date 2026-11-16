//! userset.json 설정 (Python 버전과 같은 스키마를 사용한다).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const BASE_PMI_DIR: &str = r"C:\checkPMI";
pub const LOGS_ROOT: &str = r"C:\checkPMI\logs";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SystemCfg {
    pub interval_sec: u64,
    pub sound_alert_enabled: bool,
    pub beep_freq_hz: u32,
    pub beep_duration_ms: u32,
    pub time_sync_cmd: String,
}

impl Default for SystemCfg {
    fn default() -> Self {
        Self {
            interval_sec: 2,
            sound_alert_enabled: true,
            beep_freq_hz: 1200,
            beep_duration_ms: 350,
            time_sync_cmd: "w32tm /resync /force".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Thresholds {
    pub cpu_percent: f64,
    pub ram_percent: f64,
    pub disk_percent: f64,
    pub disk_drive: String,
    pub proc_count_limit: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            cpu_percent: 85.0,
            ram_percent: 85.0,
            disk_percent: 90.0,
            disk_drive: "C:\\".to_string(),
            proc_count_limit: 300,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BatPaths {
    pub daily_fas: String,
    pub daily_eai: String,
    pub monthly: String,
    pub annual: String,
}

impl Default for BatPaths {
    fn default() -> Self {
        Self {
            daily_fas: format!(r"{}\F100-2-001.bat", BASE_PMI_DIR),
            daily_eai: format!(r"{}\F100-1-001-DETAIL.bat", BASE_PMI_DIR),
            monthly: format!(r"{}\F100-3-030.bat", BASE_PMI_DIR),
            annual: format!(r"{}\F100-4-360.bat", BASE_PMI_DIR),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub system: SystemCfg,
    pub thresholds: Thresholds,
    pub target_processes: Vec<String>,
    pub bat_paths: BatPaths,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system: SystemCfg::default(),
            thresholds: Thresholds::default(),
            target_processes: vec![
                "Gms.Application.exe".into(),
                "FasServer.exe".into(),
                "SMC.exe".into(),
                "bgw.exe".into(),
                "sqlservr.exe".into(),
            ],
            bat_paths: BatPaths::default(),
        }
    }
}

/// exe와 같은 폴더의 userset.json.
pub fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("userset.json")))
        .unwrap_or_else(|| PathBuf::from("userset.json"))
}

/// 파일이 없으면 기본값으로 만들고, 읽기에 실패하면 기본값을 쓴다.
pub fn load() -> Config {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => {
            let cfg = Config::default();
            let _ = save(&cfg);
            cfg
        }
    }
}

pub fn save(cfg: &Config) -> Result<(), String> {
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(config_path(), text).map_err(|e| e.to_string())
}
