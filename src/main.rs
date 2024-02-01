#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use std::{fs, io::{self, Read}, path::Path, process, process::Command, sync::mpsc::{self, Sender, Receiver}, thread};
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
const LAUNCHER_EXECUTABLE_PATH: &str = "/valorite.exe";

// Models
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UpdateResponse {
    launcher: UpdateFile,
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
    view_did_load: bool,
    receiver: Receiver<String>,
    sender: Sender<String>
}

impl Default for SplashScreen {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            message: "Guncellemeler denetleniyor...".to_owned(),
            view_did_load: false,
            sender: tx,
            receiver: rx
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
            self.receive_message();
            ui.image(egui::include_image!(
                "../resources/valor_splash.png"
            ));
            ui.add_space(2.0);
            ui.label(format!("{}", self.message));
        });
    }
}

impl SplashScreen {
    fn view_did_load(&mut self) {
        let sender = self.sender.clone();
        let _ = thread::spawn(move || {
            let mut updater = Updater {
                message_sender: sender,
                launcher_update_file: None
            };
            updater.start_update_check();
        });
    }

    fn receive_message(&mut self) {
        match self.receiver.recv() {
            Ok(message) => {
                self.message = message;
            }
            Err(_) => {
            }
        }
    }
}

// Business
struct Updater {
    launcher_update_file: Option<UpdateFile>,
    message_sender: Sender<String>
}

impl Updater {
    fn start_update_check(&mut self) {
        self.message_sender.send("Guncellemeler denetleniyor...".to_owned()).unwrap();
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
        let mut files_to_download: Vec<String> = Vec::new();
        self.launcher_update_file = Some(update_response.launcher);

        for update_file in update_response.files.iter() {
            let local_hash = self.get_md5(&update_file.local_path);
            if let Some(local_hash_unwrapped) = local_hash {
                if local_hash_unwrapped == update_file.hash.to_lowercase() {
                    continue;
                }
            }
            files_to_download.push(update_file.local_path.to_owned());
        }								
        println!("{:?}", files_to_download);
        self.download_files(files_to_download)
    }

    fn get_md5(&self, local_path: &str) -> Option<String> {
        let path_string = &(".".to_owned() + &local_path);
        let path: &Path = Path::new(path_string);

        if path.exists() {
            let mut f = File::open(path).unwrap();
            let mut contents = Vec::<u8>::new();
            f.read_to_end(&mut contents).unwrap();
            let digest = md5::compute(&contents.as_slice());
            let hash = format!("{:x}", digest);
            return Some(hash)
        }
        return None
    }

    fn download_files(&mut self, files_to_download: Vec<String>) {
        for file_path in files_to_download.iter() {
            self.download_file(file_path)
        }
        self.update_launcher_if_needed()
    }

    fn update_launcher_if_needed(&mut self) {
        if let Some(launcher_update_file) = &self.launcher_update_file {
            println!("launcher_update_file_hash: {}", launcher_update_file.hash);
            if let Some(local_launcher_hash) = self.get_md5(LAUNCHER_EXECUTABLE_PATH) {
                println!("local_launcher_hash: {}", local_launcher_hash);
                if launcher_update_file.hash.to_lowercase() != local_launcher_hash {
                    self.update_launcher();
                    return
                }
            }
        }
        self.start_game();
    }

    fn update_launcher(&mut self) {
        if let Some(update_file) = &self.launcher_update_file {
            println!("update 1");
            self.download_file(&update_file.local_path);
            if let Some(new_hash) = self.get_md5(&update_file.local_path) {
                println!("update 2");
                println!("new_hash: {}", new_hash);
                println!("update_file_hash: {}", update_file.hash);
                if new_hash == update_file.hash.to_lowercase() {
                    println!("call self replace: {}", update_file.hash);
                    let new_launcher_path = ".".to_owned() + &update_file.local_path;
                    let _ = self_replace::self_replace(new_launcher_path);
                    let _ = std::fs::remove_file(&update_file.local_path);
                }
            }
        }
        self.start_game()
    }

    fn start_game(&mut self) {
        println!("Starting the game...");
        self.message_sender.send("Oyun baslatiliyor...".to_owned()).unwrap();
        Command::new("valor_cuo/cuo.exe")
        .current_dir("valor_cuo/")
        .spawn()
        .unwrap();
        process::exit(0x0100);
    }

    fn download_file(&self, file_path: &str) {
        self.message_sender.send("Indiriliyor: ".to_owned() + &file_path).unwrap();
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
                    self.message_sender.send("Guncelleniyor: ".to_owned() + &file_path).unwrap();
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
            let outpath = extract_path.join(file.name());
    
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