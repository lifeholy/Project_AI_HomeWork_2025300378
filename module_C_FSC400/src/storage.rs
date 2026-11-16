//! 일별 CSV 기록, 과거 기록 읽기, 설정 파일 저장.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::protocol::Sample;

pub const CSV_HEADER: [&str; 6] = [
    "Timestamp",
    "Header",
    "Flow_Rate(m3/h)",
    "Total_Flow(m3)",
    "Velocity(m/s)",
    "Error_Code",
];
const TIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

/// exe가 있는 폴더 (개발 중에는 target 폴더).
pub fn base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn log_path(base: &Path, day: NaiveDate) -> PathBuf {
    base.join("data").join(format!("flow_log_{}.csv", day.format("%Y-%m-%d")))
}

/// 하루 단위 파일을 열어 두고 샘플을 한 줄씩 쓴다.
pub struct Recorder {
    base: PathBuf,
    current: Option<(NaiveDate, csv::Writer<File>)>,
}

impl Recorder {
    pub fn new(base: PathBuf) -> Self {
        Self { base, current: None }
    }

    pub fn write(&mut self, s: &Sample) -> io::Result<()> {
        let day = s.timestamp.date_naive();
        let need_open = !matches!(&self.current, Some((d, _)) if *d == day);
        if need_open {
            self.current = None;
            let path = log_path(&self.base, day);
            fs::create_dir_all(path.parent().unwrap_or(&self.base))?;
            let is_new = !path.exists();
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            let mut writer = csv::Writer::from_writer(file);
            if is_new {
                writer.write_record(CSV_HEADER)?;
            }
            self.current = Some((day, writer));
        }
        if let Some((_, writer)) = self.current.as_mut() {
            writer.write_record([
                s.timestamp.format(TIME_FMT).to_string(),
                s.header.clone(),
                format!("{:.3}", s.flow),
                format!("{:.3}", s.total),
                format!("{:.2}", s.velocity),
                s.error.clone(),
            ])?;
            writer.flush()?;
        }
        Ok(())
    }
}

/// 날짜 파일을 읽어 시간순 샘플 목록을 돌려준다. 잘못된 행은 건너뛴다.
pub fn read_day(path: &Path) -> Result<Vec<Sample>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|e| e.to_string())?;
    let mut samples = Vec::new();
    for rec in reader.records() {
        let Ok(rec) = rec else { continue };
        if rec.len() < 6 {
            continue;
        }
        let Ok(naive) = NaiveDateTime::parse_from_str(&rec[0], TIME_FMT) else { continue };
        let Some(ts) = naive.and_local_timezone(Local).earliest() else { continue };
        let (Ok(flow), Ok(total), Ok(velocity)) =
            (rec[2].parse::<f64>(), rec[3].parse::<f64>(), rec[4].parse::<f64>())
        else {
            continue;
        };
        samples.push(Sample {
            timestamp: ts,
            header: rec[1].to_string(),
            flow,
            total,
            velocity,
            error: rec[5].to_string(),
        });
    }
    samples.sort_by_key(|s| s.timestamp);
    Ok(samples)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub source: String,
    pub port: String,
    pub baud: u32,
    pub slave: u8,
    pub interval: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source: "MODBUS-RTU".to_string(),
            port: "COM3".to_string(),
            baud: 9600,
            slave: 1,
            interval: 1.0,
        }
    }
}

pub fn load_settings(base: &Path) -> Settings {
    fs::read_to_string(base.join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_settings(base: &Path, s: &Settings) {
    if let Ok(text) = serde_json::to_string_pretty(s) {
        let _ = fs::write(base.join("settings.json"), text);
    }
}

/// 차트 X축용 epoch 초 (밀리초 정밀도).
pub fn epoch_secs(dt: &DateTime<Local>) -> f64 {
    dt.timestamp_millis() as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn record_then_read_back() {
        let dir = std::env::temp_dir().join(format!("fsc_rs_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut rec = Recorder::new(dir.clone());
        let start = Local.with_ymd_and_hms(2026, 1, 2, 10, 0, 0).unwrap();
        for i in 0..5 {
            let s = Sample {
                timestamp: start + Duration::seconds(i),
                header: "SIM".into(),
                flow: 20.0 + i as f64,
                total: 100.0,
                velocity: 1.5,
                error: "0000000000".into(),
            };
            rec.write(&s).unwrap();
        }
        drop(rec);
        let back = read_day(&log_path(&dir, start.date_naive())).unwrap();
        assert_eq!(back.len(), 5);
        assert_eq!(back[4].flow, 24.0);
        let _ = fs::remove_dir_all(&dir);
    }
}
