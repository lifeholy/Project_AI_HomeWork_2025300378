#![windows_subsystem = "windows"]

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Local, Months, NaiveDate};
use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, RichText, Sense,
    Shape, Stroke, Vec2, ViewportCommand,
};
use egui_extras::{Column, TableBuilder};
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize};
use sysinfo::{Disks, ProcessRefreshKind, ProcessesToUpdate, System, Users};

const BASE_DIR: &str = r"C:\checkPMI";
const LOGS_DIR: &str = r"C:\checkPMI\logs";
const CONFIG_NAME: &str = "userset.json";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const COPYRIGHT: &str = "© 2026 Studio Airbook. All rights reserved.";

// 내장 점검 스크립트 원본 (UTF-8/CRLF, 편집기 기본값 및 자동 복원 원본)
struct Script {
    key: &'static str,
    file: &'static str,
    label: &'static str,
    body: &'static [u8],
}

const SCRIPTS: &[Script] = &[
    Script { key: "daily_fas", file: "F100-2-001.bat", label: "일일 FAS 점검 (F100-2-001)", body: include_bytes!("../F100-2-001.bat") },
    Script { key: "daily_fas_detail", file: "F100-2-001-DETAIL.bat", label: "일일 FAS 상세 점검 (F100-2-001-DETAIL)", body: include_bytes!("../F100-2-001-DETAIL.bat") },
    Script { key: "daily_eai", file: "F100-1-001-DETAIL.bat", label: "일일 EAI 상세 점검 (F100-1-001-DETAIL)", body: include_bytes!("../F100-1-001-DETAIL.bat") },
    Script { key: "daily_eai_basic", file: "F100-1-001.bat", label: "일일 EAI 점검 (F100-1-001)", body: include_bytes!("../F100-1-001.bat") },
    Script { key: "monthly", file: "F100-3-030.bat", label: "월간 점검 (F100-3-030)", body: include_bytes!("../F100-3-030.bat") },
    Script { key: "annual", file: "F100-4-360.bat", label: "연간 점검 (F100-4-360)", body: include_bytes!("../F100-4-360.bat") },
    Script { key: "annual_backup", file: "F100-4-360-save.bat", label: "로그 백업 (F100-4-360-save)", body: include_bytes!("../F100-4-360-save.bat") },
    Script { key: "checkpc", file: "checkPC.bat", label: "PC 상태 수집 (checkPC)", body: include_bytes!("../assets/checkPC.bat") },
];

// 스크립트가 호출하는 보조 파일 (편집 대상 아님)
const COMPANIONS: &[(&str, &[u8])] = &[("checkPC.ps1", include_bytes!("../assets/checkPC.ps1"))];

// =====================================================================
// 설정 (userset.json, exe와 같은 폴더)
// =====================================================================
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
struct SystemCfg {
    interval_sec: u64,
    sound_alert_enabled: bool,
    beep_freq_hz: u32,
    beep_duration_ms: u32,
}

impl Default for SystemCfg {
    fn default() -> Self {
        Self { interval_sec: 2, sound_alert_enabled: true, beep_freq_hz: 1200, beep_duration_ms: 350 }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
struct Thresholds {
    cpu_percent: u32,
    ram_percent: u32,
    disk_percent: u32,
    disk_drive: String,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self { cpu_percent: 85, ram_percent: 85, disk_percent: 90, disk_drive: "C:\\".into() }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
struct Config {
    system: SystemCfg,
    thresholds: Thresholds,
    target_processes: Vec<String>,
    bat_paths: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system: SystemCfg::default(),
            thresholds: Thresholds::default(),
            target_processes: ["Gms.Application.exe", "FasServer.exe", "SMC.exe", "bgw.exe", "sqlservr.exe"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            bat_paths: SCRIPTS.iter().map(|s| (s.key.to_string(), default_bat_path(s.file))).collect(),
        }
    }
}

impl Config {
    fn bat_path(&self, key: &str) -> String {
        self.bat_paths.get(key).cloned().unwrap_or_else(|| {
            SCRIPTS.iter().find(|s| s.key == key).map(|s| default_bat_path(s.file)).unwrap_or_default()
        })
    }

    // 누락된 배치 경로를 기본값으로 채움
    fn normalize(&mut self) {
        for s in SCRIPTS {
            self.bat_paths.entry(s.key.to_string()).or_insert_with(|| default_bat_path(s.file));
        }
    }
}

fn default_bat_path(file: &str) -> String {
    Path::new(BASE_DIR).join(file).to_string_lossy().into_owned()
}

fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(CONFIG_NAME)))
        .unwrap_or_else(|| PathBuf::from(CONFIG_NAME))
}

fn load_config() -> Config {
    let path = config_path();
    match fs::read(&path) {
        Ok(bytes) => {
            let mut cfg: Config = serde_json::from_str(&decode_text(&bytes)).unwrap_or_default();
            cfg.normalize();
            cfg
        }
        Err(_) => {
            let cfg = Config::default();
            let _ = save_config(&cfg);
            cfg
        }
    }
}

fn save_config(cfg: &Config) -> Result<(), String> {
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(config_path(), text).map_err(|e| e.to_string())
}

// =====================================================================
// 텍스트 인코딩 / 파일 유틸
// =====================================================================
fn decode_text(bytes: &[u8]) -> String {
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    };
    s.trim_start_matches('\u{feff}').to_string()
}

// 편집 결과는 UTF-8 + CRLF로 저장 (배치 파일 표준 줄바꿈)
fn encode_text(text: &str) -> Vec<u8> {
    text.replace("\r\n", "\n").replace('\n', "\r\n").into_bytes()
}

// 기본 파일이 없을 때만 생성 (원본 ensure_default_bats와 동일 동작)
fn ensure_default_files() {
    let base = Path::new(BASE_DIR);
    let _ = fs::create_dir_all(base);
    let _ = fs::create_dir_all(LOGS_DIR);
    for s in SCRIPTS {
        let p = base.join(s.file);
        if !p.exists() {
            let _ = fs::write(&p, s.body);
        }
    }
    for (name, body) in COMPANIONS {
        let p = base.join(name);
        if !p.exists() {
            let _ = fs::write(&p, body);
        }
    }
}

// =====================================================================
// 경보음 (kernel32 Beep)
// =====================================================================
#[link(name = "kernel32")]
unsafe extern "system" {
    fn Beep(freq: u32, duration: u32) -> i32;
}

fn start_beep(slot: &mut Option<Arc<AtomicBool>>, freq: u32, duration: u32) {
    if slot.as_ref().is_some_and(|f| f.load(AtomicOrdering::SeqCst)) {
        return;
    }
    let flag = Arc::new(AtomicBool::new(true));
    let worker_flag = flag.clone();
    thread::spawn(move || {
        while worker_flag.load(AtomicOrdering::SeqCst) {
            unsafe {
                Beep(freq, duration);
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
    *slot = Some(flag);
}

fn stop_beep(slot: &mut Option<Arc<AtomicBool>>) {
    if let Some(f) = slot.take() {
        f.store(false, AtomicOrdering::SeqCst);
    }
}

// =====================================================================
// 시스템 샘플링
// =====================================================================
#[derive(Clone)]
struct ProcRow {
    pid: u32,
    ppid: u32,
    name: String,
    user: String,
    cpu: f32,
    mem: f32,
    is_target: bool,
    started: u64,
    exe: String,
    cmd: String,
}

#[derive(Default, Clone)]
struct DiskInfo {
    mount: String,
    total_gb: f32,
    used_gb: f32,
    percent: f32,
}

#[derive(Default, Clone)]
struct Snapshot {
    cpu: f32,
    ram: f32,
    disk: f32,
    ram_total_gb: f32,
    ram_used_gb: f32,
    disks: Vec<DiskInfo>,
    procs: Vec<ProcRow>,
    missing: Vec<String>,
    top5: Vec<u32>,
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

const GB: f32 = 1_073_741_824.0;

fn list_disks() -> Vec<DiskInfo> {
    let Ok(disks) = Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    disks
        .iter()
        .map(|d| {
            let total = d.total_space() as f32;
            let avail = d.available_space() as f32;
            let used = (total - avail).max(0.0);
            DiskInfo {
                mount: d.mount_point().to_string_lossy().into_owned(),
                total_gb: total / GB,
                used_gb: used / GB,
                percent: if total > 0.0 { used / total * 100.0 } else { 0.0 },
            }
        })
        .collect()
}

fn disk_percent_for(disks: &[DiskInfo], drive: &str) -> f32 {
    let letter = drive.chars().next().map(|c| c.to_ascii_uppercase());
    disks
        .iter()
        .find(|d| d.mount.chars().next().map(|c| c.to_ascii_uppercase()) == letter)
        .map(|d| d.percent)
        .unwrap_or(0.0)
}

fn sample(sys: &mut System, users: &Users, cores: f32, cfg: &Config) -> Snapshot {
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );

    let cpu = sys.global_cpu_usage().clamp(0.0, 100.0);
    let total_mem = sys.total_memory() as f32;
    let ram = if total_mem > 0.0 { sys.used_memory() as f32 / total_mem * 100.0 } else { 0.0 };
    let disks = list_disks();
    let disk = disk_percent_for(&disks, &cfg.thresholds.disk_drive);
    let ram_total_gb = total_mem / GB;
    let ram_used_gb = sys.used_memory() as f32 / GB;

    let targets: HashMap<String, String> =
        cfg.target_processes.iter().map(|t| (t.to_lowercase(), t.clone())).collect();
    let mut found = HashSet::new();
    let mut procs = Vec::new();

    for (pid, p) in sys.processes() {
        let raw = p.name().to_string_lossy().into_owned();
        let name = if raw.is_empty() { "System".to_string() } else { raw };
        let lower = name.to_lowercase();
        let is_target = targets.contains_key(&lower);
        if is_target {
            found.insert(lower);
        }
        let user = p.user_id().and_then(|u| users.get_user_by_id(u)).map(|u| u.name().to_string()).unwrap_or_else(|| "-".into());
        procs.push(ProcRow {
            pid: pid.as_u32(),
            ppid: p.parent().map(|x| x.as_u32()).unwrap_or(0),
            name,
            user,
            cpu: round1(p.cpu_usage() / cores),
            mem: round1(p.memory() as f32 / 1_048_576.0),
            is_target,
            started: p.start_time(),
            exe: p.exe().map(|e| e.display().to_string()).unwrap_or_default(),
            cmd: p.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" "),
        });
    }

    let mut missing: Vec<String> = targets
        .iter()
        .filter(|(k, _)| !found.contains(*k))
        .map(|(_, v)| v.clone())
        .collect();
    missing.sort();

    let mut ranked: Vec<&ProcRow> = procs.iter().collect();
    ranked.sort_by(|a, b| score(b).partial_cmp(&score(a)).unwrap_or(Ordering::Equal));
    let top5 = ranked.iter().take(5).map(|p| p.pid).collect();

    Snapshot { cpu, ram, disk, ram_total_gb, ram_used_gb, disks, procs, missing, top5 }
}

fn score(p: &ProcRow) -> f32 {
    p.cpu * 2.0 + p.mem / 50.0
}

// =====================================================================
// 점검 작업 실행 (비동기)
// =====================================================================
enum JobMsg {
    Line(String),
    Done(Option<i32>),
}

fn spawn_reader<R: Read + Send + 'static>(reader: R, tx: mpsc::Sender<JobMsg>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(reader).split(b'\n').map_while(Result::ok) {
            let text = decode_text(&line).trim().to_string();
            if !text.is_empty() {
                let _ = tx.send(JobMsg::Line(text));
            }
        }
    })
}

fn run_bat_worker(path: String, tx: mpsc::Sender<JobMsg>) {
    let dir = Path::new(&path).parent().map(Path::to_path_buf).unwrap_or_default();
    let spawned = Command::new("cmd.exe")
        .arg("/c")
        .arg(&path)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();

    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(JobMsg::Line(format!("[실행 오류] {e}")));
            let _ = tx.send(JobMsg::Done(None));
            return;
        }
    };

    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        readers.push(spawn_reader(out, tx.clone()));
    }
    if let Some(err) = child.stderr.take() {
        readers.push(spawn_reader(err, tx.clone()));
    }
    let code = child.wait().ok().and_then(|s| s.code());
    for r in readers {
        let _ = r.join();
    }
    let _ = tx.send(JobMsg::Done(code));
}

// =====================================================================
// UI 보조
// =====================================================================
#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Dash,
    Proc,
    Map,
    Run,
    Result,
    Log,
    Editor,
    Settings,
    Tasks,
    Netstat,
    Ping,
}

fn apply_theme(ctx: &egui::Context, alert: bool) {
    let mut v = egui::Visuals::dark();
    let (bg, btn, btn_hover, input, border) = if alert {
        (
            Color32::from_rgb(127, 29, 29),
            Color32::from_rgb(153, 27, 27),
            Color32::from_rgb(185, 28, 28),
            Color32::from_rgb(69, 10, 10),
            Color32::from_rgb(239, 68, 68),
        )
    } else {
        (
            Color32::from_rgb(11, 25, 44),
            Color32::from_rgb(30, 62, 98),
            Color32::from_rgb(43, 86, 128),
            Color32::from_rgb(5, 11, 20),
            Color32::from_rgb(46, 89, 132),
        )
    };
    v.panel_fill = bg;
    v.window_fill = bg;
    v.extreme_bg_color = input;
    v.override_text_color = Some(Color32::WHITE);
    v.widgets.inactive.weak_bg_fill = btn;
    v.widgets.inactive.bg_fill = btn;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    v.widgets.hovered.weak_bg_fill = btn_hover;
    v.widgets.hovered.bg_fill = btn_hover;
    v.widgets.active.weak_bg_fill = btn_hover;
    v.widgets.active.bg_fill = btn_hover;
    v.selection.bg_fill = Color32::from_rgb(2, 132, 199);
    ctx.set_visuals(v);
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    if let Ok(bytes) = fs::read(r"C:\Windows\Fonts\malgun.ttf") {
        fonts.font_data.insert("malgun".to_owned(), Arc::new(FontData::from_owned(bytes)));
        for fam in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(fam).or_default().push("malgun".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

fn gauge_color(alert: bool, value: f32, warn: f32) -> Color32 {
    if alert {
        Color32::from_rgb(239, 68, 68)
    } else if value >= warn {
        Color32::from_rgb(245, 158, 11)
    } else {
        Color32::from_rgb(56, 189, 248)
    }
}

fn gauge(ui: &mut egui::Ui, title: &str, text: &str, ratio: f32, color: Color32, alert: bool, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let p = ui.painter_at(rect);
    let c = rect.center();
    let margin = (size * 0.08).max(6.0);
    let stroke_w = (size * 0.11).max(5.0);
    let r = size / 2.0 - margin;

    let track = if alert { Color32::from_black_alpha(60) } else { Color32::from_white_alpha(25) };
    p.circle_stroke(c, r, Stroke::new(stroke_w, track));

    let ratio = ratio.clamp(0.0, 1.0);
    if ratio > 0.0 {
        let n = ((ratio * 120.0).ceil() as usize).max(2);
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = -FRAC_PI_2 + TAU * ratio * i as f32 / n as f32;
                Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
            })
            .collect();
        p.add(Shape::line(pts, Stroke::new(stroke_w, color)));
    }

    p.text(
        c + Vec2::new(0.0, -size * 0.04),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional((size * 0.14).max(8.0)),
        Color32::WHITE,
    );
    p.text(
        c + Vec2::new(0.0, size * 0.16),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional((size * 0.08).max(6.0)),
        Color32::from_rgb(203, 213, 225),
    );
    resp
}

// 시간대별 차트 (작업관리자 스타일)
#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    wall: DateTime<Local>,
    cpu: f32,
    ram: f32,
    disk: f32,
}

