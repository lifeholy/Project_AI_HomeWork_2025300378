//! 점검 배치(.bat) 기본 스크립트 생성, 비동기 실행, 결과 파일 읽기.

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

use chrono::NaiveDate;

use crate::config::{BASE_PMI_DIR, LOGS_ROOT};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// (파일 이름, 내용). Python 원본 BUILTIN_BAT_SCRIPTS와 동일한 내용.
const DEFAULT_BATS: [(&str, &str); 2] = [
    ("F100-2-001.bat", include_str!("bat/F100-2-001.bat")),
    ("F100-1-001-DETAIL.bat", include_str!("bat/F100-1-001-DETAIL.bat")),
];

pub enum BatMsg {
    Line(String),
    Done(bool),
}

/// 한글 텍스트 디코딩: UTF-8을 먼저 시도하고, 실패하면 CP949(EUC-KR 계열)로 읽는다.
pub fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    }
}

pub fn ensure_default_bats() {
    let dir = Path::new(BASE_PMI_DIR);
    let _ = fs::create_dir_all(dir);
    for (name, content) in DEFAULT_BATS {
        let path = dir.join(name);
        if !path.exists() {
            let text = format!("{}\n", content.trim());
            let (bytes, _, _) = encoding_rs::EUC_KR.encode(&text);
            let _ = fs::write(path, bytes);
        }
    }
}

/// 배치를 별도 스레드에서 실행하고 출력 줄을 채널로 보낸다.
pub fn spawn(path: PathBuf, tx: Sender<BatMsg>) {
    thread::spawn(move || {
        if !path.exists() {
            let _ = tx.send(BatMsg::Line(format!("[오류] 배치 파일이 존재하지 않습니다: {}", path.display())));
            let _ = tx.send(BatMsg::Done(false));
            return;
        }
        let _ = tx.send(BatMsg::Line(format!("[*] 점검 시작: {}", path.display())));
        let mut cmd = Command::new("cmd.exe");
        cmd.arg("/c")
            .arg(&path)
            .current_dir(path.parent().unwrap_or(Path::new(".")))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(BatMsg::Line(format!("[실행 오류] {e}")));
                let _ = tx.send(BatMsg::Done(false));
                return;
            }
        };
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            readers.push(pump(out, tx.clone()));
        }
        if let Some(err) = child.stderr.take() {
            readers.push(pump(err, tx.clone()));
        }
        for r in readers {
            let _ = r.join();
        }
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        let _ = tx.send(BatMsg::Done(ok));
    });
}

fn pump<R: Read + Send + 'static>(stream: R, tx: Sender<BatMsg>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf: Vec<u8> = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = decode_text(&buf).trim().to_string();
                    if !line.is_empty() && tx.send(BatMsg::Line(line)).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// 날짜(YYYYMMDD)로 시작하는 실행 결과 폴더 이름을 최신순으로 돌려준다.
pub fn list_runs(day: NaiveDate) -> Vec<String> {
    let prefix = day.format("%Y%m%d").to_string();
    let mut names: Vec<String> = fs::read_dir(LOGS_ROOT)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with(&prefix))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.reverse();
    names
}

pub fn run_dir(name: &str) -> PathBuf {
    PathBuf::from(LOGS_ROOT).join(name)
}

pub fn read_result_text(name: &str) -> String {
    match fs::read(run_dir(name).join("check_result.txt")) {
        Ok(bytes) => decode_text(&bytes),
        Err(_) => format!("check_result.txt 없음: {}", run_dir(name).display()),
    }
}

/// CSV 결과를 행 목록으로 읽는다 (첫 행은 헤더라 제외).
pub fn read_result_rows(name: &str) -> Vec<Vec<String>> {
    let Ok(bytes) = fs::read(run_dir(name).join("check_result.csv")) else {
        return Vec::new();
    };
    let text = decode_text(&bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    reader
        .records()
        .filter_map(|r| r.ok())
        .skip(1)
        .map(|r| r.iter().map(|s| s.trim().to_string()).collect())
        .collect()
}
