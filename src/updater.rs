use std::{env, error::Error, fs::{self, File}, io::{self, Read}, os::unix::fs::PermissionsExt, path::Path, process::{self, Command}, sync::mpsc::Sender};
use crate::constants::*;
use downloader::Downloader;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;
use log::{error, info};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UpdateResponse {
    launcher: UpdateFile,
    files: Vec<UpdateFile>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFile {
    local_path: String,
    hash: String
}

pub struct Updater {
    pub(crate) launcher_update_file: Option<UpdateFile>,
    pub(crate) sender: Sender<String>,
    pub(crate) ctx: egui::Context
}

impl Updater {
    pub fn start(&mut self) {
        self.send_message(CHECKING_FOR_UPDATES.to_owned());
        match self.get_update_file() {
            Ok(update_response) => self.process_update_response(update_response),
            Err(error) => {
                error!("{}: {}", UPDATE_FILE_NAME, error);
                match self.start_game() {
                    Ok(_) => (),
                    Err(e) => error!("Failed to start game: {}", e),
                }
            },
        };
    }

    fn send_message(&self, message: String) {
        self.sender.send(message).unwrap();
        self.ctx.request_repaint();
    }

    fn get_update_file(&self) -> Result<UpdateResponse, Box<dyn Error>> {
        let response = reqwest::blocking::get(CUO_FILES_URL.to_owned() + "/" + UPDATE_FILE_NAME)?;
        let body = response.text()?;
        let update_response = serde_json::from_str(&body)?;
        Ok(update_response)
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
        self.download_files(files_to_download)
    }

    fn get_md5(&self, local_path: &str) -> Option<String> {
        let path_string = &(self.get_current_folder() + &local_path);
        let path: &Path = Path::new(path_string);
        if path.exists() {
            info!("{:?}", std::env::current_dir());
            info!("Local path exists {:?}", local_path);
            let mut f = File::open(path).unwrap();
            let mut contents = Vec::<u8>::new();
            f.read_to_end(&mut contents).unwrap();
            let digest = md5::compute(&contents.as_slice());
            let hash = format!("{:x}", digest);
            return Some(hash)
        }
        info!("Local path not exists {:?}", local_path);
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
            if let Some(local_launcher_hash) = self.get_md5(LAUNCHER_EXECUTABLE_PATH) {
                if launcher_update_file.hash.to_lowercase() != local_launcher_hash {
                    self.update_launcher();
                    return
                }
            }
        }
        match self.start_game() {
            Ok(_) => (),
            Err(e) => error!("Failed to start game: {}", e),
        }
    }

    fn update_launcher(&mut self) {
        if let Some(update_file) = &self.launcher_update_file {
            self.download_file(&update_file.local_path);
            if let Some(new_hash) = self.get_md5(&update_file.local_path) {
                if new_hash == update_file.hash.to_lowercase() {
                    let new_launcher_path = self.get_current_folder() + &update_file.local_path;
                    let _ = self_replace::self_replace(new_launcher_path);
                    let _ = std::fs::remove_file(&update_file.local_path);
                }
            }
        }
        match self.start_game() {
            Ok(_) => (),
            Err(e) => error!("Failed to start game: {}", e),
        }
    }

    fn start_game(&mut self) -> std::io::Result<()> {
        info!("Starting the game");
        self.send_message(STARTING_GAME.to_owned());
    
        if std::env::consts::OS == "macos" {
            info!("Starting the game");
            let current_folder = self.get_current_folder();
            let file_path = current_folder.to_owned() + "/" + CLIENT_BINARY;
            let mut perms = fs::metadata(&file_path)?.permissions();
            perms.set_mode(0o755); // User read/write/execute, Group and Others read/execute
            fs::set_permissions(&file_path, perms)?;
    
            Command::new(current_folder + "/" + CLIENT_BINARY)
                .arg("-uopath")
                .arg(self.get_current_folder())
                .arg("-clientversion")
                .arg(CLIENT_VERSION.to_owned())
                .spawn()?;

            self.send_message(HIDE_WINDOW_MESSAGE.to_owned());
        } else {
            Command::new(CLIENT_FOLDER_PATH.to_owned() + CLIENT_BINARY)
                .current_dir(CLIENT_FOLDER_PATH)
                .spawn()?;
            process::exit(0x0100);    
        }
    
        Ok(())
    }

    fn get_current_folder(&self) -> String {
        if std::env::consts::OS == "macos" {
            let exe_path = env::current_exe().expect("Failed to get current exe path");
            let parent_dir = exe_path.parent().expect("Failed to get parent directory");
            parent_dir.to_str().expect("Failed to convert path to string").to_owned()
        } else {
            ".".to_owned()
        }
    }

    fn download_file(&self, file_path: &str) {
        self.send_message(DOWNLOADING_FILE.to_owned() + &file_path);
        let file_url = &(CUO_FILES_URL.to_owned() + &file_path + COMPRESSION_EXTENSION);
        let path = Path::new(&file_path);
        let mut download_path = path.parent().and_then(|parent| parent.to_str());
        if download_path == None {
            download_path = Some("/");
        } else {
            fs::create_dir_all(self.get_current_folder() + download_path.unwrap())
                .expect("Failed to create");
        }

        info!("file_url: {}", file_url);
        info!("download_path: {}", download_path.unwrap());

        let zip_path_string = self.get_current_folder() + &file_path + COMPRESSION_EXTENSION;
        let zip_path  = std::path::Path::new(&zip_path_string);
        let _ = fs::remove_file(zip_path);
        
        let mut downloader = Downloader::builder()
            .download_folder(std::path::Path::new(&(self.get_current_folder() + download_path.unwrap())))
            .parallel_requests(1)
            .build()
            .unwrap();
    
        let dl = downloader::Download::new(&file_url);
        let result = downloader.download(&[dl]).unwrap();
    
        for r in result {
            match r {
                Err(download_error) => error!("{download_error}"),
                Ok(download_summary) => {
                    self.send_message(UPDATING_FILE.to_owned() + &file_path);
                    let extract_path_string = self.get_current_folder() + download_path.unwrap();
                    let extract_path = std::path::Path::new(&extract_path_string);
                    if let Err(zip_error) = self.unzip_file(zip_path, extract_path) {
                        error!("{zip_error}");
                    }
                    info!("{download_summary}");
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