type Metric = (&'static str, Color32, fn(&Sample) -> f32);

const METRICS: [Metric; 3] = [
    ("CPU", Color32::from_rgb(56, 189, 248), |s| s.cpu),
    ("RAM", Color32::from_rgb(74, 222, 128), |s| s.ram),
    ("DISK", Color32::from_rgb(245, 158, 11), |s| s.disk),
];

fn history_stats(samples: &[Sample], window: Duration, f: fn(&Sample) -> f32) -> (f32, f32, f32) {
    let now = Instant::now();
    let vals: Vec<f32> = samples.iter().filter(|s| now.duration_since(s.at) <= window).map(f).collect();
    if vals.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let min = vals.iter().cloned().fold(f32::MAX, f32::min);
    let max = vals.iter().cloned().fold(f32::MIN, f32::max);
    let avg = vals.iter().sum::<f32>() / vals.len() as f32;
    (min, avg, max)
}

fn draw_chart(ui: &mut egui::Ui, height: f32, samples: &[Sample], window: Duration, metrics: &[Metric]) {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(5, 11, 20));

    let label_w = 40.0;
    let plot = egui::Rect::from_min_max(
        Pos2::new(rect.left() + label_w, rect.top() + 8.0),
        Pos2::new(rect.right() - 12.0, rect.bottom() - 22.0),
    );
    let font = FontId::proportional((height * 0.04).clamp(10.0, 16.0));
    let grid = Color32::from_white_alpha(28);
    for pct in [0, 25, 50, 75, 100] {
        let y = plot.bottom() - plot.height() * pct as f32 / 100.0;
        p.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], Stroke::new(1.0, grid));
        p.text(Pos2::new(plot.left() - 6.0, y), Align2::RIGHT_CENTER, format!("{pct}%"), font.clone(), Color32::from_rgb(148, 163, 184));
    }
    let now = Instant::now();
    let wall_now = Local::now();
    let secs = window.as_secs().max(1) as i64;
    for i in 0..=5 {
        let x = plot.right() - plot.width() * i as f32 / 5.0;
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())], Stroke::new(1.0, grid));
        let t = wall_now - chrono::Duration::seconds(secs * i / 5);
        let align = if i == 5 { Align2::LEFT_TOP } else if i == 0 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
        p.text(Pos2::new(x, plot.bottom() + 4.0), align, t.format("%H:%M:%S").to_string(), font.clone(), Color32::from_rgb(148, 163, 184));
    }

    let to_pos = |s: &Sample, v: f32| {
        let age = now.duration_since(s.at).as_secs_f32();
        let x = plot.right() - plot.width() * (age / window.as_secs_f32());
        let y = plot.bottom() - plot.height() * v.clamp(0.0, 100.0) / 100.0;
        Pos2::new(x, y)
    };
    let in_window: Vec<&Sample> = samples.iter().filter(|s| now.duration_since(s.at) <= window).collect();
    for (_, color, f) in metrics {
        let pts: Vec<Pos2> = in_window.iter().map(|s| to_pos(s, f(s))).collect();
        if pts.len() >= 2 {
            p.add(Shape::line(pts, Stroke::new(2.0, *color)));
        }
    }

    // 마우스 위치의 값 표시
    if let Some(pos) = resp.hover_pos() {
        if plot.contains(pos) {
            let age = (plot.right() - pos.x) / plot.width() * window.as_secs_f32();
            if let Some(s) = in_window.iter().min_by(|a, b| {
                (now.duration_since(a.at).as_secs_f32() - age).abs()
                    .total_cmp(&(now.duration_since(b.at).as_secs_f32() - age).abs())
            }) {
                p.line_segment([Pos2::new(pos.x, plot.top()), Pos2::new(pos.x, plot.bottom())], Stroke::new(1.0, Color32::WHITE));
                let mut text = s.wall.format("%H:%M:%S").to_string();
                for (name, _, f) in metrics {
                    text.push_str(&format!("   {name} {:.1}%", f(s)));
                }
                p.text(Pos2::new(plot.left() + 8.0, plot.top() + 4.0), Align2::LEFT_TOP, text, font.clone(), Color32::WHITE);
            }
        }
    }
}

fn chart_legend(ui: &mut egui::Ui, metrics: &[Metric]) {
    ui.horizontal(|ui| {
        for (name, color, _) in metrics {
            ui.colored_label(*color, RichText::new(format!("■ {name}")).strong());
        }
    });
}

fn cell(ui: &mut egui::Ui, bg: Option<Color32>, text: RichText) -> egui::Response {
    if let Some(c) = bg {
        ui.painter().rect_filled(ui.max_rect(), 0.0, c);
    }
    ui.add(egui::Label::new(text).sense(Sense::click()))
}

fn plain_cell(ui: &mut egui::Ui, bg: Option<Color32>, text: &str, color: Option<Color32>) {
    if let Some(c) = bg {
        ui.painter().rect_filled(ui.max_rect(), 0.0, c);
    }
    let t = RichText::new(text);
    ui.label(match color {
        Some(c) => t.color(c),
        None => t,
    });
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.label(RichText::new(k).strong());
    ui.add(egui::Label::new(v).wrap());
    ui.end_row();
}

fn opt_str(v: &Option<String>) -> &str {
    v.as_deref().unwrap_or("")
}

// =====================================================================
// PowerShell 조회: 프로세스 상세 / 작업 스케줄러
// 출력은 UTF-8 JSON을 Base64로 감싸 콘솔 코드페이지 영향을 제거
// =====================================================================
const PS_PROCESS_DETAIL: &str = r#"
$id = __PID__
$p = Get-CimInstance Win32_Process -Filter "ProcessId=$id"
if ($p) {
    $o = Invoke-CimMethod -InputObject $p -MethodName GetOwner
    $owner = ''
    if ($o -and $o.ReturnValue -eq 0) { $owner = "$($o.Domain)\$($o.User)" }
    $pname = ''
    $par = Get-Process -Id $p.ParentProcessId
    if ($par) { $pname = $par.ProcessName }
    $started = ''
    if ($p.CreationDate) { $started = $p.CreationDate.ToString('yyyy-MM-dd HH:mm:ss') }
    $svc = @(Get-CimInstance Win32_Service -Filter "ProcessId=$id" | ForEach-Object {
        [pscustomobject]@{ Name = $_.Name; DisplayName = $_.DisplayName; State = $_.State; StartMode = $_.StartMode; Description = $_.Description }
    })
    $net = @()
    try {
        $net = @(Get-NetTCPConnection -OwningProcess $id | ForEach-Object {
            [pscustomobject]@{ Local = "$($_.LocalAddress):$($_.LocalPort)"; Remote = "$($_.RemoteAddress):$($_.RemotePort)"; State = [string]$_.State }
        })
    } catch {}
    $info = [pscustomobject]@{
        CommandLine = $p.CommandLine; ExePath = $p.ExecutablePath; Started = $started
        ParentPid = $p.ParentProcessId; ParentName = $pname; Owner = $owner
        ThreadCount = $p.ThreadCount; HandleCount = $p.HandleCount
        Services = $svc; Connections = $net
    }
    $json = $info | ConvertTo-Json -Depth 5
} else {
    $json = '{}'
}
"#;

const PS_TASKS: &str = r#"
$fmt = { param($d) if ($d -and $d.Year -gt 1900) { $d.ToString('yyyy-MM-dd HH:mm:ss') } else { '' } }
$list = @(Get-ScheduledTask | ForEach-Object {
    $t = $_
    $i = $null
    try { $i = $t | Get-ScheduledTaskInfo } catch {}
    $acts = @($t.Actions | ForEach-Object { ("$($_.Execute) $($_.Arguments)").Trim() })
    $lr = $null
    $lastRun = ''; $nextRun = ''
    if ($i) { $lr = [int64]$i.LastTaskResult; $lastRun = (& $fmt $i.LastRunTime); $nextRun = (& $fmt $i.NextRunTime) }
    [pscustomobject]@{
        Name = $t.TaskName; Path = $t.TaskPath; State = [string]$t.State; Enabled = [bool]$t.Settings.Enabled
        LastRun = $lastRun; NextRun = $nextRun; LastResult = $lr
        Author = $t.Author; RunAs = $t.Principal.UserId; Actions = ($acts -join ' ; ')
    }
})
$json = ConvertTo-Json -InputObject $list -Depth 4
"#;

