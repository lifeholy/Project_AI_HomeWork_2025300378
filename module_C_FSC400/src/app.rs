//! 관제 GUI (egui). 수신 스레드 → 채널 → GUI 순서로 데이터가 흐른다.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use chrono::{DateTime, Datelike, Local, NaiveDate};
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};

use crate::protocol::Sample;
use crate::sources::{self, Level, SourceConfig, SourceKind};
use crate::storage::{self, Recorder, Settings};

const LIVE_POINTS: usize = 300;
const LIVE_TABLE_MAX: usize = 200;
const PAGE_SIZE: usize = 500;
const SOURCE_LABELS: [&str; 3] = ["MODBUS-RTU", "ASCII 라인", "시뮬레이션"];
const COLOR_LIVE: Color32 = Color32::from_rgb(0, 188, 140);
const COLOR_HISTORY: Color32 = Color32::from_rgb(52, 152, 219);

struct HistoryView {
    day: NaiveDate,
    samples: Vec<Sample>,
    times: Vec<f64>,
    page: usize,
    selected: Option<usize>,
    scroll_pending: bool,
}

pub struct App {
    base: PathBuf,
    src_idx: usize,
    port_text: String,
    baud_text: String,
    slave_text: String,
    interval_text: String,

    tx: mpsc::Sender<Sample>,
    rx: mpsc::Receiver<Sample>,
    status_tx: mpsc::Sender<(String, Level)>,
    status_rx: mpsc::Receiver<(String, Level)>,
    stop: Option<Arc<AtomicBool>>,

