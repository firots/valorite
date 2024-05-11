use std::{env, error::Error, fs::{self, File}, io::{self, Read}, path::Path, process::{self, Command}, sync::mpsc::Sender, time::SystemTime};
use crate::constants::*;
use downloader::Downloader;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;
use log::{error, info};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UpdateResponse {
    launcher: UpdateFile,
    files: Vec<UpdateFile>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFile {
    local_path: String,
    hash: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCacheFile {
    file_path: String,
    last_modified: SystemTime,
    md5: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCache {
    pub(crate) files: Vec<UpdateCacheFile>
}

pub struct Updater {
    launcher_update_file: Option<UpdateFile>,
    update_cache: UpdateCache,
    cache_updated: bool,
    pub(crate) sender: Sender<String>,
    pub(crate) ctx: egui::Context,
}

impl Updater {
    pub fn new(sender: Sender<String>, ctx: egui::Context) -> Self {
        Self {
            launcher_update_file: None,
            update_cache: UpdateCache { files: Vec::new() },
            cache_updated: false,
            sender,
            ctx,
        }
    }
}

impl Updater {
    pub fn start(&mut self) {
        self.send_message(CHECKING_FOR_UPDATES.to_owned());

        match self.get_update_file() {
            Ok(update_response) => self.process_update_response(update_response),
            Err(error) => {
                error!("{}: {}", UPDATE_FILE_NAME, error);
                self.finish();
            },
        };
    }

    pub fn load_update_cache(&mut self) {
        let update_cache_file = self.get_current_folder() + UPDATE_CACHE_FILE;
        let update_cache_path = Path::new(&update_cache_file);
        if update_cache_path.exists() {
            if let Ok(file) = File::open(update_cache_path) {
                let reader = io::BufReader::new(file);
                if let Ok(update_cache) = serde_json::from_reader::<_, UpdateCache>(reader) {
                    self.update_cache = update_cache;
                }
            }
        } 
    }

    pub fn save_update_cache_if_needed(&self) {
        if self.cache_updated { 
            let update_cache_file = self.get_current_folder() + UPDATE_CACHE_FILE;
            let update_cache_path = Path::new(&update_cache_file);
            if let Ok(file) = File::create(update_cache_path) {
                let _ = serde_json::to_writer_pretty(file, &self.update_cache);
            }
        }
    }

    fn update_file_cache(&mut self, file_path: &str, md5: String) -> std::io::Result<()> {
        let mut existing_entry_found = false;
        let full_path = self.get_current_folder() + file_path;
        let last_modified = fs::metadata(full_path).unwrap().modified()?;

        for file in self.update_cache.files.iter_mut() {
            if file.file_path == file_path {
                existing_entry_found = true;
                if file.md5 != md5 || file.last_modified != last_modified {
                    file.md5 = md5.to_owned();
                    file.last_modified = last_modified;
                    self.cache_updated = true;
                }
            }
        }

        if !existing_entry_found {
            let update_cache_file = UpdateCacheFile {
                file_path: file_path.to_owned(),
                last_modified,
                md5
            };
            self.update_cache.files.push(update_cache_file);
            self.cache_updated = true;
        }

        Ok(())
    }

    pub fn finish(&self) {
        self.save_update_cache_if_needed();
        match self.start_game() {
            Ok(_) => {
                if std::env::consts::OS == "macos" {
                    self.send_message(LAUNCHER_READY.to_owned());
                }
            },
            Err(e) => {
                error!("Failed to start game: {}", e);
                self.send_message(e.to_string());
            }
        }
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
                    let _ = self.update_file_cache(&update_file.local_path, local_hash_unwrapped);
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
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(last_modified) = metadata.modified() {
                    for file in self.update_cache.files.iter() {
                        if file.file_path == local_path && file.last_modified == last_modified {
                            return Some(file.md5.to_owned())
                        }
                    }
                }
            }

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
        self.finish();
    }

    fn update_launcher(&mut self) {
        if let Some(update_file) = self.launcher_update_file.clone() {
            self.download_file(&update_file.local_path);
            if let Some(new_hash) = self.get_md5(&update_file.local_path) {
                if new_hash == update_file.hash.to_lowercase() {
                    let new_launcher_path = self.get_current_folder() + &update_file.local_path;
                    let _ = self_replace::self_replace(new_launcher_path);
                    let _ = std::fs::remove_file(&update_file.local_path);
                    let _ = self.update_file_cache(&update_file.local_path, new_hash);
                }
            }
        }
        self.finish();
    }

    fn start_game(&self) -> std::io::Result<()> {
        info!("Starting the game");
        self.send_message(STARTING_GAME.to_owned());
    
        if std::env::consts::OS == "macos" {
            info!("Starting the game");
            let current_folder = self.get_current_folder();
            
            #[cfg(unix)]
            {
                let file_path = current_folder.to_owned() + "/" + CLIENT_BINARY;
                let mut perms = fs::metadata(&file_path)?.permissions();
                perms.set_mode(0o755); // User read/write/execute, Group and Others read/execute
                fs::set_permissions(&file_path, perms)?;
            }
    
            Command::new(current_folder.to_owned() + "/" + CLIENT_BINARY)
                .current_dir(current_folder)
                .arg("-uopath")
                .arg(self.get_current_folder())
                .arg("-clientversion")
                .arg(CLIENT_VERSION.to_owned())
                .spawn()?;
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