fn run_ps_json(body: &str) -> Result<String, String> {
    let script = format!("$ErrorActionPreference = 'SilentlyContinue'\n{body}\n[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes([string]$json))");
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encoded.as_str()])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("PowerShell 실행 실패: {e}"))?;
    let b64 = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .map_err(|_| format!("PowerShell 출력 해석 실패: {}", decode_text(&out.stderr).trim()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn one_or_many<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        Many(Vec<T>),
        One(T),
    }
    Ok(match OneOrMany::<T>::deserialize(d)? {
        OneOrMany::Many(v) => v,
        OneOrMany::One(t) => vec![t],
    })
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct ServiceInfo {
    name: Option<String>,
    display_name: Option<String>,
    state: Option<String>,
    start_mode: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct ConnInfo {
    local: Option<String>,
    remote: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct ProcDetail {
    command_line: Option<String>,
    exe_path: Option<String>,
    started: Option<String>,
    parent_pid: Option<u32>,
    parent_name: Option<String>,
    owner: Option<String>,
    thread_count: Option<u32>,
    handle_count: Option<u32>,
    #[serde(deserialize_with = "one_or_many")]
    services: Vec<ServiceInfo>,
    #[serde(deserialize_with = "one_or_many")]
    connections: Vec<ConnInfo>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct TaskRow {
    name: Option<String>,
    path: Option<String>,
    state: Option<String>,
    enabled: Option<bool>,
    last_run: Option<String>,
    next_run: Option<String>,
    last_result: Option<i64>,
    author: Option<String>,
    run_as: Option<String>,
    actions: Option<String>,
}

fn fetch_process_detail(pid: u32) -> Result<ProcDetail, String> {
    let text = run_ps_json(&PS_PROCESS_DETAIL.replace("__PID__", &pid.to_string()))?;
    let t = text.trim();
    if t.is_empty() || t == "{}" {
        return Err("프로세스 정보를 가져올 수 없습니다 (이미 종료되었거나 접근이 거부되었습니다).".into());
    }
    serde_json::from_str(t).map_err(|e| format!("응답 해석 실패: {e}"))
}

fn fetch_tasks() -> Result<Vec<TaskRow>, String> {
    let text = run_ps_json(PS_TASKS)?;
    let t = text.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    if t.starts_with('[') {
        serde_json::from_str(t).map_err(|e| format!("응답 해석 실패: {e}"))
    } else {
        serde_json::from_str::<TaskRow>(t).map(|r| vec![r]).map_err(|e| format!("응답 해석 실패: {e}"))
    }
}

// =====================================================================
// netstat -ano 조회 (TCP/UDP 포트 개방 여부 · PID)
// =====================================================================
#[derive(Clone)]
struct NetRow {
    proto: String,
    local: String,
    remote: String,
    state: String,
    pid: u32,
}

impl NetRow {
    fn local_port(&self) -> u16 {
        self.local.rsplit_once(':').and_then(|(_, p)| p.parse().ok()).unwrap_or(0)
    }

    // LISTENING(TCP) 또는 UDP 바인딩 = 포트 개방
    fn is_open(&self) -> bool {
        self.proto == "UDP" || self.state == "LISTENING"
    }
}

fn fetch_netstat() -> Result<Vec<NetRow>, String> {
    let out = Command::new("netstat.exe")
        .arg("-ano")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("netstat 실행 실패: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows: Vec<NetRow> = text
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.as_slice() {
                ["TCP", local, remote, state, pid] => Some(NetRow {
                    proto: "TCP".into(),
                    local: local.to_string(),
                    remote: remote.to_string(),
                    state: state.to_string(),
                    pid: pid.parse().ok()?,
                }),
                ["UDP", local, remote, pid] => Some(NetRow {
                    proto: "UDP".into(),
                    local: local.to_string(),
                    remote: remote.to_string(),
                    state: String::new(),
                    pid: pid.parse().ok()?,
                }),
                _ => None,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.proto.cmp(&b.proto).then(a.local_port().cmp(&b.local_port())));
    Ok(rows)
}

fn netstat_state_desc(row: &NetRow) -> &'static str {
    if row.proto == "UDP" {
        return "UDP 포트 바인딩 (연결 상태 없음)";
    }
    match row.state.as_str() {
        "LISTENING" => "수신 대기 (연결 요청 대기 중)",
        "ESTABLISHED" => "연결 수립됨 (송수신 중)",
        "TIME_WAIT" => "종료 후 대기 (지연 패킷 대비)",
        "CLOSE_WAIT" => "상대 측 종료, 로컬 종료 대기",
        "FIN_WAIT_1" | "FIN_WAIT_2" => "로컬 종료 진행 중",
        "LAST_ACK" => "최종 종료 확인 대기",
        "SYN_SENT" => "연결 요청 전송 (응답 대기)",
        "SYN_RECEIVED" => "연결 요청 수신 (응답 중)",
        "CLOSING" => "동시 종료 진행 중",
        "CLOSED" => "닫힘",
        "DELETE_TCB" => "연결 정리 중",
        _ => "알 수 없는 상태",
    }
}

fn well_known_port(port: u16) -> Option<&'static str> {
    Some(match port {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        88 => "Kerberos",
        110 => "POP3",
        135 => "RPC 엔드포인트 매퍼",
        137 | 138 | 139 => "NetBIOS",
        143 => "IMAP",
        389 => "LDAP",
        443 => "HTTPS",
        445 => "SMB",
        1433 => "MSSQL",
        1521 => "Oracle",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PostgreSQL",
        5985 => "WinRM HTTP",
        5986 => "WinRM HTTPS",
        8080 => "HTTP (대체)",
        _ => return None,
    })
}

// =====================================================================
// 핑 점검 (ping.exe 출력에서 "TTL=" 과 "NNms" 토큰만 파싱 → 한글/영문 Windows 공통)
// =====================================================================
const PING_COUNT: u32 = 4;
const GREEN: Color32 = Color32::from_rgb(74, 222, 128);
const GRAY: Color32 = Color32::from_rgb(160, 170, 185);
const RED: Color32 = Color32::from_rgb(220, 38, 38);
const YELLOW: Color32 = Color32::from_rgb(253, 224, 71);

#[derive(Clone, Copy, PartialEq)]
enum Grade {
    Good,
    Normal,
    Bad,
}

impl Grade {
    fn label(self) -> &'static str {
        match self {
            Grade::Good => "정상",
            Grade::Normal => "중간",
            Grade::Bad => "불량",
        }
    }
}

#[derive(Clone)]
struct PingResult {
    sent: u32,
    received: u32,
    min: f32,
    avg: f32,
    max: f32,
    jitter: f32,
    ttl: Option<u32>,
    os: &'static str,
    hops: Option<u32>,
    grade: Grade,
    at: String,
}

#[derive(Clone)]
struct PingTarget {
    addr: String,
    selected: bool,
    measuring: bool,
    result: Option<PingResult>,
}

fn loss_pct(r: &PingResult) -> f32 {
    (r.sent - r.received) as f32 * 100.0 / r.sent.max(1) as f32
}

fn parse_ttl(line: &str) -> Option<u32> {
    let i = line.find("TTL=")?;
    line[i + 4..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

// "시간=12ms", "time<1ms" 등에서 ms 바로 앞의 숫자 추출
// 호스트명에 "ms"가 섞일 수 있어 '=' 또는 '<' 뒤에 오는 숫자+ms만 채택
fn parse_rtt(line: &str) -> Option<f32> {
    line.match_indices("ms").find_map(|(idx, _)| {
        let head = &line[..idx];
        let start = head
            .char_indices()
            .rev()
            .find(|(_, c)| !c.is_ascii_digit())
            .map_or(0, |(p, c)| p + c.len_utf8());
        let prev = head[..start].chars().next_back()?;
        if prev != '=' && prev != '<' {
            return None;
        }
        head[start..].parse().ok()
    })
}

// TTL 초기값 추정: Linux/Unix 64, Windows 128, 네트워크 장비 255 (경유 홉마다 1씩 감소)
fn guess_os(ttl: u32) -> (&'static str, u32) {
    if ttl <= 64 {
        ("Linux/Unix", 64)
    } else if ttl <= 128 {
        ("Windows", 128)
    } else {
        ("네트워크 장비", 255)
    }
}

fn run_ping(addr: &str, warn: f32, bad: f32) -> PingResult {
    let count = PING_COUNT.to_string();
    let out = Command::new("ping.exe")
        .args(["-n", count.as_str(), "-w", "1000", addr])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();

    let mut received = 0;
    let mut ttl = None;
    let mut rtts = Vec::new();
    for line in text.lines() {
        if let Some(t) = parse_ttl(line) {
            received += 1;
            if ttl.is_none() {
                ttl = Some(t);
            }
            if let Some(v) = parse_rtt(line) {
                rtts.push(v);
            }
        }
    }

    let (min, max, avg) = if rtts.is_empty() {
        (0.0, 0.0, 0.0)
    } else {
        (
            rtts.iter().cloned().fold(f32::MAX, f32::min),
            rtts.iter().cloned().fold(f32::MIN, f32::max),
            rtts.iter().sum::<f32>() / rtts.len() as f32,
        )
    };
    let jitter = if rtts.len() < 2 {
        0.0
    } else {
        rtts.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / (rtts.len() - 1) as f32
    };

    let mut result = PingResult {
        sent: PING_COUNT,
        received,
        min,
        avg,
        max,
        jitter,
        ttl,
        os: "-",
        hops: None,
        grade: Grade::Bad,
        at: Local::now().format("%H:%M:%S").to_string(),
    };
    if let Some(t) = ttl {
        let (os, init) = guess_os(t);
        result.os = os;
        result.hops = Some(init.saturating_sub(t));
    }
    let loss = loss_pct(&result);
    result.grade = if received == 0 || loss >= 10.0 || avg >= bad {
        Grade::Bad
    } else if loss > 0.0 || avg >= warn {
        Grade::Normal
    } else {
        Grade::Good
    };
    result
}

fn grade_count(targets: &[PingTarget], g: Grade) -> usize {
    targets.iter().filter(|t| t.result.as_ref().is_some_and(|r| r.grade == g)).count()
}

fn state_label(s: &str) -> &str {
    match s {
        "Running" => "실행 중",
        "Ready" => "준비",
        "Disabled" => "사용 안 함",
        "Queued" => "대기열",
        other => other,
    }
}

fn result_label(v: Option<i64>) -> String {
    match v {
        None => "-".into(),
        Some(0) => "성공 (0)".into(),
        Some(267011) => "실행 기록 없음".into(),
        Some(267009) => "실행 중".into(),
        Some(v) => format!("오류 (0x{:X})", v as u32),
    }
}

fn elapsed_since(started: Option<&str>) -> String {
    let Some(t) = started.and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()) else {
        return "-".into();
    };
    let secs = (Local::now().naive_local() - t).num_seconds().max(0);
    format!("{}일 {:02}:{:02}:{:02}", secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60, secs % 60)
}

// 프로세스 상세 모달 상태
struct DetailView {
    pid: u32,
    name: String,
    is_target: bool,
    cpu: f32,
    mem: f32,
    rx: Option<mpsc::Receiver<Result<ProcDetail, String>>>,
    result: Option<Result<ProcDetail, String>>,
}

fn detail_body(ui: &mut egui::Ui, d: &DetailView, info: &ProcDetail) {
    let is_svchost = d.name.eq_ignore_ascii_case("svchost.exe");
    let opt_num = |v: Option<u32>| v.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
    ui.scope(|ui| {
        egui::Grid::new("detail_kv").num_columns(2).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
            kv(ui, "PID", &d.pid.to_string());
            kv(ui, "프로세스명", &d.name);
            kv(ui, "감시 대상", if d.is_target { "예" } else { "아니오" });
            kv(ui, "시작 시간", info.started.as_deref().unwrap_or("-"));
            kv(ui, "경과 시간", &elapsed_since(info.started.as_deref()));
            kv(ui, "사용자", info.owner.as_deref().unwrap_or("-"));
            let parent = format!("{} (PID {})", info.parent_name.as_deref().unwrap_or("-"), opt_num(info.parent_pid));
            kv(ui, "부모 프로세스", &parent);
            kv(ui, "CPU (%)", &format!("{:.1}", d.cpu));
            kv(ui, "메모리 (MB)", &format!("{:.1}", d.mem));
            kv(ui, "스레드 / 핸들", &format!("{} / {}", opt_num(info.thread_count), opt_num(info.handle_count)));
            kv(ui, "실행 경로", info.exe_path.as_deref().unwrap_or("-"));
            kv(ui, "명령줄", info.command_line.as_deref().unwrap_or("-"));
        });

        ui.add_space(8.0);
        ui.label(RichText::new(format!("이 프로세스를 사용하는 Windows 서비스 ({}개)", info.services.len())).strong());
        if is_svchost {
            ui.label("svchost.exe는 여러 서비스를 묶어 실행합니다. 아래 목록이 이 프로세스를 사용하는 실제 서비스입니다.");
        }
        if info.services.is_empty() {
            ui.label("없음 (서비스 호스트 프로세스가 아님)");
        } else {
            egui::ScrollArea::vertical().id_salt("svc_scroll").max_height(170.0).auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("svc_grid").num_columns(5).striped(true).spacing([8.0, 3.0]).show(ui, |ui| {
                    for h in ["서비스명", "표시 이름", "상태", "시작 유형", "설명"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for s in &info.services {
                        ui.label(opt_str(&s.name));
                        ui.add(egui::Label::new(opt_str(&s.display_name)).wrap());
                        ui.label(opt_str(&s.state));
                        ui.label(opt_str(&s.start_mode));
                        ui.add(egui::Label::new(opt_str(&s.description)).wrap());
                        ui.end_row();
                    }
                });
            });
        }

        ui.add_space(8.0);
        ui.label(RichText::new(format!("네트워크 연결 ({}개)", info.connections.len())).strong());
        if info.connections.is_empty() {
            ui.label("없음");
        } else {
            egui::ScrollArea::vertical().id_salt("conn_scroll").max_height(120.0).auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("conn_grid").num_columns(3).striped(true).spacing([8.0, 3.0]).show(ui, |ui| {
                    for h in ["로컬 주소", "원격 주소", "상태"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for c in &info.connections {
                        ui.label(opt_str(&c.local));
                        ui.label(opt_str(&c.remote));
                        ui.label(opt_str(&c.state));
                        ui.end_row();
                    }
                });
            });
        }
    });
}

fn parse_log_date(name: &str) -> Option<NaiveDate> {
    name.get(..8).and_then(|s| NaiveDate::parse_from_str(s, "%Y%m%d").ok())
}

fn days_in_month(first: NaiveDate) -> u32 {
    let next = first.checked_add_months(Months::new(1)).unwrap_or(first);
    (next - first).num_days() as u32
}

// =====================================================================
// 애플리케이션 상태
// =====================================================================
const PID_COLORS: [Color32; 12] = [
    Color32::from_rgb(56, 189, 248),
    Color32::from_rgb(74, 222, 128),
    Color32::from_rgb(245, 158, 11),
    Color32::from_rgb(244, 114, 182),
    Color32::from_rgb(167, 139, 250),
    Color32::from_rgb(248, 113, 113),
    Color32::from_rgb(45, 212, 191),
    Color32::from_rgb(250, 204, 21),
    Color32::from_rgb(129, 140, 248),
    Color32::from_rgb(251, 146, 60),
    Color32::from_rgb(163, 230, 53),
    Color32::from_rgb(232, 121, 249),
];
const PROC_HEADERS: [&str; 5] = ["PID", "프로세스명", "CPU (%)", "Memory (MB)", "구분"];
const RESULT_HEADERS: [&str; 8] = ["구분", "주기", "장치", "항목", "대상", "순번", "상태", "비고"];
const CASCADE_ALPHA: [u8; 5] = [230, 190, 150, 110, 70];

struct App {
    cfg: Config,
    draft: Config,
    draft_procs: String,
    settings_status: String,

    sys: System,
    cores: f32,
    last_tick: Option<Instant>,
    snap: Snapshot,
    alert: bool,
    summary: String,
    theme_alert: Option<bool>,
    beep: Option<Arc<AtomicBool>>,

    tab: Tab,
    search: String,
    sort_col: usize,
    sort_asc: bool,

    job_rx: Option<mpsc::Receiver<JobMsg>>,
    job_label: String,
    console: Vec<String>,

    logs: Vec<String>,
    log_filter: String,

    cal_month: NaiveDate,
    sel_date: NaiveDate,
    log_dirs: Vec<String>,
    sel_ts: Option<String>,
    result_txt: String,
    result_rows: Vec<Vec<String>>,

    editor_idx: usize,
    editor_loaded: Option<usize>,
    editor_text: String,
    editor_status: String,

    detail: Option<DetailView>,
    tasks: Vec<TaskRow>,
    tasks_rx: Option<mpsc::Receiver<Result<Vec<TaskRow>, String>>>,
    tasks_err: Option<String>,
    tasks_loaded: bool,
    task_search: String,
    hide_disabled: bool,

    users: Users,
    map_mode: MapMode,
    map_group: bool,
    map_zoom: f32,
    map_search: String,
    map_selected: Option<u32>,
    map_pan: Vec2,
    map_fit: bool,
    map_fit_key: Option<(u8, bool)>,
    map_modal: Option<u32>,
    task_sort_col: usize,
    task_sort_asc: bool,

    netstat_rows: Vec<NetRow>,
    netstat_rx: Option<mpsc::Receiver<Result<Vec<NetRow>, String>>>,
    netstat_err: Option<String>,
    netstat_loaded: bool,
    netstat_search: String,
    netstat_open_only: bool,

    ping_targets: Vec<PingTarget>,
    ping_input: String,
    ping_rx: Option<mpsc::Receiver<(String, PingResult)>>,
    ping_pending: usize,
    ping_auto: bool,
    ping_interval: u64,
    ping_next: Option<Instant>,
    ping_warn_ms: u32,
    ping_bad_ms: u32,

