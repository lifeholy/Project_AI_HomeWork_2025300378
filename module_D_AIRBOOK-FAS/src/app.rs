//! AIRBOOK-FAS & EAI 서버 관제 GUI (egui). 탭: 대시보드 / 프로세스 / 점검실행 / 결과조회 / 로그 / 설정.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Local, NaiveDate};
use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, RichText, Sense, Shape, Stroke,
    Vec2, ViewportCommand,
};

use crate::batch::{self, BatMsg};
use crate::config::{self, Config};
use crate::logs;
use crate::monitor::{Monitor, ProcRow, Snapshot};

const COLOR_OK: Color32 = Color32::from_rgb(56, 189, 248);
const COLOR_WARN: Color32 = Color32::from_rgb(245, 158, 11);
const COLOR_ALERT: Color32 = Color32::from_rgb(239, 68, 68);
const COLOR_TEXT_DIM: Color32 = Color32::from_rgb(148, 163, 184);
const CONSOLE_MAX: usize = 2000;

/// 부저 (kernel32!Beep). Windows 전용.
#[link(name = "kernel32")]
extern "system" {
    fn Beep(dw_freq: u32, dw_duration: u32) -> i32;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Process,
    Bat,
    Results,
    Logs,
    Settings,
}

#[derive(Clone)]
struct ProcDetail {
    pid: u32,
    name: String,
    exe: String,
    cmd: String,
    start: u64,
}

pub struct App {
    cfg: Config,
    draft: Config,
    procs_text: String,
    monitor: Monitor,
    snapshot: Option<Snapshot>,
    last_tick: Instant,

    alert: bool,
    alert_msg: String,
    beep_stop: Option<Arc<AtomicBool>>,

    tab: Tab,
    proc_sort: usize,
    proc_desc: bool,
    proc_filter: String,
    detail: Option<ProcDetail>,
    show_disks: bool,

    bat_tx: mpsc::Sender<BatMsg>,
    bat_rx: mpsc::Receiver<BatMsg>,
    bat_running: bool,
    console: Vec<String>,

    result_day: NaiveDate,
    result_day_text: String,
    result_runs: Vec<String>,
    result_run: Option<String>,
    result_text: String,
    result_rows: Vec<Vec<String>>,

