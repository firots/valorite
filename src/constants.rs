pub const APP_NAME: &str = "Valor Launcher";

#[cfg(target_os = "windows")]
pub const CUO_FILES_URL: &str  = "http://valor.gen.tr/cuo_files_win";
#[cfg(target_os = "macos")]
pub const CUO_FILES_URL: &str  = "https://valor.gen.tr/cuo_files_mac";

pub const UPDATE_FILE_NAME: &str  = "update.json";
pub const COMPRESSION_EXTENSION: &str = ".zip";

#[cfg(target_os = "windows")]
pub const LAUNCHER_EXECUTABLE_PATH: &str = "/valorite.exe";
#[cfg(target_os = "macos")]
pub const LAUNCHER_EXECUTABLE_PATH: &str = "/valorite";

pub const CLIENT_FOLDER_PATH: &str = "valor_cuo/";
pub const CLIENT_VERSION: &str = "6.0.6.2";

#[cfg(target_os = "windows")]
pub const CLIENT_BINARY: &str = "cuo.exe";
#[cfg(target_os = "macos")]
pub const CLIENT_BINARY: &str = "cuo";

pub const CHECKING_FOR_UPDATES: &str = "Guncellemeler denetleniyor...";
pub const DOWNLOADING_FILE: &str = "Indiriliyor: ";
pub const UPDATING_FILE: &str = "Guncelleniyor: ";
pub const STARTING_GAME: &str = "Oyun baslatiliyor...";
pub const HIDE_WINDOW_MESSAGE: &str = "HIDE_WINDOW";