    history: VecDeque<Sample>,
    chart_secs: u64,
    resource_modal: Option<Resource>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, false);
        ensure_default_files();
        let cfg = load_config();
        let today = Local::now().date_naive();

        let mut app = Self {
            draft: cfg.clone(),
            draft_procs: cfg.target_processes.join("\n"),
            cfg,
            settings_status: String::new(),
            sys: System::new().expect("시스템 정보 초기화 실패"),
            cores: thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32,
            last_tick: None,
            snap: Snapshot::default(),
            alert: false,
            summary: "정상 감시 중".into(),
            theme_alert: Some(false),
            beep: None,
            tab: Tab::Dash,
            search: String::new(),
            sort_col: 0,
            sort_asc: true,
            job_rx: None,
            job_label: String::new(),
            console: Vec::new(),
            logs: Vec::new(),
            log_filter: String::new(),
            cal_month: today.with_day(1).unwrap_or(today),
            sel_date: today,
            log_dirs: Vec::new(),
            sel_ts: None,
            result_txt: String::new(),
            result_rows: Vec::new(),
            editor_idx: 0,
            editor_loaded: None,
            editor_text: String::new(),
            editor_status: String::new(),
            detail: None,
            tasks: Vec::new(),
            tasks_rx: None,
            tasks_err: None,
            tasks_loaded: false,
            task_search: String::new(),
            hide_disabled: false,
            users: Users::new_with_refreshed_list().expect("사용자 정보 초기화 실패"),
            map_mode: MapMode::All,
            map_group: true,
            map_zoom: 1.0,
            map_search: String::new(),
            map_selected: None,
            map_pan: Vec2::ZERO,
            map_fit: true,
            map_fit_key: None,
            map_modal: None,
            task_sort_col: 0,
            task_sort_asc: true,
            ping_targets: Vec::new(),
            ping_input: String::new(),
            ping_rx: None,
            ping_pending: 0,
            ping_auto: false,
            ping_interval: 30,
            ping_next: None,
            ping_warn_ms: 50,
            ping_bad_ms: 150,
            netstat_rows: Vec::new(),
            netstat_rx: None,
            netstat_err: None,
            netstat_loaded: false,
            netstat_search: String::new(),
            netstat_open_only: false,
            history: VecDeque::new(),
            chart_secs: 300,
            resource_modal: None,
        };
        app.log("INFO", format!("관제 엔진 초기화 완료 (논리 코어수: {}개 식별)", app.cores as usize));
        app.scan_logs();
        app
    }

    fn log(&mut self, level: &str, msg: impl AsRef<str>) {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S");
        self.logs.push(format!("[{now}] [{level:<7}] {}", msg.as_ref()));
    }

    fn evaluate_alert(&mut self) {
        let s = &self.snap;
        let th = &self.cfg.thresholds;
        let cpu_lim = th.cpu_percent as f32;
        let ram_lim = th.ram_percent as f32;
        let disk_lim = th.disk_percent as f32;
        let mut reasons = Vec::new();
        if !s.missing.is_empty() {
            reasons.push(format!("감시 프로세스 부재({})", s.missing.join(", ")));
        }
        if s.cpu >= cpu_lim {
            reasons.push(format!("CPU 초과({:.1}%)", s.cpu));
        }
        if s.ram >= ram_lim {
            reasons.push(format!("RAM 초과({:.1}%)", s.ram));
        }
        if s.disk >= disk_lim {
            reasons.push(format!("DISK 초과({:.1}%)", s.disk));
        }
        let count = s.procs.len();

        if !reasons.is_empty() {
            let msg = reasons.join(" / ");
            self.summary = format!("[경고] {msg}");
            if !self.alert {
                self.alert = true;
                if self.cfg.system.sound_alert_enabled {
                    start_beep(&mut self.beep, self.cfg.system.beep_freq_hz, self.cfg.system.beep_duration_ms);
                }
                self.log("WARN", msg);
            }
        } else {
            self.summary = format!("정상 감시 중 (실행 프로세스: {count}개)");
            if self.alert {
                self.alert = false;
                stop_beep(&mut self.beep);
                self.log("RECOVER", "모든 시스템 지표가 정상으로 복귀되었습니다.");
            }
        }
    }

    fn poll_job(&mut self) {
        let mut lines = Vec::new();
        let mut finished: Option<Option<i32>> = None;
        if let Some(rx) = self.job_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(JobMsg::Line(l)) => lines.push(l),
                    Ok(JobMsg::Done(code)) => {
                        finished = Some(code);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished = Some(None);
                        break;
                    }
                }
            }
        }
        self.console.extend(lines);

        if let Some(code) = finished {
            self.job_rx = None;
            let label = self.job_label.clone();
            if code == Some(0) {
                self.log("INSPECT", format!("{label} 완료"));
            } else {
                self.log("ERR", format!("{label} 종료 코드: {code:?}"));
            }
            self.scan_logs();
        }
    }

    fn start_job(&mut self, key: &'static str) {
        if self.job_rx.is_some() {
            return;
        }
        let path = self.cfg.bat_path(key);
        if !Path::new(&path).exists() {
            ensure_default_files();
        }
        if !Path::new(&path).exists() {
            self.log("ERR", format!("배치 파일이 없습니다: {path}"));
            return;
        }
        let label = SCRIPTS.iter().find(|s| s.key == key).map(|s| s.label).unwrap_or(key);
        self.console.clear();
        self.console.push(format!("[*] 배치 점검 시작: {path}"));
        self.log("INFO", format!("{label} 실행"));

        let (tx, rx) = mpsc::channel();
        self.job_rx = Some(rx);
        self.job_label = label.to_string();
        thread::spawn(move || run_bat_worker(path, tx));
    }

    fn scan_logs(&mut self) {
        let mut dirs = Vec::new();
        if let Ok(rd) = fs::read_dir(LOGS_DIR) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    dirs.push(e.file_name().to_string_lossy().into_owned());
                }
            }
        }
        dirs.sort_by(|a, b| b.cmp(a));
        self.log_dirs = dirs;
        self.ensure_selected_ts();
    }

    fn timestamps_for(&self, date: NaiveDate) -> Vec<String> {
        self.log_dirs.iter().filter(|d| parse_log_date(d) == Some(date)).cloned().collect()
    }

    fn ensure_selected_ts(&mut self) {
        let list = self.timestamps_for(self.sel_date);
        let valid = self.sel_ts.as_ref().is_some_and(|t| list.contains(t));
        if !valid {
            self.sel_ts = list.first().cloned();
            self.load_result();
        }
    }

    fn load_result(&mut self) {
        let Some(ts) = self.sel_ts.clone() else {
            self.result_txt = "해당 일자에 실행된 점검 로그가 없습니다.".into();
            self.result_rows.clear();
            return;
        };
        let dir = Path::new(LOGS_DIR).join(&ts);
        let txt_path = dir.join("check_result.txt");
        let csv_path = dir.join("check_result.csv");

        self.result_txt = match fs::read(&txt_path) {
            Ok(b) => decode_text(&b),
            Err(_) => format!("check_result.txt 없음: {}", txt_path.display()),
        };

        self.result_rows.clear();
        if let Ok(b) = fs::read(&csv_path) {
            let text = decode_text(&b);
            let mut rdr = csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(text.as_bytes());
            for rec in rdr.records().skip(1).flatten() {
                self.result_rows.push(rec.iter().map(|s| s.trim().to_string()).collect());
            }
        }
    }

    fn load_editor(&mut self, idx: usize) {
        let s = &SCRIPTS[idx];
        let path = self.cfg.bat_path(s.key);
        match fs::read(&path) {
            Ok(b) => {
                self.editor_text = decode_text(&b);
                self.editor_status = format!("파일을 불러왔습니다: {path}");
            }
            Err(_) => {
                self.editor_text = decode_text(s.body);
                self.editor_status = "파일이 없습니다. 기본 내용을 표시했으며, 저장 시 생성됩니다.".into();
            }
        }
        self.editor_loaded = Some(idx);
    }

    fn save_editor(&mut self) {
        let idx = self.editor_idx;
        let s = &SCRIPTS[idx];
        let path = self.cfg.bat_path(s.key);
        if let Some(parent) = Path::new(&path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::write(&path, encode_text(&self.editor_text)) {
            Ok(()) => {
                self.editor_status = format!("저장 완료: {path}");
                self.log("CONFIG", format!("스크립트 저장: {path}"));
            }
            Err(e) => self.editor_status = format!("저장 실패: {e}"),
        }
    }

    fn save_settings(&mut self) {
        let mut new_cfg = self.draft.clone();
        new_cfg.target_processes = self
            .draft_procs
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        new_cfg.normalize();
        match save_config(&new_cfg) {
            Ok(()) => {
                self.cfg = new_cfg.clone();
                self.draft = new_cfg;
                self.settings_status = "환경설정이 userset.json에 반영되었습니다.".into();
                self.log("CONFIG", "userset.json 설정 저장 완료");
            }
            Err(e) => self.settings_status = format!("저장 실패: {e}"),
        }
    }

    // ---------------- 탭 렌더링 ----------------
    fn ui_dashboard(&mut self, ui: &mut egui::Ui, big: bool) {
        let s = self.snap.clone();
        let th = self.cfg.thresholds.clone();
        let cpu_lim = th.cpu_percent as f32;
        let ram_lim = th.ram_percent as f32;
        let disk_lim = th.disk_percent as f32;
        let proc_count = s.procs.len();

        let mut clicked: Option<Resource> = None;
        let mut proc_clicked = false;
        let gauge_row = |ui: &mut egui::Ui, size: f32, clicked: &mut Option<Resource>, proc_clicked: &mut bool| {
            ui.horizontal(|ui| {
                let cpu_a = s.cpu >= cpu_lim;
                if gauge(ui, "CPU", &format!("{}%", s.cpu as i32), s.cpu / 100.0, gauge_color(cpu_a, s.cpu, cpu_lim), cpu_a, size).clicked() {
                    *clicked = Some(Resource::Cpu);
                }
                let ram_a = s.ram >= ram_lim;
                if gauge(ui, "RAM", &format!("{}%", s.ram as i32), s.ram / 100.0, gauge_color(ram_a, s.ram, ram_lim), ram_a, size).clicked() {
                    *clicked = Some(Resource::Ram);
                }
                let disk_a = s.disk >= disk_lim;
                if gauge(ui, "DISK", &format!("{}%", s.disk as i32), s.disk / 100.0, gauge_color(disk_a, s.disk, disk_lim), disk_a, size).clicked() {
                    *clicked = Some(Resource::Disk);
                }
                let proc_a = !s.missing.is_empty();
                if gauge(
                    ui,
                    "PROCESS",
                    &format!("{proc_count}EA"),
                    proc_count as f32 / 300.0,
                    gauge_color(proc_a, proc_count as f32, 500.0),
                    proc_a,
                    size,
                )
                .clicked()
                {
                    *proc_clicked = true;
                }
            });
        };

        if big {
            let avail = ui.available_size();
            let size = ((avail.x - 36.0) / 4.0).min(avail.y * 0.55).clamp(80.0, 420.0);
            gauge_row(ui, size, &mut clicked, &mut proc_clicked);
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(&self.summary).size(14.0).color(Color32::from_rgb(226, 232, 240)));
            });
            ui.add_space(6.0);
            let w = ui.available_width();
            let legend_h = 22.0;
            let chart_h = (ui.available_height() - legend_h - 4.0).max(120.0);
            ui.horizontal(|ui| {
                chart_legend(ui, &METRICS);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (label, secs) in [("1시간", 3600u64), ("15분", 900), ("5분", 300), ("1분", 60)] {
                        if ui.selectable_label(self.chart_secs == secs, label).clicked() {
                            self.chart_secs = secs;
                        }
                    }
                    ui.label("표시 범위:");
                });
            });
            let hist: Vec<Sample> = self.history.iter().copied().collect();
            ui.allocate_ui(Vec2::new(w, chart_h), |ui| {
                draw_chart(ui, chart_h, &hist, Duration::from_secs(self.chart_secs), &METRICS);
            });
        } else {
            let size = ((ui.available_width() - 24.0) / 4.0).clamp(60.0, 150.0);
            gauge_row(ui, size, &mut clicked, &mut proc_clicked);
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(&self.summary).size(11.0).color(Color32::from_rgb(226, 232, 240)));
            });
        }

        if let Some(r) = clicked {
            self.resource_modal = Some(r);
        }
        if proc_clicked {
            self.tab = Tab::Proc;
        }
    }

    fn ui_process(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("검색:");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("프로세스명 또는 PID 필터 검색...")
                    .desired_width(f32::INFINITY),
            );
        });

        let kw = self.search.trim().to_lowercase();
        let snap = &self.snap;
        let mut rows: Vec<ProcRow> = snap
            .procs
            .iter()
            .filter(|p| kw.is_empty() || p.name.to_lowercase().contains(&kw) || p.pid.to_string().contains(&kw))
            .cloned()
            .collect();
        let col = self.sort_col;
        rows.sort_by(|a, b| {
            let o = match col {
                0 => a.pid.cmp(&b.pid),
                1 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => a.cpu.total_cmp(&b.cpu),
                3 => a.mem.total_cmp(&b.mem),
                _ => a.is_target.cmp(&b.is_target),
            };
            if self.sort_asc { o } else { o.reverse() }
        });

        let top5 = snap.top5.clone();
        let mut clicked_col = None;
        let mut dbl: Option<usize> = None;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::exact(70.0))
            .column(Column::remainder().at_least(120.0))
            .column(Column::exact(70.0))
            .column(Column::exact(95.0))
            .column(Column::exact(80.0))
            .header(22.0, |mut h| {
                for (i, label) in PROC_HEADERS.iter().enumerate() {
                    h.col(|ui| {
                        let mark = if i == col { if self.sort_asc { " ▲" } else { " ▼" } } else { "" };
                        if ui.button(format!("{label}{mark}")).clicked() {
                            clicked_col = Some(i);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(18.0, rows.len(), |mut row| {
                    let idx = row.index();
                    let r = &rows[idx];
                    let rank = top5.iter().position(|&p| p == r.pid);
                    let bg = rank.map(|k| Color32::from_rgba_unmultiplied(185, 28, 28, CASCADE_ALPHA[k]));
                    let accent = Color32::from_rgb(56, 189, 248);
                    let kind = if r.is_target {
                        "감시대상".to_string()
                    } else if let Some(k) = rank {
                        format!("Top {}", k + 1)
                    } else {
                        "일반".to_string()
                    };
                    let name_text = RichText::new(&r.name);
                    let name_text = if r.is_target { name_text.color(accent).strong() } else { name_text };
                    let kind_text = RichText::new(kind);
                    let kind_text = if r.is_target { kind_text.color(accent).strong() } else { kind_text };

                    let mut hit = false;
                    row.col(|ui| {
                        hit |= cell(ui, bg, RichText::new(r.pid.to_string())).double_clicked();
                    });
                    row.col(|ui| {
                        hit |= cell(ui, bg, name_text).double_clicked();
                    });
                    row.col(|ui| {
                        hit |= cell(ui, bg, RichText::new(format!("{:.1}", r.cpu))).double_clicked();
                    });
                    row.col(|ui| {
                        hit |= cell(ui, bg, RichText::new(format!("{:.1}", r.mem))).double_clicked();
                    });
                    row.col(|ui| {
                        hit |= cell(ui, bg, kind_text).double_clicked();
                    });
                    if hit {
                        dbl = Some(idx);
                    }
                });
            });

        if let Some(i) = clicked_col {
            if self.sort_col == i {
                self.sort_asc = !self.sort_asc;
            } else {
                self.sort_col = i;
                self.sort_asc = true;
            }
        }
        if let Some(i) = dbl {
            let row = rows[i].clone();
            self.open_detail(&row);
        }
    }

    fn ui_run(&mut self, ui: &mut egui::Ui) {
        let running = self.job_rx.is_some();
        let mut to_run: Option<&'static str> = None;
        ui.horizontal_wrapped(|ui| {
            for s in SCRIPTS {
                if ui.add_enabled(!running, egui::Button::new(s.label)).clicked() {
                    to_run = Some(s.key);
                }
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.console {
                    ui.label(RichText::new(line).monospace().color(Color32::from_rgb(165, 243, 252)));
                }
            });
        if let Some(key) = to_run {
            self.start_job(key);
        }
    }

    fn ui_result(&mut self, ui: &mut egui::Ui) {
        let mut need_load = false;
        let mut need_scan = false;
        let dates: BTreeSet<NaiveDate> = self.log_dirs.iter().filter_map(|d| parse_log_date(d)).collect();

        ui.columns(2, |cols| {
            // 달력
            let ui = &mut cols[0];
            ui.horizontal(|ui| {
                if ui.button("◀").clicked() {
                    self.cal_month = self.cal_month.checked_sub_months(Months::new(1)).unwrap_or(self.cal_month);
                }
                ui.label(format!("{}년 {}월", self.cal_month.year(), self.cal_month.month()));
                if ui.button("▶").clicked() {
                    self.cal_month = self.cal_month.checked_add_months(Months::new(1)).unwrap_or(self.cal_month);
                }
            });
            let lead = self.cal_month.weekday().num_days_from_sunday();
            egui::Grid::new("calendar").num_columns(7).spacing([4.0, 4.0]).show(ui, |ui| {
                for w in ["일", "월", "화", "수", "목", "금", "토"] {
                    ui.label(w);
                }
                ui.end_row();
                for _ in 0..lead {
                    ui.label("");
                }
                let mut cells = lead;
                for d in 1..=days_in_month(self.cal_month) {
                    let date = NaiveDate::from_ymd_opt(self.cal_month.year(), self.cal_month.month(), d)
                        .unwrap_or(self.cal_month);
                    let text = if dates.contains(&date) { format!("{d}•") } else { d.to_string() };
                    if ui.selectable_label(date == self.sel_date, text).clicked() {
                        self.sel_date = date;
                        self.sel_ts = None;
                        need_load = true;
                    }
                    cells += 1;
                    if cells % 7 == 0 {
                        ui.end_row();
                    }
                }
            });

            // 시간대 선택
            let ui = &mut cols[1];
            ui.label("점검 이력 시간대 (타임스탬프):");
            let list = self.timestamps_for(self.sel_date);
            let current = self.sel_ts.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt("timestamps")
                .width(ui.available_width() - 8.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for ts in &list {
                        if ui.selectable_label(self.sel_ts.as_ref() == Some(ts), ts).clicked() {
                            self.sel_ts = Some(ts.clone());
                            need_load = true;
                        }
                    }
                });
            if ui.button("이력 새로고침").clicked() {
                need_scan = true;
            }
        });

        ui.add_space(6.0);
        ui.columns(2, |cols| {
            egui::ScrollArea::vertical().id_salt("res_txt").show(&mut cols[0], |ui| {
                ui.label(RichText::new(&self.result_txt).monospace());
            });

            let rows = &self.result_rows;
            TableBuilder::new(&mut cols[1])
                .striped(true)
                .resizable(true)
                .columns(Column::initial(80.0).at_least(40.0).resizable(true), RESULT_HEADERS.len() - 1)
                .column(Column::remainder())
                .header(20.0, |mut h| {
                    for label in RESULT_HEADERS {
                        h.col(|ui| {
                            ui.strong(label);
                        });
                    }
                })
                .body(|body| {
                    body.rows(18.0, rows.len(), |mut row| {
                        let r = &rows[row.index()];
                        for i in 0..RESULT_HEADERS.len() {
                            let val = r.get(i).cloned().unwrap_or_default();
                            row.col(|ui| {
                                let color = match val.as_str() {
                                    "PASS" => Some(Color32::from_rgb(56, 189, 248)),
                                    "FAIL" | "ERR" => Some(Color32::from_rgb(239, 68, 68)),
                                    "WARN" | "CHECK" => Some(Color32::from_rgb(245, 158, 11)),
                                    _ => None,
                                };
                                let t = RichText::new(val);
                                ui.label(match color {
                                    Some(c) => t.color(c),
                                    None => t,
                                });
                            });
                        }
                    });
                });
        });

        if need_scan {
            self.scan_logs();
        } else if need_load {
            self.ensure_selected_ts();
            self.load_result();
        }
    }

    fn open_detail(&mut self, row: &ProcRow) {
        let (tx, rx) = mpsc::channel();
        let pid = row.pid;
        thread::spawn(move || {
            let _ = tx.send(fetch_process_detail(pid));
        });
        self.detail = Some(DetailView {
            pid,
            name: row.name.clone(),
            is_target: row.is_target,
            cpu: row.cpu,
            mem: row.mem,
            rx: Some(rx),
            result: None,
        });
    }

    fn start_tasks_load(&mut self) {
        if self.tasks_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(fetch_tasks());
        });
        self.tasks_rx = Some(rx);
    }

    fn poll_background(&mut self) {
        if let Some(d) = self.detail.as_mut() {
            if let Some(r) = d.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
                d.result = Some(r);
                d.rx = None;
            }
        }
        if let Some(r) = self.tasks_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            match r {
                Ok(list) => {
                    self.tasks = list;
                    self.tasks_err = None;
                }
                Err(e) => self.tasks_err = Some(e),
            }
            self.tasks_loaded = true;
            self.tasks_rx = None;
        }
        if let Some(r) = self.netstat_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            match r {
                Ok(rows) => {
                    self.netstat_rows = rows;
                    self.netstat_err = None;
                }
                Err(e) => self.netstat_err = Some(e),
            }
            self.netstat_loaded = true;
            self.netstat_rx = None;
        }
    }

    fn start_netstat_load(&mut self) {
        if self.netstat_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(fetch_netstat());
        });
        self.netstat_rx = Some(rx);
    }

    fn ping_add(&mut self) {
        let addr = self.ping_input.trim().to_string();
        if addr.is_empty() {
            return;
        }
        if addr.starts_with('-') || addr.chars().any(char::is_whitespace) {
            self.log("WARN", format!("잘못된 주소 형식: {addr}"));
            return;
        }
        if !self.ping_targets.iter().any(|t| t.addr.eq_ignore_ascii_case(&addr)) {
            self.ping_targets.push(PingTarget { addr, selected: true, measuring: false, result: None });
        }
        self.ping_input.clear();
    }

    fn start_ping(&mut self) {
        if self.ping_rx.is_some() || !self.ping_targets.iter().any(|t| t.selected) {
            return;
        }
        let (warn, bad) = (self.ping_warn_ms as f32, self.ping_bad_ms as f32);
        let (tx, rx) = mpsc::channel();
        let mut n = 0;
        for t in self.ping_targets.iter_mut().filter(|t| t.selected) {
            t.measuring = true;
            n += 1;
            let (tx, addr) = (tx.clone(), t.addr.clone());
            thread::spawn(move || {
                let r = run_ping(&addr, warn, bad);
                let _ = tx.send((addr, r));
            });
        }
        self.ping_pending = n;
        self.ping_rx = Some(rx);
        self.ping_next = None;
    }

    fn poll_ping(&mut self) {
        let mut got = Vec::new();
        if let Some(rx) = self.ping_rx.as_ref() {
            while let Ok(m) = rx.try_recv() {
                got.push(m);
            }
        }
        for (addr, r) in got {
            self.ping_pending = self.ping_pending.saturating_sub(1);
            if let Some(t) = self.ping_targets.iter_mut().find(|t| t.addr == addr) {
                t.measuring = false;
                t.result = Some(r);
            }
        }
        if self.ping_rx.is_some() && self.ping_pending == 0 {
            self.ping_rx = None;
            let g = grade_count(&self.ping_targets, Grade::Good);
            let n = grade_count(&self.ping_targets, Grade::Normal);
            let b = grade_count(&self.ping_targets, Grade::Bad);
            let level = if b > 0 { "WARN" } else { "INFO" };
            self.log(level, format!("핑 점검 완료: 정상 {g} / 중간 {n} / 불량 {b}"));
            if self.ping_auto {
                self.ping_next = Some(Instant::now() + Duration::from_secs(self.ping_interval));
            }
        }
    }

    // 자동 반복: 주기가 도래하면 선택 대상 재측정
    fn ping_tick(&mut self) {
        if self.ping_auto && self.ping_rx.is_none() && self.ping_next.is_some_and(|t| Instant::now() >= t) {
            self.start_ping();
        }
    }

    fn ui_detail_modal(&mut self, ctx: &egui::Context) {
        let Some(d) = self.detail.as_mut() else {
            return;
        };
        if let Some(r) = d.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            d.result = Some(r);
            d.rx = None;
        }
        let d: &DetailView = d;
        let title = format!("프로세스 상세 - {} (PID {})", d.name, d.pid);
        let mut close = false;
        modal_window(ctx, "proc_detail", &title, &mut close, |ui| match &d.result {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("프로세스 정보를 불러오는 중...");
                });
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(239, 68, 68), e);
            }
            Some(Ok(info)) => detail_body(ui, d, info),
        });
        if close {
            self.detail = None;
        }
    }

    fn ui_resource_modal(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.resource_modal else {
            return;
        };
        let snap = self.snap.clone();
        let th = self.cfg.thresholds.clone();
        let cores = self.cores;
        let window = Duration::from_secs(self.chart_secs);
        let hist: Vec<Sample> = self.history.iter().copied().collect();
        let title = match kind {
            Resource::Cpu => "CPU 상세",
            Resource::Ram => "RAM 상세",
            Resource::Disk => "DISK 상세",
        };
        let mut close = false;
        modal_window(ctx, "resource_modal", title, &mut close, |ui| {
            resource_body(ui, kind, &snap, cores, &th, &hist, window);
        });
        if close {
            self.resource_modal = None;
        }
    }

    fn ui_tasks(&mut self, ui: &mut egui::Ui) {
        if !self.tasks_loaded && self.tasks_rx.is_none() {
            self.start_tasks_load();
        }
        let loading = self.tasks_rx.is_some();
        let mut refresh = false;
        ui.horizontal(|ui| {
            refresh = ui.add_enabled(!loading, egui::Button::new("새로고침")).clicked();
            if loading {
                ui.spinner();
                ui.label("불러오는 중...");
            }
            ui.checkbox(&mut self.hide_disabled, "사용 안 함 숨기기");
            ui.label("검색:");
            ui.add(egui::TextEdit::singleline(&mut self.task_search).desired_width(200.0));
        });
        if let Some(e) = &self.tasks_err {
            ui.colored_label(Color32::from_rgb(239, 68, 68), e);
        }

        let kw = self.task_search.trim().to_lowercase();
        let hide = self.hide_disabled;
        let mut rows: Vec<&TaskRow> = self
            .tasks
            .iter()
            .filter(|t| !(hide && t.enabled == Some(false)))
            .filter(|t| {
                kw.is_empty()
                    || opt_str(&t.name).to_lowercase().contains(&kw)
                    || opt_str(&t.path).to_lowercase().contains(&kw)
                    || opt_str(&t.actions).to_lowercase().contains(&kw)
            })
            .collect();
        let (col, asc) = (self.task_sort_col, self.task_sort_asc);
        let mut clicked_col: Option<usize> = None;
        rows.sort_by(|a, b| {
            let o = match col {
                0 => state_label(opt_str(&a.state)).cmp(state_label(opt_str(&b.state))),
                1 => a.enabled.cmp(&b.enabled),
                2 => opt_str(&a.name).to_lowercase().cmp(&opt_str(&b.name).to_lowercase()),
                3 => opt_str(&a.path).to_lowercase().cmp(&opt_str(&b.path).to_lowercase()),
                4 => opt_str(&a.last_run).cmp(opt_str(&b.last_run)),
                5 => opt_str(&a.next_run).cmp(opt_str(&b.next_run)),
                6 => a.last_result.cmp(&b.last_result),
                7 => opt_str(&a.run_as).cmp(opt_str(&b.run_as)),
                _ => opt_str(&a.actions).cmp(opt_str(&b.actions)),
            };
            if asc { o } else { o.reverse() }
        });
        let running = self.tasks.iter().filter(|t| t.state.as_deref() == Some("Running")).count();
        ui.label(format!(
            "전체 {}개 / 표시 {}개 / 실행 중 {}개",
            self.tasks.len(),
            rows.len(),
            running
        ));

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::initial(80.0).at_least(60.0))
            .column(Column::initial(50.0).at_least(40.0))
            .column(Column::initial(180.0).at_least(100.0))
            .column(Column::initial(200.0).at_least(100.0))
            .column(Column::initial(130.0).at_least(100.0))
            .column(Column::initial(130.0).at_least(100.0))
            .column(Column::initial(110.0).at_least(80.0))
            .column(Column::initial(120.0).at_least(80.0))
            .column(Column::remainder().at_least(150.0))
            .header(22.0, |mut h| {
                for (i, l) in ["실행 상태", "사용", "작업 이름", "작업 경로", "마지막 실행", "다음 실행", "마지막 결과", "실행 계정", "실행 명령"].iter().enumerate() {
                    h.col(|ui| {
                        let mark = if i == col { if asc { " ▲" } else { " ▼" } } else { "" };
                        if ui.button(format!("{l}{mark}")).clicked() {
                            clicked_col = Some(i);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(18.0, rows.len(), |mut row| {
                    let t = rows[row.index()];
                    let is_running = t.state.as_deref() == Some("Running");
                    let disabled = t.enabled == Some(false);
                    let bg = is_running.then(|| Color32::from_rgba_unmultiplied(2, 132, 199, 110));
                    let color = disabled.then_some(Color32::GRAY);
                    let state = state_label(opt_str(&t.state)).to_string();
                    let enabled = match t.enabled {
                        Some(true) => "예",
                        Some(false) => "아니오",
                        None => "-",
                    };
                    let result = result_label(t.last_result);
                    row.col(|ui| plain_cell(ui, bg, &state, if is_running { Some(Color32::WHITE) } else { color }));
                    row.col(|ui| plain_cell(ui, bg, enabled, color));
                    row.col(|ui| plain_cell(ui, bg, opt_str(&t.name), color));
                    row.col(|ui| plain_cell(ui, bg, opt_str(&t.path), color));
                    row.col(|ui| plain_cell(ui, bg, opt_str(&t.last_run), color));
                    row.col(|ui| plain_cell(ui, bg, opt_str(&t.next_run), color));
                    row.col(|ui| plain_cell(ui, bg, &result, color));
                    row.col(|ui| plain_cell(ui, bg, opt_str(&t.run_as), color));
                    row.col(|ui| plain_cell(ui, bg, opt_str(&t.actions), color));
                });
            });

        if let Some(i) = clicked_col {
            if self.task_sort_col == i {
                self.task_sort_asc = !self.task_sort_asc;
            } else {
                self.task_sort_col = i;
                self.task_sort_asc = true;
            }
            ui.ctx().request_repaint();
        }
        if refresh {
            self.start_tasks_load();
        }
    }

    fn ui_netstat(&mut self, ui: &mut egui::Ui) {
        if !self.netstat_loaded && self.netstat_rx.is_none() {
            self.start_netstat_load();
        }
        let loading = self.netstat_rx.is_some();
        let mut refresh = false;
        ui.horizontal(|ui| {
            refresh = ui.add_enabled(!loading, egui::Button::new("새로고침")).clicked();
            if loading {
                ui.spinner();
                ui.label("불러오는 중...");
            }
            ui.checkbox(&mut self.netstat_open_only, "개방 포트만 보기");
            ui.label("검색:");
            ui.add(
                egui::TextEdit::singleline(&mut self.netstat_search)
                    .hint_text("주소, 포트, PID, 프로세스명")
                    .desired_width(240.0),
            );
        });
        if let Some(e) = &self.netstat_err {
            ui.colored_label(Color32::from_rgb(239, 68, 68), e);
        }

        // PID -> 프로세스명 (스냅샷 기준)
        let names: HashMap<u32, &str> = self.snap.procs.iter().map(|p| (p.pid, p.name.as_str())).collect();
        let kw = self.netstat_search.trim().to_lowercase();
        let open_only = self.netstat_open_only;
        let rows: Vec<&NetRow> = self
            .netstat_rows
            .iter()
            .filter(|r| !open_only || r.is_open())
            .filter(|r| {
                kw.is_empty()
                    || r.proto.to_lowercase().contains(&kw)
                    || r.local.to_lowercase().contains(&kw)
                    || r.remote.to_lowercase().contains(&kw)
                    || r.pid.to_string().contains(&kw)
                    || names.get(&r.pid).is_some_and(|n| n.to_lowercase().contains(&kw))
            })
            .collect();
        // 같은 PID는 같은 색 (PID 순서대로 팔레트를 순환 배정)
        let mut uniq: Vec<u32> = self.netstat_rows.iter().map(|r| r.pid).collect();
        uniq.sort_unstable();
        uniq.dedup();
        let pid_color = |pid: u32| PID_COLORS[uniq.binary_search(&pid).unwrap_or(0) % PID_COLORS.len()];
        let open_total = self.netstat_rows.iter().filter(|r| r.is_open()).count();
        ui.label(format!(
            "전체 {}개 / 표시 {}개 / 개방 {}개",
            self.netstat_rows.len(),
            rows.len(),
            open_total
        ));

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::initial(70.0).at_least(60.0))
            .column(Column::initial(65.0).at_least(50.0))
            .column(Column::initial(170.0).at_least(100.0))
            .column(Column::initial(170.0).at_least(100.0))
            .column(Column::initial(110.0).at_least(80.0))
            .column(Column::initial(70.0).at_least(50.0))
            .column(Column::initial(150.0).at_least(100.0))
            .column(Column::remainder().at_least(180.0))
            .header(22.0, |mut h| {
                for l in ["개방 여부", "프로토콜", "로컬 주소", "외부 주소", "상태", "PID", "프로세스명", "설명"] {
                    h.col(|ui| {
                        ui.strong(l);
                    });
                }
            })
            .body(|body| {
                body.rows(18.0, rows.len(), |mut row| {
                    let r = rows[row.index()];
                    let open = r.is_open();
                    let pc = pid_color(r.pid);
                    let tint = Some(Color32::from_rgba_unmultiplied(pc.r(), pc.g(), pc.b(), 50));
                    let status_color = open.then_some(GREEN);
                    let pname = names.get(&r.pid).copied().unwrap_or("-");
                    let svc = well_known_port(r.local_port()).map(|s| format!("{s} · ")).unwrap_or_default();
                    let desc = format!("{svc}{}", netstat_state_desc(r));
                    let state = if r.proto == "UDP" { "-" } else { r.state.as_str() };
                    let pid = r.pid.to_string();
                    row.col(|ui| plain_cell(ui, tint, if open { "개방" } else { "연결" }, status_color));
                    row.col(|ui| plain_cell(ui, tint, &r.proto, None));
                    row.col(|ui| plain_cell(ui, tint, &r.local, None));
                    row.col(|ui| plain_cell(ui, tint, &r.remote, None));
                    row.col(|ui| plain_cell(ui, tint, state, None));
                    row.col(|ui| plain_cell(ui, tint, &pid, Some(pc)));
                    row.col(|ui| plain_cell(ui, tint, pname, Some(pc)));
                    row.col(|ui| plain_cell(ui, tint, &desc, None));
                });
            });

        if refresh {
            self.start_netstat_load();
        }
    }

    fn ui_ping(&mut self, ui: &mut egui::Ui) {
        let running = self.ping_rx.is_some();
        let (mut add, mut run, mut delete, mut auto_changed) = (false, false, false, false);
        let mut sel_all: Option<bool> = None;

        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.ping_input)
                    .hint_text("IP 또는 호스트명을 입력하고 Enter (한 줄에 하나씩 추가)")
                    .desired_width(340.0),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                add = true;
            }
            if ui.button("추가").clicked() {
                add = true;
            }
        });

        ui.horizontal(|ui| {
            run = ui.add_enabled(!running, egui::Button::new("체크 대상 핑")).clicked();
            if running {
                ui.spinner();
                ui.label(format!("측정 중... (남은 {}개)", self.ping_pending));
            }
            if ui.button("전체 선택").clicked() {
                sel_all = Some(true);
            }
            if ui.button("전체 해제").clicked() {
                sel_all = Some(false);
            }
            delete = ui.add_enabled(!running, egui::Button::new("체크 항목 삭제")).clicked();
        });

        ui.horizontal(|ui| {
            auto_changed = ui.checkbox(&mut self.ping_auto, "자동 반복").changed();
            let auto_on = self.ping_auto;
            ui.add_enabled(auto_on, egui::DragValue::new(&mut self.ping_interval).range(5..=3600).suffix(" 초"));
            ui.separator();
            ui.label("중간 기준");
            ui.add(egui::DragValue::new(&mut self.ping_warn_ms).range(1..=5000).suffix(" ms"));
            ui.label("불량 기준");
            ui.add(egui::DragValue::new(&mut self.ping_bad_ms).range(1..=10000).suffix(" ms"));
        });

        ui.label(
            RichText::new(format!(
                "판정: 정상 = 손실 0% & 평균 < 중간 기준 / 중간 = 손실 1~9% 또는 평균 ≥ 중간 기준 / \
                 불량 = 응답 없음, 손실 10% 이상 또는 평균 ≥ 불량 기준   |   OS는 응답 TTL로 추정 (64=Linux, 128=Windows, 255=장비)"
            ))
            .size(11.0)
            .color(GRAY),
        );

        let n_sel = self.ping_targets.iter().filter(|t| t.selected).count();
        ui.label(format!(
            "전체 {} / 체크 {} / 정상 {} / 중간 {} / 불량 {}",
            self.ping_targets.len(),
            n_sel,
            grade_count(&self.ping_targets, Grade::Good),
            grade_count(&self.ping_targets, Grade::Normal),
            grade_count(&self.ping_targets, Grade::Bad),
        ));

        let view: Vec<PingTarget> = self.ping_targets.clone();
        let mut sel: Vec<bool> = view.iter().map(|t| t.selected).collect();
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::initial(45.0).at_least(40.0))
            .column(Column::initial(170.0).at_least(100.0))
            .column(Column::initial(60.0).at_least(50.0))
            .column(Column::initial(55.0).at_least(45.0))
            .column(Column::initial(70.0).at_least(55.0))
            .column(Column::initial(65.0).at_least(50.0))
            .column(Column::initial(65.0).at_least(50.0))
            .column(Column::initial(65.0).at_least(50.0))
            .column(Column::initial(45.0).at_least(40.0))
            .column(Column::initial(120.0).at_least(90.0))
            .column(Column::initial(45.0).at_least(40.0))
            .column(Column::remainder().at_least(70.0))
            .header(22.0, |mut h| {
                for l in ["선택", "주소", "상태", "손실", "평균(ms)", "최소(ms)", "최대(ms)", "지터(ms)", "TTL", "OS 추정", "홉", "측정 시각"] {
                    h.col(|ui| {
                        ui.strong(l);
                    });
                }
            })
            .body(|body| {
                body.rows(18.0, view.len(), |mut row| {
                    let i = row.index();
                    let t = &view[i];
                    let (status, bg, fg) = match (&t.result, t.measuring) {
                        (_, true) => ("측정 중".to_string(), None, GRAY),
                        (None, _) => ("미측정".to_string(), None, GRAY),
                        (Some(r), _) => match r.grade {
                            Grade::Good => (r.grade.label().to_string(), None, GREEN),
                            Grade::Normal => (r.grade.label().to_string(), None, GRAY),
                            Grade::Bad => (r.grade.label().to_string(), Some(YELLOW), RED),
                        },
                    };
                    let fmt = |f: fn(&PingResult) -> String| t.result.as_ref().map_or_else(|| "-".to_string(), f);
                    let cells = [
                        status,
                        fmt(|r| format!("{:.0}%", loss_pct(r))),
                        fmt(|r| format!("{:.1}", r.avg)),
                        fmt(|r| format!("{:.1}", r.min)),
                        fmt(|r| format!("{:.1}", r.max)),
                        fmt(|r| format!("{:.1}", r.jitter)),
                        fmt(|r| r.ttl.map_or("-".to_string(), |v| v.to_string())),
                        fmt(|r| r.os.to_string()),
                        fmt(|r| r.hops.map_or("-".to_string(), |v| v.to_string())),
                        fmt(|r| r.at.clone()),
                    ];
                    row.col(|ui| {
                        if let Some(c) = bg {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, c);
                        }
                        ui.checkbox(&mut sel[i], "");
                    });
                    row.col(|ui| plain_cell(ui, bg, &t.addr, Some(fg)));
                    for c in &cells {
                        row.col(|ui| plain_cell(ui, bg, c.as_str(), Some(fg)));
                    }
                });
            });
        for (t, s) in self.ping_targets.iter_mut().zip(sel) {
            t.selected = s;
        }

        if auto_changed {
            self.ping_next = if self.ping_auto { Some(Instant::now()) } else { None };
        }
        if let Some(v) = sel_all {
            for t in &mut self.ping_targets {
                t.selected = v;
            }
        }
        if delete {
            self.ping_targets.retain(|t| !t.selected);
        }
        if add {
            self.ping_add();
        }
        if run {
            self.start_ping();
        }
    }

    fn ui_logs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("검색:");
            ui.add(
                egui::TextEdit::singleline(&mut self.log_filter)
                    .hint_text("로그 검색...")
                    .desired_width(f32::INFINITY),
            );
        });
        let kw = self.log_filter.to_lowercase();
        egui::ScrollArea::vertical()
            .stick_to_bottom(kw.is_empty())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for l in self.logs.iter().filter(|l| kw.is_empty() || l.to_lowercase().contains(&kw)) {
                    ui.label(RichText::new(l).monospace().color(Color32::from_rgb(56, 189, 248)));
                }
            });
    }

    fn ui_editor(&mut self, ui: &mut egui::Ui) {
        if self.editor_loaded != Some(self.editor_idx) {
            self.load_editor(self.editor_idx);
        }
        let mut select: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (i, s) in SCRIPTS.iter().enumerate() {
                if ui.selectable_label(self.editor_idx == i, s.file).clicked() {
                    select = Some(i);
                }
            }
        });
        if let Some(i) = select {
            self.editor_idx = i;
            return;
        }

        let key = SCRIPTS[self.editor_idx].key;
        ui.label(RichText::new(format!("경로: {}", self.cfg.bat_path(key))).small());

        let mut reload = false;
        let mut restore = false;
        let mut save = false;
        ui.horizontal(|ui| {
            reload = ui.button("파일에서 다시 읽기").clicked();
            restore = ui.button("기본 내용 불러오기").clicked();
            save = ui.button("저장").clicked();
        });
        ui.label(RichText::new(&self.editor_status).small().color(Color32::from_rgb(203, 213, 225)));

        egui::ScrollArea::vertical().id_salt("editor").auto_shrink([false, false]).show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.editor_text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(24),
            );
        });

        if reload {
            self.load_editor(self.editor_idx);
        }
        if restore {
            self.editor_text = decode_text(SCRIPTS[self.editor_idx].body);
            self.editor_status = "기본 내용을 편집기에 채웠습니다. 저장해야 파일에 반영됩니다.".into();
        }
        if save {
            self.save_editor();
        }
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.group(|ui| {
                ui.label(RichText::new("감시 대상 프로세스 목록 (줄바꿈 구분)").strong());
                ui.add(
                    egui::TextEdit::multiline(&mut self.draft_procs)
                        .desired_width(f32::INFINITY)
                        .desired_rows(5),
                );
            });

            ui.group(|ui| {
                ui.label(RichText::new("리소스 경고 임계치 (%)").strong());
                let th = &mut self.draft.thresholds;
                ui.add(egui::Slider::new(&mut th.cpu_percent, 10..=100).text("CPU (%)"));
                ui.add(egui::Slider::new(&mut th.ram_percent, 10..=100).text("RAM (%)"));
                ui.add(egui::Slider::new(&mut th.disk_percent, 10..=100).text("DISK (%)"));
                ui.horizontal(|ui| {
                    ui.label("감시 드라이브:");
                    ui.text_edit_singleline(&mut th.disk_drive);
                });
            });

            ui.group(|ui| {
                ui.label(RichText::new("점검 배치 스크립트 경로 (.bat / .ps1 호출 bat)").strong());
                for s in SCRIPTS {
                    let path = self.draft.bat_paths.entry(s.key.to_string()).or_default();
                    ui.horizontal(|ui| {
                        ui.add_sized([150.0, 20.0], egui::Label::new(s.file));
                        ui.add(egui::TextEdit::singleline(path).desired_width(f32::INFINITY));
                    });
                }
            });

            if ui.button(RichText::new("설정 저장 (userset.json 즉시 반영)").strong()).clicked() {
                self.save_settings();
            }
            if !self.settings_status.is_empty() {
                ui.label(&self.settings_status);
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let inner = ctx.input(|i| i.viewport().inner_rect);
        let is_big = inner.is_some_and(|r| r.width() as i32 == 1920 && r.height() as i32 == 1080);

        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
        }

        let interval = Duration::from_secs(self.cfg.system.interval_sec.max(1));
        if self.last_tick.is_none_or(|t| t.elapsed() >= interval) {
            self.last_tick = Some(Instant::now());
            self.users.refresh();
            self.snap = sample(&mut self.sys, &self.users, self.cores, &self.cfg);
            self.history.push_back(Sample {
                at: Instant::now(),
                wall: Local::now(),
                cpu: self.snap.cpu,
                ram: self.snap.ram,
                disk: self.snap.disk,
            });
            while self.history.front().is_some_and(|s| s.at.elapsed() > Duration::from_secs(3600)) {
                self.history.pop_front();
            }
            self.evaluate_alert();
        }
        self.poll_job();
        self.poll_background();
        self.poll_ping();
        self.ping_tick();

        if self.theme_alert != Some(self.alert) {
            apply_theme(ctx, self.alert);
            self.theme_alert = Some(self.alert);
        }

        let busy = self.job_rx.is_some()
            || self.tasks_rx.is_some()
            || self.netstat_rx.is_some()
            || self.ping_rx.is_some()
            || self.detail.as_ref().is_some_and(|d| d.rx.is_some());
        let repaint = if busy { Duration::from_millis(100) } else { interval };
        ctx.request_repaint_after(repaint);

        egui::CentralPanel::default().show(root, |ui| {
            ui.horizontal(|ui| {
                let (text, color) = if self.alert {
                    ("SYSTEM ALERT", Color32::from_rgb(220, 38, 38))
                } else {
                    ("SYSTEM NORMAL", Color32::from_rgb(2, 132, 199))
                };
                ui.label(RichText::new(text).strong().color(Color32::WHITE).background_color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(if fullscreen { "창모드" } else { "전체화면" }).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
                    }
                    if ui.button(if is_big { "400p" } else { "1080p" }).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
                        let (w, h) = if is_big { (400.0, 400.0) } else { (1920.0, 1080.0) };
                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(w, h)));
                    }
                });
            });

            ui.horizontal_wrapped(|ui| {
                let tabs = [
                    (Tab::Dash, "대시보드"),
                    (Tab::Proc, "프로세스"),
                    (Tab::Map, "프로세스맵"),
                    (Tab::Run, "점검실행"),
                    (Tab::Result, "결과조회"),
                    (Tab::Log, "로그"),
                    (Tab::Editor, "스크립트 편집"),
                    (Tab::Tasks, "작업 스케줄러"),
                    (Tab::Netstat, "네트워크"),
                    (Tab::Ping, "핑"),
                    (Tab::Settings, "설정"),
                ];
                for (t, label) in tabs {
                    if ui.selectable_label(self.tab == t, RichText::new(label).strong()).clicked() {
                        self.tab = t;
                    }
                }
            });
            ui.separator();

            let footer_h = 16.0;
            let body_h = (ui.available_height() - footer_h).max(0.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), body_h), |ui| match self.tab {
                Tab::Dash => self.ui_dashboard(ui, fullscreen),
                Tab::Proc => self.ui_process(ui),
                Tab::Map => self.ui_map(ui),
                Tab::Run => self.ui_run(ui),
                Tab::Result => self.ui_result(ui),
                Tab::Log => self.ui_logs(ui),
                Tab::Editor => self.ui_editor(ui),
                Tab::Settings => self.ui_settings(ui),
                Tab::Tasks => self.ui_tasks(ui),
                Tab::Netstat => self.ui_netstat(ui),
                Tab::Ping => self.ui_ping(ui),
            });

            ui.vertical_centered(|ui| {
                ui.label(RichText::new(COPYRIGHT).size(9.0).color(Color32::from_white_alpha(110)));
            });
        });
        self.ui_detail_modal(ctx);
        self.ui_resource_modal(ctx);
        self.ui_map_modal(ctx);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        stop_beep(&mut self.beep);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Studio Airbook - FAS & EAI Server Guard")
            .with_inner_size([400.0, 400.0])
            .with_min_inner_size([380.0, 380.0]),
        // Intel Vulkan/OpenGL 드라이버(igvk64.dll) 충돌 회피: DX12 백엔드 고정
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: {
            let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
            setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
            eframe::egui_wgpu::WgpuConfiguration {
                wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(setup),
                ..Default::default()
            }
        },
        ..Default::default()
    };
    eframe::run_native(
        "checkserver",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Resource {
    Cpu,
    Ram,
    Disk,
}

// 공통 모달: 화면 비율 크기, 상단 [닫기] 고정, 해상도에 따라 글자 크기 확대
#[allow(deprecated)]
fn modal_window(ctx: &egui::Context, id: &str, title: &str, close: &mut bool, body: impl FnOnce(&mut egui::Ui)) {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *close = true;
    }
    let screen = ctx.content_rect();
    let scale = (screen.height() / 900.0).clamp(1.0, 1.8);
    egui::Area::new(egui::Id::new((id, "overlay")))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(170));
            if resp.clicked() {
                *close = true;
            }
        });

    egui::Window::new(title)
        .id(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(screen.width() * 0.6, screen.height() * 0.8))
        .max_size(screen.size() * 0.96)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            for (_, font) in ui.style_mut().text_styles.iter_mut() {
                font.size *= scale;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).strong().size(18.0 * scale.min(1.4)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("닫기").size(16.0)).clicked() {
                        *close = true;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, body);
        });
}

