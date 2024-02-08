use std::{error::Error, fs::{self, File}, io::{self, Read}, path::Path, process::{self, Command}, sync::mpsc::Sender};
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
    pub(crate) message_sender: Sender<String>
}

impl Updater {
    pub fn start_update_check(&mut self) {
        self.message_sender.send(CHECKING_FOR_UPDATES.to_owned()).unwrap();
        match self.get_update_file() {
            Ok(update_response) => self.process_update_response(update_response),
            Err(error) => {
                error!("{}: {}", UPDATE_FILE_NAME, error);
                self.start_game()
            },
        };
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
            if let Some(local_launcher_hash) = self.get_md5(LAUNCHER_EXECUTABLE_PATH) {
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
            self.download_file(&update_file.local_path);
            if let Some(new_hash) = self.get_md5(&update_file.local_path) {
                if new_hash == update_file.hash.to_lowercase() {
                    let new_launcher_path = ".".to_owned() + &update_file.local_path;
                    let _ = self_replace::self_replace(new_launcher_path);
                    let _ = std::fs::remove_file(&update_file.local_path);
                }
            }
        }
        self.start_game()
    }

    fn start_game(&mut self) {
        info!("Starting the game");
        self.message_sender.send(STARTING_GAME.to_owned()).unwrap();
        Command::new(CLIENT_FOLDER_PATH.to_owned() + CLIENT_BINARY)
            .current_dir(CLIENT_FOLDER_PATH)
            .spawn()
            .unwrap();
        process::exit(0x0100);
    }

    fn download_file(&self, file_path: &str) {
        self.message_sender.send(DOWNLOADING_FILE.to_owned() + &file_path).unwrap();
        let file_url = &(CUO_FILES_URL.to_owned() + &file_path + COMPRESSION_EXTENSION);
        let path = Path::new(&file_path);
        let mut download_path = path.parent().and_then(|parent| parent.to_str());
        if download_path == None {
            download_path = Some("/");
        } else {
            fs::create_dir_all(".".to_owned() + download_path.unwrap())
                .expect("Failed to create");
        }

        info!("file_url: {}", file_url);
        info!("download_path: {}", download_path.unwrap());

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
                Err(e) => error!("{}", e.to_string()),
                Ok(s) => {
                    self.message_sender.send(UPDATING_FILE.to_owned() + &file_path).unwrap();
                    let extract_path_string = ".".to_owned() + download_path.unwrap();
                    let extract_path = std::path::Path::new(&extract_path_string);
                    let _ = self.unzip_file(zip_path, extract_path);
                    info!("{}", s)
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