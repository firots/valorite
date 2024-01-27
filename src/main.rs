#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{fmt::format, fs::{self, File}, io::{self, Read}, path::Path, process, process::Command};
use eframe::egui;
use serde::{Serialize, Deserialize};
use reqwest;
use md5;


const CUO_FILES_URL: &str  = "http://valor.gen.tr/cuo_files_win/";
const UPDATE_FILE_NAME: &str  = "update.json";
const COMPRESSION_EXTENSION: &str = ".zip";


// Models
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UpdateResponse {
    launcher_build: u32,
    files: Vec<UpdateFile>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UpdateFile {
    local_path: String,
    hash: String
}

// UI
fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([512.0, 512.0])
            .with_resizable(false)
            .with_maximize_button(false),
        ..Default::default()
    };
    eframe::run_native(
        "Valor Launcher",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Box::<SplashScreen>::default()
        }),
    )
}

struct SplashScreen {
    message: String,
    percentage: u32,
    update_check_started: bool,
    remote_launcher_build: u32,
    local_launcher_build: u32
}

impl Default for SplashScreen {
    fn default() -> Self {
        Self {
            message: "Guncellemeler denetleniyor...".to_owned(),
            percentage: 0,
            update_check_started: false,
            local_launcher_build: 1,
            remote_launcher_build: 0
        }
    }
}

impl eframe::App for SplashScreen {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.update_check_started {
                self.start_update_check();
            }
            ui.image(egui::include_image!(
                "../resources/valor_splash.png"
            ));
            ui.add_space(2.0);
            ui.label(format!("{}", self.message));
        });
    }
}

// Business
impl SplashScreen {
    fn start_update_check(&mut self) {
        self.update_check_started = true;
        let response: reqwest::blocking::Response = reqwest::blocking::get(CUO_FILES_URL.to_owned() + UPDATE_FILE_NAME).unwrap();
        let update_response: UpdateResponse = response.json().unwrap();
        println!("{:?}", update_response);
        self.filter_update_response(update_response);
    }

    fn filter_update_response(&self, update_response: UpdateResponse) {
        let mut files_to_download: Vec<String> = Vec::new();

        for update_file in update_response.files.iter() {
            let path: &Path = Path::new(&update_file.local_path);
            if path.exists() {
                let mut f = File::open(path).unwrap();
                let mut contents = Vec::<u8>::new();
                f.read_to_end(&mut contents).unwrap();
                let digest = md5::compute(&contents.as_slice());
                let local_hash = format!("{:x}", digest).to_uppercase();
                if local_hash == update_file.hash {
                    continue;
                }
            }
            files_to_download.push(update_file.local_path.to_owned());
            // path.file_name().unwrap().to_str().unwrap().to_owned()
        }								
        println!("{:?}", files_to_download);
        self.download_files(files_to_download)
    }

    fn download_files(&self, files_to_download: Vec<String>) {
        self.update_launcher_if_needed()
    }

    fn update_launcher_if_needed(&self) {
        if self.local_launcher_build >= self.remote_launcher_build {
            self.start_game()
        } else {
            self.update_launcher()
        }
    }

    fn update_launcher(&self) {

    }

    fn start_game(&self) {
        /*Command::new("valor_cuo/ClassicUO.exe")
        .current_dir("valor_cuo/")
        .spawn()
        .unwrap();
        process::exit(0x0100);*/
    }
}