    log_lines: Vec<String>,
    log_filter: String,
    time_sync_out: String,
    fullscreen: bool,
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\malgun.ttf") {
        fonts.font_data.insert("malgun".to_owned(), FontData::from_owned(bytes));
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push("malgun".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

fn beep_loop(stop: Arc<AtomicBool>, freq: u32, duration: u32) {
    while !stop.load(Ordering::SeqCst) {
        unsafe {
            Beep(freq, duration);
        }
        thread::sleep(Duration::from_secs(1));
    }
}

/// 원형 게이지. 클릭 여부를 Response로 돌려준다.
fn donut(ui: &mut egui::Ui, title: &str, value_text: &str, ratio: f32, alert: bool, warn: bool) -> egui::Response {
    let size = 150.0_f32;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = size / 2.0 - 14.0;
    let stroke_w = 12.0;
    let track = if alert {
        Color32::from_black_alpha(90)
    } else {
        Color32::from_white_alpha(25)
    };
    painter.circle_stroke(center, radius, Stroke::new(stroke_w, track));

    let color = if alert {
        COLOR_ALERT
    } else if warn {
        COLOR_WARN
    } else {
        COLOR_OK
    };
    let ratio = ratio.clamp(0.0, 1.0);
    if ratio > 0.0 {
        let steps = ((ratio * 120.0).ceil() as usize).max(2);
        let points: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                let angle = FRAC_PI_2 - ratio * TAU * t;
                Pos2::new(center.x + radius * angle.cos(), center.y - radius * angle.sin())
            })
            .collect();
        painter.add(Shape::line(points, Stroke::new(stroke_w, color)));
    }
    painter.text(
        center + Vec2::new(0.0, -6.0),
        Align2::CENTER_CENTER,
        value_text,
        FontId::proportional(22.0),
        Color32::WHITE,
    );
    painter.text(
        center + Vec2::new(0.0, 20.0),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(14.0),
        COLOR_TEXT_DIM,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn status_color(text: &str) -> Color32 {
    match text {
        "PASS" => COLOR_OK,
        "FAIL" | "ERR" => COLOR_ALERT,
        "WARN" | "CHECK" => COLOR_WARN,
        _ => Color32::WHITE,
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        batch::ensure_default_bats();
        let cfg = config::load();
        let (bat_tx, bat_rx) = mpsc::channel();
        let today = Local::now().date_naive();
        let mut app = Self {
            draft: cfg.clone(),
            procs_text: cfg.target_processes.join("\n"),
            cfg,
            monitor: Monitor::new(),
            snapshot: None,
            last_tick: Instant::now() - Duration::from_secs(60),
            alert: false,
            alert_msg: "정상 감시 중".to_string(),
            beep_stop: None,
            tab: Tab::Dashboard,
            proc_sort: 2,
            proc_desc: true,
            proc_filter: String::new(),
            detail: None,
            show_disks: false,
            bat_tx,
            bat_rx,
            bat_running: false,
            console: Vec::new(),
            result_day: today,
            result_day_text: today.format("%Y-%m-%d").to_string(),
            result_runs: Vec::new(),
            result_run: None,
            result_text: String::new(),
            result_rows: Vec::new(),
            log_lines: Vec::new(),
            log_filter: String::new(),
            time_sync_out: String::new(),
            fullscreen: false,
        };
        app.refresh_logs();
        app.refresh_runs(true);
        app
    }

    // ----- 감시 -----

    fn tick(&mut self) {
        let th = self.cfg.thresholds.clone();
        let snap = self.monitor.sample(&self.cfg.target_processes, &th.disk_drive);

        let mut reasons: Vec<String> = Vec::new();
        if !snap.missing_targets.is_empty() {
            reasons.push(format!("감시 프로세스 부재({})", snap.missing_targets.join(", ")));
        }
        if snap.proc_count >= th.proc_count_limit {
            reasons.push(format!(
                "프로세스 수량 초과({}EA >= {}EA)",
                snap.proc_count, th.proc_count_limit
            ));
        }
        if snap.cpu >= th.cpu_percent {
            reasons.push(format!("CPU 초과({:.1}%)", snap.cpu));
        }
        if snap.ram >= th.ram_percent {
            reasons.push(format!("RAM 초과({:.1}%)", snap.ram));
        }
        if snap.disk >= th.disk_percent {
            reasons.push(format!("DISK 초과({:.1}%)", snap.disk));
        }

        let alert = !reasons.is_empty();
        if alert {
            self.alert_msg = format!("[경고] {}", reasons.join(" / "));
        } else {
            self.alert_msg = format!("정상 감시 중 (실행 프로세스: {}개)", snap.proc_count);
        }
        if alert && !self.alert {
            self.alert = true;
            self.start_beep();
            logs::append("WARN", &reasons.join(" / "));
        } else if !alert && self.alert {
            self.alert = false;
            self.stop_beep();
            logs::append("RECOVER", "모든 시스템 지표가 정상으로 복귀되었습니다.");
        }
        self.snapshot = Some(snap);
    }

    fn start_beep(&mut self) {
        if !self.cfg.system.sound_alert_enabled || self.beep_stop.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let freq = self.cfg.system.beep_freq_hz;
        let dur = self.cfg.system.beep_duration_ms;
        let flag = stop.clone();
        thread::spawn(move || beep_loop(flag, freq, dur));
        self.beep_stop = Some(stop);
    }

    fn stop_beep(&mut self) {
        if let Some(flag) = self.beep_stop.take() {
            flag.store(true, Ordering::SeqCst);
        }
    }

    // ----- 배치 -----

    fn run_bat(&mut self, path: String) {
        if self.bat_running {
            return;
        }
        let path = std::path::PathBuf::from(path);
        if !path.exists() {
            batch::ensure_default_bats();
        }
        self.console.clear();
        self.bat_running = true;
        self.tab = Tab::Bat;
        batch::spawn(path, self.bat_tx.clone());
    }

    fn poll_bat(&mut self) {
        while let Ok(msg) = self.bat_rx.try_recv() {
            match msg {
                BatMsg::Line(line) => {
                    self.console.push(line);
                    if self.console.len() > CONSOLE_MAX {
                        self.console.remove(0);
                    }
                }
                BatMsg::Done(ok) => {
                    self.bat_running = false;
                    if ok {
                        logs::append("INSPECT", "점검 배치 완료 (TXT/CSV 결과 저장)");
                        self.result_day = Local::now().date_naive();
                        self.result_day_text = self.result_day.format("%Y-%m-%d").to_string();
                        self.refresh_runs(true);
                        self.refresh_logs();
                    } else {
                        logs::append("ERR", "배치 실행 중 오류 감지");
                        self.console.push("[오류] 배치 실행이 실패했습니다.".to_string());
                    }
                }
            }
        }
    }

    // ----- 결과 / 로그 -----

    fn refresh_runs(&mut self, select_first: bool) {
        self.result_runs = batch::list_runs(self.result_day);
        let keep = self
            .result_run
            .as_ref()
            .map(|r| self.result_runs.contains(r))
            .unwrap_or(false);
        if !keep {
            self.result_run = if select_first { self.result_runs.first().cloned() } else { None };
        }
        self.load_run();
    }

    fn load_run(&mut self) {
        match self.result_run.clone() {
            Some(name) => {
                self.result_text = batch::read_result_text(&name);
                self.result_rows = batch::read_result_rows(&name);
            }
            None => {
                self.result_text = "해당 일자에 실행된 점검 로그가 없습니다.".to_string();
                self.result_rows.clear();
            }
        }
    }

    fn refresh_logs(&mut self) {
        self.log_lines = logs::read_recent(1000);
    }

    fn shift_result_day(&mut self, days: i64) {
        self.result_day += ChronoDuration::days(days);
        self.result_day_text = self.result_day.format("%Y-%m-%d").to_string();
        self.refresh_runs(true);
    }

    // ----- 화면 -----

    fn draw_top(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            let (badge, color) = if self.alert {
                ("SYSTEM ALERT", COLOR_ALERT)
            } else {
                ("SYSTEM NORMAL", COLOR_OK)
            };
            ui.label(RichText::new(badge).strong().color(Color32::WHITE).background_color(color));
            ui.label(RichText::new(&self.alert_msg).color(if self.alert { COLOR_ALERT } else { COLOR_TEXT_DIM }));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("전체화면 (F11)").clicked() {
                    self.toggle_fullscreen(ctx);
                }
                if ui.button("1080p").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(1920.0, 1080.0)));
                }
            });
        });
        ui.horizontal(|ui| {
            let tabs = [
                (Tab::Dashboard, "대시보드"),
                (Tab::Process, "프로세스"),
                (Tab::Bat, "점검실행"),
                (Tab::Results, "결과조회"),
                (Tab::Logs, "로그"),
                (Tab::Settings, "설정"),
            ];
            for (tab, label) in tabs {
                if ui.selectable_label(self.tab == tab, label).clicked() {
                    self.tab = tab;
                    if tab == Tab::Logs {
                        self.refresh_logs();
                    }
                }
            }
        });
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn draw_dashboard(&mut self, ui: &mut egui::Ui) {
        let Some(snap) = self.snapshot.clone() else {
            ui.label("데이터 수집 중...");
            return;
        };
        let th = self.cfg.thresholds.clone();
        ui.horizontal_wrapped(|ui| {
            let cpu = donut(
                ui,
                "CPU",
                &format!("{:.0}%", snap.cpu),
                (snap.cpu / 100.0) as f32,
                snap.cpu >= th.cpu_percent,
                snap.cpu >= th.cpu_percent * 0.8,
            );
            if cpu.clicked() {
                self.open_process_tab(2);
            }
            let ram = donut(
                ui,
                "RAM",
                &format!("{:.0}%", snap.ram),
                (snap.ram / 100.0) as f32,
                snap.ram >= th.ram_percent,
                snap.ram >= th.ram_percent * 0.8,
            );
            if ram.clicked() {
                self.open_process_tab(3);
            }
            let disk = donut(
                ui,
                "DISK",
                &format!("{:.0}%", snap.disk),
                (snap.disk / 100.0) as f32,
                snap.disk >= th.disk_percent,
                snap.disk >= th.disk_percent * 0.8,
            );
            if disk.clicked() {
                self.show_disks = true;
            }
            let limit = th.proc_count_limit.max(1) as f32;
            let proc_alert = snap.proc_count >= th.proc_count_limit || !snap.missing_targets.is_empty();
            let proc = donut(
                ui,
                "PROCESS",
                &format!("{}EA", snap.proc_count),
                (snap.proc_count as f32 / limit).min(1.0),
                proc_alert,
                false,
            );
            if proc.clicked() {
                self.open_process_tab(1);
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new(format!(
            "CPU 경고 {:.0}% / RAM 경고 {:.0}% / DISK 경고 {:.0}% ({}) / 프로세스 경고 {}EA",
            th.cpu_percent, th.ram_percent, th.disk_percent, th.disk_drive, th.proc_count_limit
        ))
        .color(COLOR_TEXT_DIM));
        if !snap.missing_targets.is_empty() {
            ui.colored_label(COLOR_ALERT, format!("감시 프로세스 부재: {}", snap.missing_targets.join(", ")));
        }
    }

    fn open_process_tab(&mut self, sort_col: usize) {
        self.tab = Tab::Process;
        self.proc_sort = sort_col;
        self.proc_desc = true;
    }

    fn draw_process(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("검색:");
            ui.text_edit_singleline(&mut self.proc_filter);
        });
        let Some(snap) = self.snapshot.clone() else {
            return;
        };
        let keyword = self.proc_filter.trim().to_lowercase();
        let mut rows: Vec<&ProcRow> = snap
            .procs
            .iter()
            .filter(|p| keyword.is_empty() || p.name.to_lowercase().contains(&keyword) || p.pid.to_string().contains(&keyword))
            .collect();
        rows.sort_by(|a, b| {
            let ord = match self.proc_sort {
                0 => a.pid.cmp(&b.pid),
                1 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => a.cpu.partial_cmp(&b.cpu).unwrap_or(std::cmp::Ordering::Equal),
                _ => a.mem_mb.partial_cmp(&b.mem_mb).unwrap_or(std::cmp::Ordering::Equal),
            };
            if self.proc_desc { ord.reverse() } else { ord }
        });

        // 점유 상위 5개 (python 버전과 같은 가중치: cpu*2 + mem/50)
        let mut ranked: Vec<&ProcRow> = snap.procs.iter().collect();
        ranked.sort_by(|a, b| {
            let sa = a.cpu * 2.0 + a.mem_mb / 50.0;
            let sb = b.cpu * 2.0 + b.mem_mb / 50.0;
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        let top5: Vec<u32> = ranked.iter().take(5).map(|p| p.pid).collect();

        let mut open_detail: Option<ProcDetail> = None;
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("proc_grid").striped(true).num_columns(5).show(ui, |ui| {
                let headers = ["PID", "프로세스명", "CPU (%)", "Memory (MB)", "구분"];
                for (i, h) in headers.iter().enumerate() {
                    if ui.selectable_label(self.proc_sort == i, *h).clicked() {
                        if self.proc_sort == i {
                            self.proc_desc = !self.proc_desc;
                        } else {
                            self.proc_sort = i;
                            self.proc_desc = true;
                        }
                    }
                }
                ui.end_row();
                for p in rows.iter().take(1000) {
                    let top_rank = top5.iter().position(|pid| *pid == p.pid);
                    let mut text = |s: String| {
                        let mut t = RichText::new(s);
                        if p.is_target {
                            t = t.color(COLOR_OK).strong();
                        } else if top_rank.is_some() {
                            t = t.color(COLOR_ALERT);
                        }
                        t
                    };
                    ui.label(text(p.pid.to_string()));
                    let name_resp = ui.add(egui::Label::new(text(p.name.clone())).sense(Sense::click()));
                    if name_resp.double_clicked() {
                        open_detail = Some(ProcDetail {
                            pid: p.pid,
                            name: p.name.clone(),
                            exe: p.exe.clone(),
                            cmd: p.cmd.clone(),
                            start: p.start,
                        });
                    }
                    ui.label(text(format!("{:.1}", p.cpu)));
                    ui.label(text(format!("{:.1}", p.mem_mb)));
                    let kind = if p.is_target {
                        "감시대상".to_string()
                    } else if let Some(r) = top_rank {
                        format!("Top {}", r + 1)
                    } else {
                        "일반".to_string()
                    };
                    ui.label(text(kind));
                    ui.end_row();
                }
            });
        });
        if open_detail.is_some() {
            self.detail = open_detail;
        }
    }

    fn draw_bat(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let enabled = !self.bat_running;
            if ui
                .add_enabled(enabled, egui::Button::new("일일 FAS 점검 (.bat)"))
                .clicked()
            {
                let p = self.cfg.bat_paths.daily_fas.clone();
                self.run_bat(p);
            }
            if ui
                .add_enabled(enabled, egui::Button::new("일일 EAI 점검 (.bat)"))
                .clicked()
            {
                let p = self.cfg.bat_paths.daily_eai.clone();
                self.run_bat(p);
            }
            if self.bat_running {
                ui.spinner();
                ui.label("실행 중...");
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &self.console {
                    ui.label(RichText::new(line).monospace().color(Color32::from_rgb(165, 243, 252)));
                }
            });
    }

    fn draw_results(&mut self, ui: &mut egui::Ui) {
        let mut reload_day = false;
        ui.horizontal(|ui| {
            if ui.button("◀ 전날").clicked() {
                self.shift_result_day(-1);
            }
            if ui.button("다음날 ▶").clicked() {
                self.shift_result_day(1);
            }
            ui.label("날짜 (YYYY-MM-DD):");
            let resp = ui.text_edit_singleline(&mut self.result_day_text);
            if resp.lost_focus() {
                match NaiveDate::parse_from_str(self.result_day_text.trim(), "%Y-%m-%d") {
                    Ok(d) => {
                        self.result_day = d;
                        reload_day = true;
                    }
                    Err(_) => {
                        self.result_day_text = self.result_day.format("%Y-%m-%d").to_string();
                    }
                }
            }
            if ui.button("이력 새로고침").clicked() {
                reload_day = true;
            }
        });
        if reload_day {
            self.refresh_runs(true);
        }

        ui.horizontal(|ui| {
            ui.label("실행 시각:");
            let current = self.result_run.clone().unwrap_or_else(|| "-".to_string());
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_source("runs")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for name in &self.result_runs {
                        if ui.selectable_label(self.result_run.as_deref() == Some(name.as_str()), name).clicked() {
                            picked = Some(name.clone());
                        }
                    }
                });
            if let Some(name) = picked {
                self.result_run = Some(name);
                self.load_run();
            }
        });
        ui.separator();

        ui.columns(2, |cols| {
            egui::ScrollArea::vertical()
                .id_source("result_text")
                .auto_shrink([false, false])
                .show(&mut cols[0], |ui| {
                    ui.label(RichText::new(&self.result_text).monospace());
                });
            egui::ScrollArea::both()
                .id_source("result_csv")
                .auto_shrink([false, false])
                .show(&mut cols[1], |ui| {
                    egui::Grid::new("result_grid").striped(true).num_columns(8).show(ui, |ui| {
                        for h in ["구분", "주기", "장치", "항목", "대상", "순번", "상태", "비고"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for row in &self.result_rows {
                            for i in 0..8 {
                                let value = row.get(i).cloned().unwrap_or_default();
                                let color = if i == 6 { status_color(&value) } else { Color32::WHITE };
                                ui.label(RichText::new(value).color(color));
                            }
                            ui.end_row();
                        }
                    });
                });
        });
    }

    fn draw_logs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("검색:");
            ui.text_edit_singleline(&mut self.log_filter);
            if ui.button("누적파일 재동기화").clicked() {
                self.refresh_logs();
            }
        });
        let keyword = self.log_filter.to_lowercase();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in self.log_lines.iter().filter(|l| keyword.is_empty() || l.to_lowercase().contains(&keyword)) {
                    ui.label(RichText::new(line).monospace().color(Color32::from_rgb(56, 189, 248)));
                }
            });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        let drives: Vec<String> = self
            .snapshot
            .as_ref()
            .map(|s| s.disks.iter().map(|d| d.mount.clone()).collect())
            .unwrap_or_default();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.group(|ui| {
                ui.strong("감시 대상 프로세스 (한 줄에 하나)");
                ui.add(egui::TextEdit::multiline(&mut self.procs_text).desired_rows(5).desired_width(f32::INFINITY));
            });
            ui.group(|ui| {
                ui.strong("리소스 경고 임계치 및 감시 드라이브");
                egui::Grid::new("thr").num_columns(2).show(ui, |ui| {
                    ui.label("CPU (%)");
                    ui.add(egui::DragValue::new(&mut self.draft.thresholds.cpu_percent).range(10.0..=100.0));
                    ui.end_row();
                    ui.label("RAM (%)");
                    ui.add(egui::DragValue::new(&mut self.draft.thresholds.ram_percent).range(10.0..=100.0));
                    ui.end_row();
                    ui.label("DISK (%)");
                    ui.add(egui::DragValue::new(&mut self.draft.thresholds.disk_percent).range(10.0..=100.0));
                    ui.end_row();
                    ui.label("프로세스 수량 경보 기준(EA)");
                    ui.add(egui::DragValue::new(&mut self.draft.thresholds.proc_count_limit).range(50..=2000));
                    ui.end_row();
                    ui.label("기본 감시 드라이브");
                    egui::ComboBox::from_id_source("drive")
                        .selected_text(self.draft.thresholds.disk_drive.clone())
                        .show_ui(ui, |ui| {
                            for d in &drives {
                                ui.selectable_value(&mut self.draft.thresholds.disk_drive, d.clone(), d.clone());
                            }
                        });
                    ui.end_row();
                });
            });
            ui.group(|ui| {
                ui.strong("소리 알림 / 시간 동기화");
                ui.checkbox(&mut self.draft.system.sound_alert_enabled, "경고음 사용");
                egui::Grid::new("sys").num_columns(2).show(ui, |ui| {
                    ui.label("감시 주기(초)");
                    ui.add(egui::DragValue::new(&mut self.draft.system.interval_sec).range(1..=60));
                    ui.end_row();
                    ui.label("시간 동기화 명령어");
                    ui.add(egui::TextEdit::singleline(&mut self.draft.system.time_sync_cmd).desired_width(320.0));
                    ui.end_row();
                });
                if ui.button("지금 즉시 시간 동기화 실행").clicked() {
                    let cmd = self.draft.system.time_sync_cmd.clone();
                    self.time_sync_out = run_cmd_line(&cmd);
                    logs::append("TIME", &format!("동기화 결과: {}", self.time_sync_out));
                }
                if !self.time_sync_out.is_empty() {
                    ui.label(RichText::new(&self.time_sync_out).monospace());
                }
            });
            ui.group(|ui| {
                ui.strong("점검 배치 스크립트 경로 (.bat)");
                egui::Grid::new("bats").num_columns(2).show(ui, |ui| {
                    ui.label("FAS 일일점검 BAT");
                    ui.add(egui::TextEdit::singleline(&mut self.draft.bat_paths.daily_fas).desired_width(420.0));
                    ui.end_row();
                    ui.label("EAI 일일점검 BAT");
                    ui.add(egui::TextEdit::singleline(&mut self.draft.bat_paths.daily_eai).desired_width(420.0));
                    ui.end_row();
                });
            });
            if ui.button("설정 저장 (userset.json 즉시 반영)").clicked() {
                self.draft.target_processes = self
                    .procs_text
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                match config::save(&self.draft) {
                    Ok(()) => {
                        self.cfg = self.draft.clone();
                        logs::append("CONFIG", "userset.json 설정 저장 및 반영 완료");
                        self.set_saved_note();
                    }
                    Err(e) => logs::append("ERR", &format!("설정 저장 실패: {e}")),
                }
            }
        });
    }

    fn set_saved_note(&mut self) {
        self.alert_msg = "설정이 저장되었습니다".to_string();
    }

    fn draw_disk_window(&mut self, ctx: &egui::Context) {
        let disks = self.snapshot.as_ref().map(|s| s.disks.clone()).unwrap_or_default();
        let mut open = self.show_disks;
        egui::Window::new("시스템 하드디스크 볼륨별 사용 현황")
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                for d in disks {
                    ui.label(RichText::new(format!(
                        "{}  [{}]  전체 {:.1} GB / 사용 {:.1} GB / 잔여 {:.1} GB",
                        d.mount, d.fs, d.total_gb, d.used_gb, d.free_gb
                    )));
                    let color = if d.pct >= 90.0 { COLOR_ALERT } else { COLOR_OK };
                    ui.add(
                        egui::ProgressBar::new((d.pct / 100.0) as f32)
                            .text(format!("{:.1}%", d.pct))
                            .fill(color),
                    );
                    ui.add_space(6.0);
                }
            });
        self.show_disks = open;
    }

    fn draw_detail_window(&mut self, ctx: &egui::Context) {
        let Some(detail) = self.detail.clone() else { return };
        let mut open = true;
        egui::Window::new(format!("프로세스 실행 정보 - PID: {}", detail.pid))
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(format!("대상 프로세스: {} (PID: {})", detail.name, detail.pid));
                ui.label(format!(
                    "실행 파일 경로(EXE): {}",
                    if detail.exe.is_empty() { "접근 권한 없음 (관리자 권한 필요)" } else { &detail.exe }
                ));
                ui.label(format!("구동 시각 (epoch): {}", detail.start));
                ui.label("실행 명령줄:");
                let cmd = if detail.cmd.is_empty() { "명령줄 인수 없음 또는 접근 불가".to_string() } else { detail.cmd.clone() };
                ui.add(egui::TextEdit::multiline(&mut cmd.as_str()).desired_width(f32::INFINITY));
            });
        if !open {
            self.detail = None;
        }
    }
}

