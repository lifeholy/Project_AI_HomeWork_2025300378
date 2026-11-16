//! FSC400 초음파 유량계 실시간 관제 (Rust 버전).

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod protocol;
mod sources;
mod storage;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FSC400 초음파 유량계 실시간 관제")
            .with_inner_size([1440.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "FSC400",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
