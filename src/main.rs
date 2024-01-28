#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use std::{fs, io::{self, Read}, path::Path, process, process::Command};
use std::fs::File;
use eframe::egui;
use serde::{Serialize, Deserialize};
use reqwest;
use md5;
use guard::guard;
use zip::read::ZipArchive;
use downloader::Downloader;

const CUO_FILES_URL: &str  = "http://valor.gen.tr/cuo_files_win";
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
    view_did_load: bool,
    remote_launcher_build: u32,
    local_launcher_build: u32
}

impl Default for SplashScreen {
    fn default() -> Self {
        Self {
            message: "Guncellemeler denetleniyor...".to_owned(),
            percentage: 0,
            view_did_load: false,
            local_launcher_build: 1,
            remote_launcher_build: 0
        }
    }
}

impl eframe::App for SplashScreen {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.view_did_load {
                self.view_did_load();
                self.view_did_load = true
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
    fn view_did_load(&mut self) {
        self.start_update_check()
    }

    fn start_update_check(&mut self) {
        guard!(let Ok(response) = reqwest::blocking::get(CUO_FILES_URL.to_owned() + "/" + UPDATE_FILE_NAME) else { 
            println!("Cannot fetch the update.json");
            self.start_game();
            return 
        });

        guard!(let Ok(body) = response.text() else { 
            println!("Cannot get body for update.json.");
            self.start_game();
            return 
        });

        guard!(let Ok(update_response) = serde_json::from_str(&body) else { 
            println!("Cannot get body for update.json.");
            self.start_game();
            return 
        });
        self.process_update_response(update_response);
    }

    fn process_update_response(&mut self, update_response: UpdateResponse) {
        self.remote_launcher_build = update_response.launcher_build;
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

    fn download_files(&mut self, files_to_download: Vec<String>) {
        for file_path in files_to_download.iter() {
            self.download_file(file_path.to_string())
        }
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
        Command::new("valor_cuo/ClassicUO.exe")
        .current_dir("valor_cuo/")
        .spawn()
        .unwrap();
        process::exit(0x0100);
    }

    fn download_file(&mut self, file_path: String) {
        self.message = "Indiriliyor: ".to_owned() + &file_path;
        let file_url = &(CUO_FILES_URL.to_owned() + &file_path + COMPRESSION_EXTENSION);
        let path = Path::new(&file_path);
        let mut download_path = path.parent().and_then(|parent| parent.to_str());
        if download_path == None {
            download_path = Some("/");
        } else {
            fs::create_dir_all(".".to_owned() + download_path.unwrap())
                .expect("Failed to create");
        }

        println!("file_url: {}", file_url);
        println!("download_path: {}", download_path.unwrap());

        let zip_path_string = ".".to_owned() + &file_path + COMPRESSION_EXTENSION;
        let zip_path  = std::path::Path::new(&zip_path_string);
        let _ = fs::remove_file(zip_path);
        
        let mut downloader = Downloader::builder()
            .download_folder(std::path::Path::new(&(".".to_owned() + download_path.unwrap())))
            .parallel_requests(1)
            .build()
            .unwrap();
    
        let dl = downloader::Download::new(&file_url);
        let result = downloader.download(&[dl]).unwrap();
    
        for r in result {
            match r {
                Err(e) => println!("Error: {}", e.to_string()),
                Ok(s) => {
                    let extract_path_string = ".".to_owned() + download_path.unwrap();
                    let extract_path = std::path::Path::new(&extract_path_string);
                    let _ = self.unzip_file(zip_path, extract_path);
                    println!("Success: {}", s)
                },
            };
        }
    }
    
    fn unzip_file(&self, zip_path: &Path, extract_path: &Path) -> Result<(), io::Error> {
        let file = File::open(zip_path)?;
        let reader = io::BufReader::new(file);
        let mut archive = ZipArchive::new(reader)?;
    
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = extract_path.join(file.sanitized_name());
    
            if (&*file.name()).ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(&p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }

        fs::remove_file(zip_path)?;
    
        Ok(())
    }
}