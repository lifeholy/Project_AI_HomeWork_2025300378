//! 데이터 수신 스레드: MODBUS-RTU / ASCII 라인 / 시뮬레이션.
//! 샘플은 채널로 GUI에 전달하고, 끊기면 재연결을 반복한다.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use rand::Rng;
use serialport::{ClearBuffer, DataBits, Parity, SerialPort, StopBits};

use crate::protocol::{build_read_request, parse_response, regs_to_sample, Sample, REG_COUNT};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Info,
    Ok,
    Warn,
    Err,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceKind {
    Modbus,
    Ascii,
    Simulation,
}

#[derive(Clone, Debug)]
pub struct SourceConfig {
    pub kind: SourceKind,
    pub port: String,
    pub baud: u32,
    pub slave: u8,
    pub interval: f64,
}

pub type StatusTx = mpsc::Sender<(String, Level)>;

const MAX_FAILS: u32 = 5;

trait Source: Send {
    /// 0이면 블로킹 수신(줄 단위 장비), 0보다 크면 주기 폴링.
    fn interval(&self) -> f64;
    /// Ok(None)은 해석 불가 데이터(건너뜀), Err는 통신 오류.
    fn read(&mut self) -> Result<Option<Sample>, String>;
}

struct ModbusSource {
    port: Box<dyn SerialPort>,
    slave: u8,
    interval: f64,
}

impl Source for ModbusSource {
    fn interval(&self) -> f64 {
        self.interval
    }

    fn read(&mut self) -> Result<Option<Sample>, String> {
        let _ = self.port.clear(ClearBuffer::Input);
        let req = build_read_request(self.slave, 0, REG_COUNT);
        self.port.write_all(&req).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 3 + 2 * REG_COUNT as usize + 2];
        self.port.read_exact(&mut buf).map_err(|e| e.to_string())?;
        let regs = parse_response(&buf, self.slave, REG_COUNT)?;
        Ok(Some(regs_to_sample(&regs)))
    }
}

struct AsciiSource {
    port: Box<dyn SerialPort>,
}

impl Source for AsciiSource {
    fn interval(&self) -> f64 {
        0.0
    }

    fn read(&mut self) -> Result<Option<Sample>, String> {
        let mut line: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match self.port.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    line.push(byte[0]);
                    if line.len() > 1024 {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        let text: String = String::from_utf8_lossy(&line).chars().filter(|c| c.is_ascii()).collect();
        Ok(parse_ascii_line(text.trim()))
    }
}

/// "헤더,유량,적산,에러,유속" 한 줄 해석 (유속은 생략 가능).
pub fn parse_ascii_line(line: &str) -> Option<Sample> {
    let parts: Vec<&str> = line.split(',').map(|p| p.trim()).collect();
    if parts.len() < 4 {
        return None;
    }
    let flow = parts[1].replace('+', "").trim().parse::<f64>().ok()?;
    let total = parts[2].replace('+', "").trim().parse::<f64>().ok()?;
    let velocity = if parts.len() > 4 {
        parts[4].parse::<f64>().ok()?
    } else {
        0.0
    };
    Some(Sample {
        timestamp: Local::now(),
        header: parts[0].to_string(),
        flow,
        total,
        velocity,
        error: parts[3].to_string(),
    })
}

struct SimulationSource {
    interval: f64,
    flow: f64,
    total: f64,
}

impl Source for SimulationSource {
    fn interval(&self) -> f64 {
        self.interval
    }