fn top_table(ui: &mut egui::Ui, procs: &[ProcRow], key: fn(&ProcRow) -> f32, label: &str) {
    let mut v: Vec<&ProcRow> = procs.iter().collect();
    v.sort_by(|a, b| key(b).total_cmp(&key(a)));
    ui.label(RichText::new(format!("{label} 상위 15개 프로세스")).strong());
    egui::Grid::new(format!("top_{label}")).num_columns(4).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
        for h in ["PID", "프로세스", "CPU (%)", "메모리 (MB)"] {
            ui.strong(h);
        }
        ui.end_row();
        for p in v.iter().take(15) {
            ui.label(p.pid.to_string());
            ui.label(&p.name);
            ui.label(format!("{:.1}", p.cpu));
            ui.label(format!("{:.1}", p.mem));
            ui.end_row();
        }
    });
}

fn resource_body(ui: &mut egui::Ui, kind: Resource, snap: &Snapshot, cores: f32, th: &Thresholds, hist: &[Sample], window: Duration) {
    let (name, cur, limit, metric) = match kind {
        Resource::Cpu => ("CPU", snap.cpu, th.cpu_percent as f32, METRICS[0]),
        Resource::Ram => ("RAM", snap.ram, th.ram_percent as f32, METRICS[1]),
        Resource::Disk => ("DISK", snap.disk, th.disk_percent as f32, METRICS[2]),
    };
    let (min, avg, max) = history_stats(hist, window, metric.2);
    let status = if cur >= limit { "경고" } else { "정상" };
    let color = if cur >= limit { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(56, 189, 248) };
    ui.label(RichText::new(format!("{name} 사용률 {cur:.1}%   (경고 임계치 {limit:.0}%)   상태: {status}")).strong().color(color));
    ui.label(format!(
        "최근 {}분 통계  |  최소 {min:.1}%  평균 {avg:.1}%  최대 {max:.1}%",
        (window.as_secs() / 60).max(1)
    ));
    ui.add_space(6.0);
    draw_chart(ui, 240.0, hist, window, &[metric]);
    ui.add_space(10.0);

    match kind {
        Resource::Cpu => {
            ui.label(format!("논리 코어 수: {cores:.0}개"));
            top_table(ui, &snap.procs, |p| p.cpu, "CPU");
        }
        Resource::Ram => {
            let free = (snap.ram_total_gb - snap.ram_used_gb).max(0.0);
            ui.label(format!(
                "전체 {:.1} GB  /  사용 {:.1} GB  /  여유 {:.1} GB",
                snap.ram_total_gb, snap.ram_used_gb, free
            ));
            top_table(ui, &snap.procs, |p| p.mem, "메모리");
        }
        Resource::Disk => {
            ui.label(RichText::new("드라이브별 사용량").strong());
            egui::Grid::new("disk_grid").num_columns(4).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                for h in ["드라이브", "전체 (GB)", "사용 (GB)", "사용률 (%)"] {
                    ui.strong(h);
                }
                ui.end_row();
                for d in &snap.disks {
                    ui.label(&d.mount);
                    ui.label(format!("{:.1}", d.total_gb));
                    ui.label(format!("{:.1}", d.used_gb));
                    ui.label(format!("{:.1}", d.percent));
                    ui.end_row();
                }
            });
        }
    }
}

