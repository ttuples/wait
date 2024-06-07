use std::{collections::HashMap, path::PathBuf};
use registry::{Data, Hive, Security};
use regex::Regex;
use sysinfo::System;

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

#[derive(Debug, Clone)]
pub struct AlreadyLoggedInError;

impl std::fmt::Display for AlreadyLoggedInError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Already logged in")
    }
}

impl std::error::Error for AlreadyLoggedInError {}

static STEAM_ROOT: &str = r"Software\Valve\Steam";

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct SteamID {
    pub id3: i64,
    pub id64: i64,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct SteamAccount {
    pub name: String,
    pub id: Option<SteamID>,
    pub games: Vec<i32>,
}

impl From<i64> for SteamID { // From SteamID64
    fn from(id64: i64) -> Self {
        Self {
            id3: (id64 & 0xFFFFFFFF) as i64,
            id64,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SteamModel {
    pub path: PathBuf,
    pub current_user: String,
    pub user_cache: Vec<SteamAccount>,
    pub directories: HashMap<PathBuf, Vec<i64>>,
    pub games: serde_json::Value, // GameID: Manifest
}

impl SteamModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            path: get_path()?,
            current_user: get_current_user()?,
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
            // Get account name
            let name = match value.get("AccountName") {
                Some(data) => data.as_str().unwrap_or_else(|| ""),
                None => "",
            };
            if name.is_empty() {
                eprintln!("Failed to load account '{}', Could not read 'AccountName'", key);
                continue;
            }

            // Get account SteamID
            let id = match key.parse::<i64>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Failed to load account '{}', Could not read 'SteamID'", key);
                    continue;
                },
            };
            let steamid: SteamID = SteamID::from(id);

            // Get account games
            let mut user_games: Vec<i32> = Vec::new();
            let user_path = self.path.join("userdata").join(format!("{}", steamid.id3));
            let localconfig_path = user_path.join("config").join("localconfig.vdf");

            if !localconfig_path.exists() {
                eprintln!("Path does not exist: {:?}", localconfig_path);
                return Err(Box::new(PathError { path: localconfig_path }));
            }

            let localconfig_data = match manifest::parse_manifest(localconfig_path) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to parse manifest: {:?}", e);
                    return Err(Box::new(ManifestParseError));
                }
            };

            let software = match localconfig_data.get("Software") {
                Some(data) => data.as_object().unwrap(),
                None => {
                    eprintln!("Failed to load account {}", name);
                    continue;
                }
            };

            let mut app_data = if software.contains_key("Valve") {
                software.get("Valve").unwrap().clone()
            } else if software.contains_key("valve") {
                software.get("valve").unwrap().clone()
            } else {
                continue;
            };

            if app_data.as_object().unwrap().contains_key("Steam") {
                app_data = app_data.get("Steam").unwrap().clone();
            } else if app_data.as_object().unwrap().contains_key("steam") {
                app_data = app_data.get("steam").unwrap().clone();
            } else {
                continue;
            };

            for (appid, _) in app_data.get("apps").unwrap().as_object().unwrap() {
                user_games.push(appid.parse::<i32>().unwrap());
            }

            detected_accounts.push(SteamAccount {
                name: name.to_string(),
                id: Some(steamid),
                games: user_games,
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
            if re.is_match(key) {
                let path = value.get("path").unwrap().as_str().unwrap();
                let apps: Vec<i64> = match value.get("apps").unwrap().as_object() {
                    Some(data) => {
                        if data.keys().next().unwrap().is_empty() {
                            Vec::new()
                        } else {
                            data.keys().map(|x| x.parse().unwrap()).collect()
                        }
                    },
                    None => Vec::new(),
                };
                if apps.is_empty() {
                    eprintln!("No games detected for path {}", path);
                    continue;
                }
                
                self.directories.insert(PathBuf::from(path), apps);

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
        let librarycache_path = self.path.join("appcache").join("librarycache");

        let portrait_jpg = format!("{}_library_600x900.jpg", appid);
        let portrait_png = format!("{}_library_600x900.png", appid);
        let landscape_jpg = format!("{}_header.jpg", appid);
        let landscape_png = format!("{}_header.png", appid);

        let portrait = if librarycache_path.join(&portrait_jpg).exists() {
            Some(librarycache_path.join(&portrait_jpg))
        } else if librarycache_path.join(&portrait_png).exists() {
            Some(librarycache_path.join(&portrait_png))
        } else {
            None
        };

        let landscape = if librarycache_path.join(&landscape_jpg).exists() {
            Some(librarycache_path.join(&landscape_jpg))
        } else if librarycache_path.join(&landscape_png).exists() {
            Some(librarycache_path.join(&landscape_png))
        } else {
            None
        };
        
        Ok((portrait, landscape))
    }

    pub fn set_login_account(&self, account: &SteamAccount) -> Result<(), Box<dyn std::error::Error>> {
        let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::AllAccess)?;

        if regkey.value("AutoLoginUser")?.to_string() == account.name {
            return Err(Box::new(AlreadyLoggedInError));
        }

        // Set AutoLoginUser and RememberPassword
        let user_data: Data = Data::String(utfx::WideCString::from_str(&account.name).unwrap().into());
        regkey.set_value("AutoLoginUser", &user_data)?;
        regkey.set_value("RememberPassword", &Data::U32(1))?;

        Ok(())
    }

    pub fn run(&self, args: Option<Vec<String>>) -> Result<(), Box<dyn std::error::Error>> {
        let steam_exe = self.path.join("steam.exe");

        // Spawn thread to start steam
        std::thread::spawn(move || {
            let mut system = System::new_all();
            system.refresh_all();

            // Close steam if running
            if system.processes_by_exact_name("steam.exe").count() > 0 {
                println!("Steam is running, closing...");
                std::process::Command::new(steam_exe.clone()).arg("-exitsteam").output().unwrap();

                // Wait for steam to close
                while system.processes_by_exact_name("steam.exe").count() > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    system.refresh_all();
                }
                println!("Steam closed");
            }

            // Start steam
            println!("Starting Steam with args: {:?}", args.clone().unwrap_or(vec![]));
            std::process::Command::new(steam_exe)
                .args(args.unwrap_or(vec![]))
                .spawn().unwrap();
        });

        Ok(())
    }
    
    pub fn login(&self, account: &SteamAccount) -> Result<(), Box<dyn std::error::Error>> {
        match self.set_login_account(account) {
            Ok(_) => (),
            Err(e) => {
                if e.downcast_ref::<AlreadyLoggedInError>().is_some() {
                    return Ok(());
                }
            },
        }

        self.run(None)?;

        Ok(())
    }

    pub fn launch_game(&self, account: &SteamAccount, appid: &i32) -> Result<(), Box<dyn std::error::Error>> {
        match self.set_login_account(account) {
            Ok(_) => (),
            Err(e) => {
                if e.downcast_ref::<AlreadyLoggedInError>().is_none() {
                    return Err(e);
                }
            },
        }

        let args = vec![
            "-noreactlogin".to_string(),
            "-silent".to_string(),
            "-applaunch".to_string(),
            appid.to_string(),
        ];

        self.run(Some(args))?;

        Ok(())
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