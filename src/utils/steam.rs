#![allow(unused)]

use std::{borrow::BorrowMut, collections::HashMap, default, fmt, path::{self, PathBuf}};
use registry::{Error, Hive, Security};
use regex::Regex;

use super::manifest::{self, ManifestParseError};

#[derive(Debug, Clone)]
pub struct PathError {
    pub path: PathBuf,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Path does not exist: {:?}", self.path)
    }
}

impl std::error::Error for PathError {}

static STEAM_ROOT: &str = r"Software\Valve\Steam";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SteamID {
    pub id3: i64,
    pub id64: i64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SteamAccount {
    pub name: String,
    pub id: Option<SteamID>,
}

impl SteamAccount {
    pub fn new(name: String, id: Option<SteamID>) -> Self {
        Self { name, id }
    }
}

impl From<i64> for SteamID { // From SteamID64
    fn from(id64: i64) -> Self {
        Self {
            id3: (id64 & 0xFFFFFFFF) as i64,
            id64,
        }
    }
}

#[derive(Debug, Default)]
pub struct SteamData {
    pub path: PathBuf,
    pub current_user: String,
    pub user_cache: Vec<SteamAccount>,
    pub remember_pass: bool,
    pub directories: HashMap<String, PathBuf>,
    pub games: serde_json::Value, // GameID: Manifest
}

impl SteamData {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            path: get_path()?,
            current_user: get_current_user()?,
            remember_pass: get_remember_pass_checked()?,
            games: serde_json::Value::Object(serde_json::Map::new()), // GameID: Manifest
            ..Default::default()
        })
    }

    pub fn detect_accounts(&mut self) -> Result<&Vec<SteamAccount>, Box<dyn std::error::Error>> {
        let mut detected_accounts = Vec::new();

        let config_path = self.path.join("config");
        let loginusers_path = config_path.join("loginusers.vdf");

        if !loginusers_path.exists() {
            eprintln!("Path does not exist: {:?}", loginusers_path);
            return Err(Box::new(PathError { path: loginusers_path }));
        }

        // Parse loginusers.vdf
        let loginusers_data = match manifest::parse_manifest(loginusers_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to parse manifest: {:?}", e);
                return Err(Box::new(ManifestParseError));
            }
        };

        for (key, value) in loginusers_data.as_object().unwrap() {
            let name = value.get("AccountName").unwrap().as_str().unwrap();
            let id64: i64 = key.parse().unwrap();
            detected_accounts.push(SteamAccount {
                name: name.to_string(),
                id: Some(SteamID::from(id64)),
            });
        }

        self.user_cache = detected_accounts;
        Ok(&self.user_cache)
    }

    pub fn detect_installs(&mut self, path: PathBuf) -> Result<Vec<(i32, String)>, Box<dyn std::error::Error>> {
        let mut detected_installs = Vec::new();

        let steamapps_path = path.join("steamapps");
        let libfolder_path = steamapps_path.join("libraryfolders.vdf");

        if !libfolder_path.exists() {
            eprintln!("Path does not exist: {:?}", libfolder_path);
            return Err(Box::new(PathError { path: libfolder_path }));
        }

        // Parse libraryfolders.vdf
        let libfolder_data = match manifest::parse_manifest(libfolder_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to parse manifest: {:?}", e);
                return Err(Box::new(ManifestParseError));
            }
        };

        // Add directories from libraryfolders.vdf
        let re = Regex::new(r"^[0-9]+$").unwrap();
        for (key, value) in libfolder_data.as_object().unwrap() {
            println!("Key: {}", key);
            if re.is_match(key) {
                let path = value.get("path").unwrap().as_str().unwrap();
                let drive = path.chars().nth(0).unwrap();
                self.directories.insert(key.to_string(), PathBuf::from(path));

                // Load paths game manifests
                let steamapps_path = format!("{}\\steamapps", path);
                for entry in std::fs::read_dir(steamapps_path)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if let Some(ext) = entry_path.extension() {
                        if ext != "acf" {
                            continue;
                        }
                        let game_manifest = match manifest::parse_manifest(entry_path) {
                            Ok(data) => data,
                            Err(e) => {
                                eprintln!("Failed to parse manifest: {:?}", e);
                                return Err(Box::new(ManifestParseError));
                            }
                        };
                        let game_id = game_manifest.get("appid").unwrap().as_str().unwrap();
                        let game_name = game_manifest.get("name").unwrap().as_str().unwrap();
                        detected_installs.push((game_id.parse().unwrap(), game_name.to_string()));
                        self.games.as_object_mut().unwrap().insert(game_id.parse().unwrap(), game_manifest);
                    }
                }
            }
        }
        Ok(detected_installs)
    }

    pub fn game_thumbnail(&self, appid: &i32) -> Result<(Option<PathBuf>, Option<PathBuf>), Box<dyn std::error::Error>> {
        let mut portrait: Option<PathBuf> = None;
        let mut landscape: Option<PathBuf> = None;

        let librarycache = self.path.join("appcache").join("librarycache");
        if !librarycache.exists() {
            eprintln!("Path does not exist: {:?}", librarycache);
            return Err(Box::new(PathError { path: librarycache }));
        }

        for entry in std::fs::read_dir(librarycache)? {
            let entry = entry?;
            let entry_path = entry.path();

            // Check if image is jpg or png
            let ext = entry_path.extension().unwrap();
            if ext != "jpg" && ext != "png" {
                continue;
            }

            let entry_name = entry_path.file_name().unwrap().to_str().unwrap().split('.').next().unwrap();
            if entry_name == format!("{}_header", appid) {
                landscape = Some(entry_path);
            } else if entry_name == format!("{}_library_600x900", appid) {
                portrait = Some(entry_path);
            }
        }
        
        Ok((portrait, landscape))
    }
}

pub fn get_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::Read)?;
    let path = regkey.value("SteamPath")?;
    Ok(
        PathBuf::from(
            path.to_string()
        )
    )
}

pub fn get_current_user() -> Result<String, Box<dyn std::error::Error>> {
    let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::Read)?;
    let user = regkey.value("AutoLoginUser")?;
    Ok(user.to_string())
}

pub fn get_remember_pass_checked() -> Result<bool, Box<dyn std::error::Error>> {
    let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::Read)?;
    let remember_pass = regkey.value("RememberPassword")?;
    Ok(remember_pass.to_string() == "1")
}