// =====================================================================
// 프로세스맵: 부모→자식 종속 관계를 좌→우 트리로 표시
// =====================================================================
#[derive(Clone, Copy, PartialEq)]
enum MapMode {
    All,
    Targets,
    Search,
    Selected,
}

const MAP_COL_W: f32 = 230.0;
const MAP_ROW_H: f32 = 46.0;
const MAP_NODE_W: f32 = 190.0;
const MAP_NODE_H: f32 = 36.0;

#[derive(Clone, PartialEq, Eq, Hash)]
enum MapKey {
    Account(String),
    Proc(u32),
}

struct MapLayout {
    pos: HashMap<MapKey, (usize, f32)>,
    edges: Vec<(MapKey, MapKey)>,
    cols: usize,
    rows: f32,
}

struct MapBuilder<'a> {
    children: &'a HashMap<u32, Vec<u32>>,
    next_row: f32,
    pos: HashMap<MapKey, (usize, f32)>,
    edges: Vec<(MapKey, MapKey)>,
    max_col: usize,
    visited: HashSet<u32>,
}

impl MapBuilder<'_> {
    fn place(&mut self, pid: u32, col: usize) -> f32 {
        self.visited.insert(pid);
        self.max_col = self.max_col.max(col);
        let kids: Vec<u32> = self
            .children
            .get(&pid)
            .map(|v| v.iter().copied().filter(|c| !self.visited.contains(c)).collect())
            .unwrap_or_default();
        let row = if kids.is_empty() {
            let r = self.next_row;
            self.next_row += 1.0;
            r
        } else {
            let mut sum = 0.0;
            for c in &kids {
                let r = self.place(*c, col + 1);
                sum += r;
                self.edges.push((MapKey::Proc(pid), MapKey::Proc(*c)));
            }
            sum / kids.len() as f32
        };
        self.pos.insert(MapKey::Proc(pid), (col, row));
        row
    }
}