    status: (String, Level),
    live_points: VecDeque<(f64, f64)>,
    live_samples: VecDeque<Sample>,
    last: Option<Sample>,
    history: Option<HistoryView>,
    recorder: Recorder,
    cal_year: i32,
    cal_month: u32,
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

fn level_color(level: Level) -> Color32 {
    match level {
        Level::Info => Color32::GRAY,
        Level::Ok => Color32::from_rgb(56, 189, 248),
        Level::Warn => Color32::from_rgb(245, 158, 11),
        Level::Err => Color32::from_rgb(239, 68, 68),
    }
}

fn fmt_time(secs: f64, with_seconds: bool) -> String {
    DateTime::from_timestamp(secs as i64, 0)
        .map(|d| {
            let local = d.with_timezone(&Local);
            if with_seconds {
                local.format("%H:%M:%S").to_string()
            } else {
                local.format("%m-%d %H:%M").to_string()
            }
        })
        .unwrap_or_default()
}

fn card(ui: &mut egui::Ui, title: &str, value: &str, color: Color32) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(title).color(color));
        ui.label(RichText::new(value).size(26.0).monospace().color(color));
    });
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        let base = storage::base_dir();
        let settings = storage::load_settings(&base);
        let (tx, rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();
        let src_idx = SOURCE_LABELS
            .iter()
            .position(|l| *l == settings.source)
            .unwrap_or(0);
        let today = Local::now().date_naive();
        Self {
            recorder: Recorder::new(base.clone()),
            base,
            src_idx,
            port_text: settings.port,
            baud_text: settings.baud.to_string(),
            slave_text: settings.slave.to_string(),
            interval_text: settings.interval.to_string(),
            tx,
            rx,
            status_tx,
            status_rx,
            stop: None,
            status: ("중지됨".to_string(), Level::Info),
            live_points: VecDeque::new(),
            live_samples: VecDeque::new(),
            last: None,
            history: None,
            cal_year: today.year(),
            cal_month: today.month(),
        }
    }

    fn set_status(&mut self, text: impl Into<String>, level: Level) {
        self.status = (text.into(), level);
    }

    fn is_running(&self) -> bool {
        self.stop.is_some()
    }

    fn start(&mut self) {
        let kind = match self.src_idx {
            0 => SourceKind::Modbus,
            1 => SourceKind::Ascii,
            _ => SourceKind::Simulation,
        };
        let baud = match self.baud_text.trim().parse::<u32>() {
            Ok(v) => v,
            Err(_) => return self.set_status("보드레이트는 숫자여야 합니다", Level::Err),
        };
        let slave = match self.slave_text.trim().parse::<u8>() {
            Ok(v) if (1..=247).contains(&v) => v,
            _ => return self.set_status("슬레이브 주소는 1~247 이어야 합니다", Level::Err),
        };
        let interval = match self.interval_text.trim().parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            _ => return self.set_status("주기는 0보다 큰 숫자여야 합니다", Level::Err),
        };
        let port = self.port_text.trim().to_string();

        self.settings_store(Settings {
            source: SOURCE_LABELS[self.src_idx].to_string(),
            port: port.clone(),
            baud,
            slave,
            interval,
        });

        let stop = Arc::new(AtomicBool::new(false));
        sources::spawn(
            SourceConfig { kind, port, baud, slave, interval },
            self.tx.clone(),
            self.status_tx.clone(),
            stop.clone(),
        );
        self.stop = Some(stop);
        self.set_status("연결 중...", Level::Warn);
    }

    fn settings_store(&self, s: Settings) {
        storage::save_settings(&self.base, &s);
    }

    fn stop_worker(&mut self) {
        if let Some(flag) = self.stop.take() {
            flag.store(true, Ordering::SeqCst);
        }
        self.set_status("중지됨", Level::Info);
    }

    fn drain_channels(&mut self) {
        while let Ok((text, level)) = self.status_rx.try_recv() {
            self.set_status(text, level);
        }
        while let Ok(sample) = self.rx.try_recv() {
            self.on_sample(sample);
        }
    }

    fn on_sample(&mut self, s: Sample) {
        self.live_points.push_back((storage::epoch_secs(&s.timestamp), s.flow));
        if self.live_points.len() > LIVE_POINTS {
            self.live_points.pop_front();
        }
        if let Err(e) = self.recorder.write(&s) {
            self.set_status(format!("CSV 기록 실패: {e}"), Level::Err);
        }
        self.live_samples.push_back(s.clone());
        if self.live_samples.len() > LIVE_TABLE_MAX {
            self.live_samples.pop_front();
        }
        self.last = Some(s);
    }

    fn open_history(&mut self, day: NaiveDate) {
        let path = storage::log_path(&self.base, day);
        match storage::read_day(&path) {
            Ok(samples) if !samples.is_empty() => {
                let times = samples.iter().map(|s| storage::epoch_secs(&s.timestamp)).collect();
                let n = samples.len();
                self.history = Some(HistoryView {
                    day,
                    samples,
                    times,
                    page: 0,
                    selected: None,
                    scroll_pending: false,
                });
                self.set_status(format!("과거 기록 {day}: {n}건"), Level::Info);
            }
            Ok(_) => self.set_status(format!("{day} 기록이 비어 있습니다"), Level::Warn),
            Err(e) => self.set_status(format!("기록 파일을 읽을 수 없습니다: {e}"), Level::Err),
        }
    }

    fn jump_to_secs(&mut self, x: f64) {
        if let Some(h) = self.history.as_mut() {
            if h.times.is_empty() {
                return;
            }
            let mut idx = h.times.partition_point(|&t| t < x).min(h.times.len() - 1);
            if idx > 0 && (h.times[idx - 1] - x).abs() < (h.times[idx] - x).abs() {
                idx -= 1;
            }
            h.page = idx / PAGE_SIZE;
            h.selected = Some(idx);
            h.scroll_pending = true;
        }
    }

    fn draw_side(&mut self, ui: &mut egui::Ui) {
        let (flow, total, vel, err, time) = match &self.last {
            Some(s) => (
                format!("{:.3}", s.flow),
                format!("{:.3}", s.total),
                format!("{:.2}", s.velocity),
                s.error.clone(),
                format!("마지막 수신: {}", s.timestamp.format("%Y-%m-%d %H:%M:%S")),
            ),
            None => ("-".into(), "-".into(), "-".into(), "-".into(), "마지막 수신: -".into()),
        };
        card(ui, "순시 유량 (m³/h)", &flow, COLOR_LIVE);
        card(ui, "적산 유량 (m³)", &total, Color32::from_rgb(93, 214, 120));
        card(ui, "유속 (m/s)", &vel, Color32::from_rgb(96, 165, 250));
        card(ui, "에러 코드", &err, Color32::GRAY);
        ui.label(RichText::new(time).small().color(Color32::GRAY));
        ui.add_space(8.0);
        self.draw_calendar(ui);
    }

    fn draw_calendar(&mut self, ui: &mut egui::Ui) {
        ui.heading("과거 기록 달력");
        ui.horizontal(|ui| {
            if ui.button("◀").clicked() {
                self.shift_month(-1);
            }
            ui.label(format!("{}년 {:02}월", self.cal_year, self.cal_month));
            if ui.button("▶").clicked() {
                self.shift_month(1);
            }
        });
        let Some(first) = NaiveDate::from_ymd_opt(self.cal_year, self.cal_month, 1) else {
            return;
        };
        let next = if self.cal_month == 12 {
            NaiveDate::from_ymd_opt(self.cal_year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(self.cal_year, self.cal_month + 1, 1)
        };
        let Some(next) = next else { return };
        let days = (next - first).num_days() as u32;
        let start_col = first.weekday().num_days_from_sunday() as usize;
        let today = Local::now().date_naive();

        let mut clicked: Option<NaiveDate> = None;
        egui::Grid::new("calendar").num_columns(7).spacing([4.0, 4.0]).show(ui, |ui| {
            for name in ["일", "월", "화", "수", "목", "금", "토"] {
                ui.label(name);
            }
            ui.end_row();
            let mut col = 0usize;
            for _ in 0..start_col {
                ui.label("  ");
                col += 1;
            }
            for day in 1..=days {
                let date = NaiveDate::from_ymd_opt(self.cal_year, self.cal_month, day).unwrap_or(first);
                let exists = storage::log_path(&self.base, date).exists();
                let enabled = date <= today && (date < today || exists);
                let fill = if exists {
                    COLOR_HISTORY
                } else {
                    Color32::from_gray(70)
                };
                let btn = egui::Button::new(RichText::new(day.to_string()).color(Color32::WHITE)).fill(fill);
                if ui.add_enabled(enabled, btn).clicked() {
                    clicked = Some(date);
                }
                col += 1;
                if col % 7 == 0 {
                    ui.end_row();
                }
            }
        });
        if let Some(date) = clicked {
            self.open_history(date);
        }
    }

    fn shift_month(&mut self, delta: i32) {
        let m = self.cal_month as i32 + delta;
        self.cal_year += (m - 1).div_euclid(12);
        self.cal_month = ((m - 1).rem_euclid(12) + 1) as u32;
    }

    fn draw_chart(&mut self, ui: &mut egui::Ui) {
        let is_history = self.history.is_some();
        let (title, points, color, with_seconds) = match &self.history {
            Some(h) => (
                format!("{} 유량 이력 (차트 클릭 시 목록 연동)", h.day),
                h.times
                    .iter()
                    .zip(h.samples.iter())
                    .map(|(t, s)| [*t, s.flow])
                    .collect::<Vec<[f64; 2]>>(),
                COLOR_HISTORY,
                false,
            ),
            None => (
                format!("실시간 순시 유량 (최근 {}개)", self.live_points.len()),
                self.live_points.iter().map(|(t, f)| [*t, *f]).collect::<Vec<[f64; 2]>>(),
                COLOR_LIVE,
                true,
            ),
        };
        ui.heading(title);

        let mut clicked_x: Option<f64> = None;
        Plot::new("flow_plot")
            .legend(Legend::default())
            .height(ui.available_height())
            .x_axis_formatter(move |mark, _range| fmt_time(mark.value, with_seconds))
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(PlotPoints::from(points)).name("순시 유량").color(color));
                if is_history && plot_ui.response().clicked() {
                    clicked_x = plot_ui.pointer_coordinate().map(|p| p.x);
                }
            });
        if let Some(x) = clicked_x {
            self.jump_to_secs(x);
        }
    }

    fn draw_table(&mut self, ui: &mut egui::Ui) {
        let rows: Vec<Sample> = match &self.history {
            Some(h) => {
                let start = h.page * PAGE_SIZE;
                h.samples.iter().skip(start).take(PAGE_SIZE).cloned().collect()
            }
            None => self.live_samples.iter().rev().cloned().collect(),
        };
        let page_index = self.history.as_ref().map(|h| h.page);
        let selected_in_page = self.history.as_ref().and_then(|h| {
            h.selected
                .filter(|i| i / PAGE_SIZE == h.page)
                .map(|i| i % PAGE_SIZE)
        });
        let scroll = self.history.as_ref().map(|h| h.scroll_pending).unwrap_or(false);
        let mut scrolled = false;

        ui.horizontal(|ui| {
            if let Some(h) = self.history.as_ref() {
                ui.label(RichText::new(format!("과거 기록 모드: {}", h.day)).strong().color(COLOR_HISTORY));
            } else {
                ui.label(RichText::new("실시간 모드").strong().color(COLOR_LIVE));
            }
            if let Some(page) = page_index {
                let total_pages = self
                    .history
                    .as_ref()
                    .map(|h| h.samples.len().div_ceil(PAGE_SIZE).max(1))
                    .unwrap_or(1);
                if ui.add_enabled(page > 0, egui::Button::new("◀ 이전")).clicked() {
                    if let Some(h) = self.history.as_mut() {
                        h.page -= 1;
                        h.selected = None;
                    }
                }
                ui.label(format!("{} / {} 페이지", page + 1, total_pages));
                if ui.add_enabled(page + 1 < total_pages, egui::Button::new("다음 ▶")).clicked() {
                    if let Some(h) = self.history.as_mut() {
                        h.page += 1;
                        h.selected = None;
                    }
                }
            }
            if self.history.is_some() && ui.button("실시간으로 돌아가기").clicked() {
                self.history = None;
            }
        });

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("data_grid").striped(true).num_columns(6).show(ui, |ui| {
                for h in ["시간", "헤더", "순시 유량(m³/h)", "적산 유량(m³)", "유속(m/s)", "에러 코드"] {
                    ui.strong(h);
                }
                ui.end_row();
                for (i, s) in rows.iter().enumerate() {
                    let is_sel = selected_in_page == Some(i);
                    let text = |t: String| {
                        if is_sel {
                            RichText::new(t).color(Color32::YELLOW)
                        } else {
                            RichText::new(t)
                        }
                    };
                    let first = ui.label(text(s.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()));
                    if is_sel && scroll && !scrolled {
                        first.scroll_to_me(Some(egui::Align::Center));
                        scrolled = true;
                    }
                    ui.label(text(s.header.clone()));
                    ui.label(text(format!("{:.3}", s.flow)));
                    ui.label(text(format!("{:.3}", s.total)));
                    ui.label(text(format!("{:.2}", s.velocity)));
                    ui.label(text(s.error.clone()));
                    ui.end_row();
                }
            });
        });
        if scrolled {
            if let Some(h) = self.history.as_mut() {
                h.scroll_pending = false;
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_channels();
        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("📊 FSC400 초음파 유량계 통합 관제");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, level) = self.status.clone();
                    ui.label(RichText::new(text).color(level_color(level)));
                });
            });
            ui.horizontal(|ui| {
                ui.label("소스");
                egui::ComboBox::from_id_source("source")
                    .selected_text(SOURCE_LABELS[self.src_idx])
                    .show_ui(ui, |ui| {
                        for (i, label) in SOURCE_LABELS.iter().enumerate() {
                            ui.selectable_value(&mut self.src_idx, i, *label);
                        }
                    });
                ui.label("포트");
                ui.add(egui::TextEdit::singleline(&mut self.port_text).desired_width(80.0));
                ui.label("보드레이트");
                ui.add(egui::TextEdit::singleline(&mut self.baud_text).desired_width(70.0));
                ui.label("슬레이브");
                ui.add(egui::TextEdit::singleline(&mut self.slave_text).desired_width(40.0));
                ui.label("주기(초)");
                ui.add(egui::TextEdit::singleline(&mut self.interval_text).desired_width(50.0));
                let label = if self.is_running() { "■ 중지" } else { "▶ 시작" };
                let color = if self.is_running() {
                    Color32::from_rgb(239, 68, 68)
                } else {
                    Color32::from_rgb(34, 139, 34)
                };
                if ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(color)).clicked() {
                    if self.is_running() {
                        self.stop_worker();
                    } else {
                        self.start();
                    }
                }
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("side").exact_width(400.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.draw_side(ui));
        });

        egui::TopBottomPanel::bottom("table")
            .resizable(true)
            .default_height(280.0)
            .show(ctx, |ui| self.draw_table(ui));

        egui::CentralPanel::default().show(ctx, |ui| self.draw_chart(ui));
    }

}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(flag) = self.stop.take() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}
