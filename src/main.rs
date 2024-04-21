#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use std::env;
use eframe::egui;
use valorite::{constants::*, splash_screen::SplashScreen};

fn main() -> Result<(), eframe::Error> {
    let key = "RUST_LOG";
    env::set_var(key, "info");
    assert_eq!(env::var(key), Ok("info".to_string()));
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([512.0, 512.0])
            .with_resizable(false)
            .with_maximize_button(false),
        ..Default::default()
    };
    
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::<SplashScreen>::default()
        }),
    )
}