fn build_map(procs: &[ProcRow], keep: &HashSet<u32>, group: bool) -> MapLayout {
    let by_pid: HashMap<u32, &ProcRow> = procs.iter().filter(|p| keep.contains(&p.pid)).map(|p| (p.pid, p)).collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut roots: Vec<&ProcRow> = Vec::new();
    for p in by_pid.values() {
        if p.ppid != p.pid && by_pid.contains_key(&p.ppid) {
            children.entry(p.ppid).or_default().push(p.pid);
        } else {
            roots.push(p);
        }
    }
    for v in children.values_mut() {
        v.sort();
    }
    roots.sort_by_key(|p| p.pid);

    let mut b = MapBuilder {
        children: &children,
        next_row: 0.0,
        pos: HashMap::new(),
        edges: Vec::new(),
        max_col: 0,
        visited: HashSet::new(),
    };
    let base_col = if group { 1 } else { 0 };
    if group {
        let mut by_user: std::collections::BTreeMap<String, Vec<u32>> = std::collections::BTreeMap::new();
        for r in &roots {
            by_user.entry(r.user.clone()).or_default().push(r.pid);
        }
        for (user, pids) in by_user {
            let mut sum = 0.0;
            for pid in &pids {
                let row = b.place(*pid, base_col);
                sum += row;
                b.edges.push((MapKey::Account(user.clone()), MapKey::Proc(*pid)));
            }
            b.pos.insert(MapKey::Account(user.clone()), (0, sum / pids.len() as f32));
        }
    } else {
        for r in &roots {
            b.place(r.pid, base_col);
        }
    }
    MapLayout { pos: b.pos, edges: b.edges, cols: b.max_col + 1, rows: b.next_row }
}

// 연결 기준 pid 집합: 조상 경로와 하위 트리 포함
fn relation_set(ppid: &HashMap<u32, u32>, children: &HashMap<u32, Vec<u32>>, seed: u32, keep: &mut HashSet<u32>) {
    keep.insert(seed);
    let mut cur = seed;
    for _ in 0..128 {
        match ppid.get(&cur) {
            Some(&p) if p != 0 && p != cur && ppid.contains_key(&p) => {
                keep.insert(p);
                cur = p;
            }
            _ => break,
        }
    }
    let mut stack = vec![seed];
    while let Some(c) = stack.pop() {
        if let Some(kids) = children.get(&c) {
            for k in kids {
                if keep.insert(*k) {
                    stack.push(*k);
                }
            }
        }
    }
}

fn bezier(a: Pos2, b: Pos2) -> Vec<Pos2> {
    let dx = (b.x - a.x).abs().max(20.0) * 0.5;
    let c1 = Pos2::new(a.x + dx, a.y);
    let c2 = Pos2::new(b.x - dx, b.y);
    (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            let u = 1.0 - t;
            let x = u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x;
            let y = u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y;
            Pos2::new(x, y)
        })
        .collect()
}

fn fmt_epoch(secs: u64) -> String {
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".into())
}

fn elapsed_text(secs: u64) -> String {
    let now = Local::now().timestamp().max(0) as u64;
    let s = now.saturating_sub(secs);
    format!("{}일 {:02}:{:02}:{:02}", s / 86400, (s % 86400) / 3600, (s % 3600) / 60, s % 60)
}