    fn read(&mut self) -> Result<Option<Sample>, String> {
        let mut rng = rand::thread_rng();
        self.flow = (self.flow + rng.gen_range(-1.5..1.5)).clamp(4.0, 48.0);
        self.total += self.flow / 3600.0;
        let velocity = self.flow / 12.8 + rng.gen_range(-0.02..0.02);
        let error = if rng.gen::<f64>() > 0.99 { "1100000000" } else { "0000000000" };
        Ok(Some(Sample {
            timestamp: Local::now(),
            header: "SIM_FSC400".to_string(),
            flow: round(self.flow, 3),
            total: round(self.total, 3),
            velocity: round(velocity, 2),
            error: error.to_string(),
        }))
    }
}

fn round(v: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (v * m).round() / m
}

fn open(cfg: &SourceConfig) -> Result<Box<dyn Source>, String> {
    match cfg.kind {
        SourceKind::Modbus => {
            let port = serialport::new(&cfg.port, cfg.baud)
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .timeout(Duration::from_millis(500))
                .open()
                .map_err(|e| e.to_string())?;
            Ok(Box::new(ModbusSource { port, slave: cfg.slave, interval: cfg.interval }))
        }
        SourceKind::Ascii => {
            let port = serialport::new(&cfg.port, cfg.baud)
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .timeout(Duration::from_millis(1500))
                .open()
                .map_err(|e| e.to_string())?;
            Ok(Box::new(AsciiSource { port }))
        }
        SourceKind::Simulation => Ok(Box::new(SimulationSource {
            interval: cfg.interval,
            flow: 22.8,
            total: 125.45,
        })),
    }
}

/// 짧은 간격으로 중지 플래그를 확인하며 대기한다.
fn wait(stop: &AtomicBool, secs: f64) {
    let end = Instant::now() + Duration::from_secs_f64(secs.max(0.0));
    while Instant::now() < end && !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn spawn(cfg: SourceConfig, tx: mpsc::Sender<Sample>, status: StatusTx, stop: Arc<AtomicBool>) {
    thread::spawn(move || run(cfg, tx, status, stop));
}

fn run(cfg: SourceConfig, tx: mpsc::Sender<Sample>, status: StatusTx, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        let mut src = match open(&cfg) {
            Ok(s) => s,
            Err(e) => {
                let _ = status.send((format!("연결 실패: {e}"), Level::Err));
                wait(&stop, 3.0);
                continue;
            }
        };
        let _ = status.send((format!("수신 중 ({})", kind_label(cfg.kind)), Level::Ok));
        let mut fails = 0u32;
        while !stop.load(Ordering::SeqCst) {
            let t0 = Instant::now();
            match src.read() {
                Ok(Some(sample)) => {
                    fails = 0;
                    let _ = tx.send(sample);
                }
                Ok(None) => fails = 0,
                Err(e) => {
                    fails += 1;
                    let _ = status.send((format!("수신 오류 ({fails}/{MAX_FAILS}): {e}"), Level::Warn));
                    if fails >= MAX_FAILS {
                        break;
                    }
                }
            }
            let interval = src.interval();
            if interval > 0.0 {
                wait(&stop, interval - t0.elapsed().as_secs_f64());
            }
        }
        if !stop.load(Ordering::SeqCst) {
            let _ = status.send(("연결 끊김, 재연결 대기 중".to_string(), Level::Err));
            wait(&stop, 3.0);
        }
    }
}

fn kind_label(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Modbus => "MODBUS-RTU",
        SourceKind::Ascii => "ASCII 라인",
        SourceKind::Simulation => "시뮬레이션",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_line_with_velocity() {
        let s = parse_ascii_line("DEV1,+12.5,+300.25,0000000000,1.75").unwrap();
        assert_eq!(s.header, "DEV1");
        assert_eq!(s.flow, 12.5);
        assert_eq!(s.total, 300.25);
        assert_eq!(s.velocity, 1.75);
    }

    #[test]
    fn ascii_line_without_velocity() {
        let s = parse_ascii_line("DEV1,1.0,2.0,0000").unwrap();
        assert_eq!(s.velocity, 0.0);
    }

    #[test]
    fn ascii_line_rejects_garbage() {
        assert!(parse_ascii_line("hello").is_none());
        assert!(parse_ascii_line("a,x,y,z").is_none());
    }
}