/// "cmd /C" 로 명령을 실행하고 표준 출력 또는 오류 문자열을 돌려준다.
fn run_cmd_line(line: &str) -> String {
    let out = Command::new("cmd").args(["/C", line]).output();
    match out {
        Ok(o) => {
            let text = batch::decode_text(&o.stdout).trim().to_string();
            if !text.is_empty() {
                text
            } else {
                let err = batch::decode_text(&o.stderr).trim().to_string();
                if err.is_empty() { "명령이 실행되었습니다.".to_string() } else { err }
            }
        }
        Err(e) => format!("실행 실패: {e}"),
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_bat();
        let interval = Duration::from_secs(self.cfg.system.interval_sec.max(1));
        if self.last_tick.elapsed() >= interval {
            self.last_tick = Instant::now();
            self.tick();
        }
        ctx.request_repaint_after(Duration::from_millis(250));

        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen(ctx);
        }

        let mut visuals = egui::Visuals::dark();
        if self.alert {
            visuals.panel_fill = Color32::from_rgb(127, 29, 29);
            visuals.window_fill = Color32::from_rgb(127, 29, 29);
        }
        ctx.set_visuals(visuals);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            self.draw_top(ui, ctx);
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("© 2026 Studio Airbook. All rights reserved.").small().color(COLOR_TEXT_DIM));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Dashboard => self.draw_dashboard(ui),
            Tab::Process => self.draw_process(ui),
            Tab::Bat => self.draw_bat(ui),
            Tab::Results => self.draw_results(ui),
            Tab::Logs => self.draw_logs(ui),
            Tab::Settings => self.draw_settings(ui),
        });

        if self.show_disks {
            self.draw_disk_window(ctx);
        }
        self.draw_detail_window(ctx);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.stop_beep();
    }
}