impl App {
    fn ui_map(&mut self, ui: &mut egui::Ui) {
        let snap = self.snap.clone();
        let mut open_wmi: Option<ProcRow> = None;
        let mut open_modal: Option<u32> = None;

        ui.horizontal_wrapped(|ui| {
            ui.label("표시:");
            egui::ComboBox::from_id_salt("map_mode")
                .selected_text(match self.map_mode {
                    MapMode::All => "전체",
                    MapMode::Targets => "감시 대상 관련",
                    MapMode::Search => "검색 일치 관련",
                    MapMode::Selected => "선택 프로세스 관련",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.map_mode, MapMode::All, "전체");
                    ui.selectable_value(&mut self.map_mode, MapMode::Targets, "감시 대상 관련");
                    ui.selectable_value(&mut self.map_mode, MapMode::Search, "검색 일치 관련");
                    ui.selectable_value(&mut self.map_mode, MapMode::Selected, "선택 프로세스 관련");
                });
            ui.checkbox(&mut self.map_group, "계정별 묶음");
            ui.label("검색:");
            ui.add(egui::TextEdit::singleline(&mut self.map_search).desired_width(140.0).hint_text("이름 또는 PID"));
            if ui.button("맞춤 보기").clicked() {
                self.map_fit = true;
            }
            if ui.button("100%").clicked() {
                self.map_zoom = 1.0;
                self.map_pan = Vec2::ZERO;
            }
            ui.label(format!("배율 {:.0}%  (휠/핀치: 확대·축소, 드래그: 이동)", self.map_zoom * 100.0));
        });

        let ppid: HashMap<u32, u32> = snap.procs.iter().map(|p| (p.pid, p.ppid)).collect();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for p in &snap.procs {
            if p.ppid != p.pid && ppid.contains_key(&p.ppid) {
                children.entry(p.ppid).or_default().push(p.pid);
            }
        }
        let kw = self.map_search.trim().to_lowercase();
        let mut keep: HashSet<u32> = HashSet::new();
        match self.map_mode {
            MapMode::All => keep.extend(snap.procs.iter().map(|p| p.pid)),
            MapMode::Targets => {
                for p in snap.procs.iter().filter(|p| p.is_target) {
                    relation_set(&ppid, &children, p.pid, &mut keep);
                }
            }
            MapMode::Search => {
                for p in snap
                    .procs
                    .iter()
                    .filter(|p| !kw.is_empty() && (p.name.to_lowercase().contains(&kw) || p.pid.to_string().contains(&kw)))
                {
                    relation_set(&ppid, &children, p.pid, &mut keep);
                }
            }
            MapMode::Selected => {
                if let Some(s) = self.map_selected {
                    relation_set(&ppid, &children, s, &mut keep);
                }
            }
        }

        let mode_idx = self.map_mode as u8;
        let key = (mode_idx, self.map_group);
        if self.map_fit_key != Some(key) {
            self.map_fit_key = Some(key);
            self.map_fit = true;
        }

        let layout = build_map(&snap.procs, &keep, self.map_group);
        if keep.is_empty() {
            ui.label(if self.map_mode == MapMode::Search && kw.is_empty() {
                "검색어를 입력하면 해당 프로세스의 관계도가 표시됩니다."
            } else {
                "표시할 프로세스가 없습니다."
            });
        }
        ui.label(format!(
            "표시 노드 {}개 / 전체 {}개  (클릭: 선택 · 더블클릭: 관계 정보 모달)",
            keep.len(),
            snap.procs.len()
        ));

        let by_pid: HashMap<u32, &ProcRow> = snap.procs.iter().map(|p| (p.pid, p)).collect();
        let info_w = 300.0;
        let avail = ui.available_size();
        let canvas_size = Vec2::new((avail.x - info_w - 12.0).max(300.0), avail.y.max(300.0));

        ui.horizontal(|ui| {
            let (rect, resp) = ui.allocate_exact_size(canvas_size, Sense::click_and_drag());
            let gw = layout.cols as f32 * MAP_COL_W + MAP_NODE_W;
            let gh = layout.rows.max(1.0) * MAP_ROW_H;

            if self.map_fit && !keep.is_empty() {
                let z = ((rect.width() - 40.0) / gw).min((rect.height() - 40.0) / gh).clamp(0.05, 1.5);
                self.map_zoom = z;
                self.map_pan = (rect.size() - Vec2::new(gw, gh) * z) / 2.0;
                self.map_fit = false;
            }

            if resp.dragged() {
                self.map_pan += resp.drag_delta();
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }

            if resp.hovered() {
                let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                let factor = pinch * (scroll * 0.0015).exp();
                if (factor - 1.0).abs() > 1e-4 {
                    if let Some(ptr) = resp.hover_pos() {
                        let rel = ptr - rect.min;
                        let g = (rel - self.map_pan) / self.map_zoom;
                        let nz = (self.map_zoom * factor).clamp(0.05, 3.0);
                        self.map_zoom = nz;
                        self.map_pan = rel - g * nz;
                    }
                }
            }

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 6.0, Color32::from_rgb(5, 11, 20));
            let z = self.map_zoom;
            let origin = rect.min + self.map_pan;
            let node_tl = |key: &MapKey| -> Option<Pos2> {
                layout.pos.get(key).map(|(c, r)| origin + Vec2::new(*c as f32 * MAP_COL_W * z, *r * MAP_ROW_H * z))
            };
            let nw = MAP_NODE_W * z;
            let nh = MAP_NODE_H * z;

            for (a, b) in &layout.edges {
                if let (Some(pa), Some(pb)) = (node_tl(a), node_tl(b)) {
                    let s = pa + Vec2::new(nw, nh / 2.0);
                    let e = pb + Vec2::new(0.0, nh / 2.0);
                    painter.add(Shape::line(bezier(s, e), Stroke::new((1.5 * z).clamp(0.8, 3.0), Color32::from_rgb(100, 116, 139))));
                }
            }

            let font = FontId::proportional((12.0 * z).clamp(6.0, 22.0));
            let small = FontId::proportional((10.0 * z).clamp(5.0, 18.0));
            for key in layout.pos.keys() {
                let Some(tl) = node_tl(key) else { continue };
                let r = egui::Rect::from_min_size(tl, Vec2::new(nw, nh));
                if !rect.intersects(r) {
                    continue;
                }
                let id_str = match key {
                    MapKey::Account(n) => format!("a:{n}"),
                    MapKey::Proc(p) => format!("p:{p}"),
                };
                let nresp = ui.interact(r.intersect(rect), ui.next_auto_id().with(id_str), Sense::click_and_drag());
                if nresp.dragged() {
                    self.map_pan += nresp.drag_delta();
                }
                match key {
                    MapKey::Account(name) => {
                        painter.rect(
                            r,
                            6.0 * z,
                            Color32::from_rgb(76, 29, 149),
                            Stroke::new(1.0, Color32::from_rgb(167, 139, 250)),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            r.left_center() + Vec2::new(8.0 * z, 0.0),
                            Align2::LEFT_CENTER,
                            format!("계정: {name}"),
                            font.clone(),
                            Color32::WHITE,
                        );
                    }
                    MapKey::Proc(pid) => {
                        let Some(p) = by_pid.get(pid) else { continue };
                        let selected = self.map_selected == Some(*pid);
                        let fill = if p.is_target { Color32::from_rgb(12, 60, 90) } else { Color32::from_rgb(15, 27, 46) };
                        let stroke = if selected {
                            Stroke::new(2.0, Color32::from_rgb(250, 204, 21))
                        } else if p.is_target {
                            Stroke::new(1.5, Color32::from_rgb(56, 189, 248))
                        } else {
                            Stroke::new(1.0, Color32::from_rgb(51, 65, 85))
                        };
                        painter.rect(r, 6.0 * z, fill, stroke, egui::StrokeKind::Inside);
                        let name: String = if p.name.chars().count() > 22 {
                            p.name.chars().take(21).collect::<String>() + "…"
                        } else {
                            p.name.clone()
                        };
                        painter.text(r.left_top() + Vec2::new(8.0 * z, 4.0 * z), Align2::LEFT_TOP, name, font.clone(), Color32::WHITE);
                        painter.text(
                            r.left_bottom() + Vec2::new(8.0 * z, -4.0 * z),
                            Align2::LEFT_BOTTOM,
                            format!("PID {}  CPU {:.1}%  {:.0}MB", p.pid, p.cpu, p.mem),
                            small.clone(),
                            Color32::from_rgb(148, 163, 184),
                        );
                        if nresp.clicked() {
                            self.map_selected = Some(*pid);
                        }
                        if nresp.double_clicked() {
                            open_modal = Some(*pid);
                        }
                    }
                }
            }

            ui.vertical(|ui| {
                ui.set_width(info_w);
                ui.heading("선택 프로세스");
                match self.map_selected.and_then(|s| by_pid.get(&s).copied()) {
                    None => {
                        ui.label("노드를 클릭하면 정보가 표시됩니다.");
                    }
                    Some(p) => {
                        let parent = by_pid.get(&p.ppid).map(|x| x.name.clone()).unwrap_or_else(|| "-".into());
                        let child_count = children.get(&p.pid).map(|v| v.len()).unwrap_or(0);
                        egui::Grid::new("map_info").num_columns(2).striped(true).show(ui, |ui| {
                            ui.label("이름");
                            ui.label(&p.name);
                            ui.end_row();
                            ui.label("PID");
                            ui.label(p.pid.to_string());
                            ui.end_row();
                            ui.label("부모");
                            ui.label(format!("{parent} ({})", p.ppid));
                            ui.end_row();
                            ui.label("계정");
                            ui.label(&p.user);
                            ui.end_row();
                            ui.label("자식 수");
                            ui.label(child_count.to_string());
                            ui.end_row();
                            ui.label("CPU / 메모리");
                            ui.label(format!("{:.1}% / {:.1} MB", p.cpu, p.mem));
                            ui.end_row();
                        });
                        if ui.button("관계 정보 모달 열기").clicked() {
                            open_modal = Some(p.pid);
                        }
                        if ui.button("WMI 상세 (서비스·연결 포함)").clicked() {
                            open_wmi = Some((*p).clone());
                        }
                    }
                }
            });
        });

        if let Some(pid) = open_modal {
            self.map_selected = Some(pid);
            self.map_modal = Some(pid);
        }
        if let Some(p) = open_wmi {
            self.open_detail(&p);
        }
    }

    fn ui_map_modal(&mut self, ctx: &egui::Context) {
        let Some(pid) = self.map_modal else {
            return;
        };
        let snap = self.snap.clone();
        let by_pid: HashMap<u32, &ProcRow> = snap.procs.iter().map(|p| (p.pid, p)).collect();
        let p: ProcRow = match by_pid.get(&pid) {
            Some(p) => (*p).clone(),
            None => {
                self.map_modal = None;
                return;
            }
        };

        // 부모 체인 (부모 → 조상 순)
        let mut ancestors: Vec<ProcRow> = Vec::new();
        let mut cur = p.ppid;
        for _ in 0..64 {
            match by_pid.get(&cur) {
                Some(a) if a.pid != p.pid && !ancestors.iter().any(|x| x.pid == a.pid) => {
                    ancestors.push((*a).clone());
                    cur = a.ppid;
                }
                _ => break,
            }
        }
        let mut kids: Vec<ProcRow> = snap.procs.iter().filter(|c| c.ppid == pid && c.pid != pid).cloned().collect();
        kids.sort_by_key(|c| c.pid);
        let kid_count = |c: &ProcRow| snap.procs.iter().filter(|x| x.ppid == c.pid && x.pid != c.pid).count();

        let title = format!("프로세스 관계 - {} (PID {})", p.name, p.pid);
        let mut close = false;
        let mut nav: Option<u32> = None;
        let mut wmi = false;
        modal_window(ctx, "map_modal", &title, &mut close, |ui| {
            ui.label(RichText::new("기본 정보").strong().size(16.0));
            egui::Grid::new("map_modal_info").num_columns(2).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                kv(ui, "프로세스명", &p.name);
                kv(ui, "PID", &p.pid.to_string());
                kv(ui, "감시 대상", if p.is_target { "예" } else { "아니오" });
                kv(ui, "계정", &p.user);
                kv(ui, "시작 시간", &if p.started > 0 { fmt_epoch(p.started) } else { "-".into() });
                kv(ui, "경과 시간", &if p.started > 0 { elapsed_text(p.started) } else { "-".into() });
                kv(ui, "CPU (%)", &format!("{:.1}", p.cpu));
                kv(ui, "메모리 (MB)", &format!("{:.1}", p.mem));
                kv(ui, "실행 파일", if p.exe.is_empty() { "-" } else { &p.exe });
                kv(ui, "명령줄", if p.cmd.is_empty() { "-" } else { &p.cmd });
            });

            ui.add_space(12.0);
            ui.label(RichText::new(format!("부모 체인 (조상 {}단계)", ancestors.len())).strong().size(16.0));
            if ancestors.is_empty() {
                ui.label("최상위 프로세스이거나 부모가 이미 종료되었습니다.");
            } else {
                for (depth, a) in ancestors.iter().rev().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(depth as f32 * 18.0);
                        ui.label("↳");
                        if ui.button(format!("{} (PID {}) · {}", a.name, a.pid, a.user)).clicked() {
                            nav = Some(a.pid);
                        }
                    });
                }
            }
            ui.horizontal(|ui| {
                ui.add_space(ancestors.len() as f32 * 18.0);
                ui.label("▶");
                ui.strong(format!("{} (PID {})  ← 현재", p.name, p.pid));
            });

            ui.add_space(12.0);
            ui.label(RichText::new(format!("자식 프로세스 ({}개)", kids.len())).strong().size(16.0));
            if kids.is_empty() {
                ui.label("자식 프로세스가 없습니다 (말단 프로세스).");
            } else {
                egui::Grid::new("map_modal_kids").num_columns(6).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                    for h in ["PID", "프로세스명", "계정", "CPU (%)", "메모리 (MB)", "하위 수"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for c in &kids {
                        ui.label(c.pid.to_string());
                        if ui.button(&c.name).on_hover_text("클릭하면 이 프로세스의 관계 정보로 이동").clicked() {
                            nav = Some(c.pid);
                        }
                        ui.label(&c.user);
                        ui.label(format!("{:.1}", c.cpu));
                        ui.label(format!("{:.1}", c.mem));
                        ui.label(kid_count(c).to_string());
                        ui.end_row();
                    }
                });
            }

            ui.add_space(12.0);
            if ui.button("WMI 상세 정보 열기 (서비스 · 네트워크 연결 포함)").clicked() {
                wmi = true;
            }
        });

        if close {
            self.map_modal = None;
        } else if let Some(n) = nav {
            self.map_modal = Some(n);
        }
        if wmi {
            self.open_detail(&p);
        }
    }
}
