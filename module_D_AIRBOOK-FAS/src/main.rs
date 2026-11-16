//! AIRBOOK-FAS & EAI 서버 관제 (Rust 버전). Python 원본(CheckServer.py)의 기능을 따른다.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod batch;
mod config;
mod logs;
mod monitor;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Studio Airbook - FAS & EAI Server Guard")
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CheckServer",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
