#![allow(unused)]
mod data;
use data::*;
mod error;
use error::LoginError;
mod manifest;
use manifest::prelude::*;

use std::{collections::{HashMap, HashSet}, path::PathBuf};
use registry::{Data, Hive, Security};
use regex::Regex;
use sysinfo::System;

#[allow(unused)]
pub mod prelude {
    pub use super::error::LoginError;
    pub use super::data::{SteamID, SteamAccount, AppID, Thumbnail};
    pub use super::SteamModel;
}

static STEAM_ROOT: &str = r"Software\Valve\Steam";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! steam_command {
    ($exec:expr, $args:expr) => {
        std::process::Command::new($exec).args($args)
    };
}

/// Steam Model
/// 
/// The Steam Model is a struct that contains all the data and functions required to interact with the Steam client.
/// 
/// # Variables
/// 
/// - `install_path` - The path to the Steam install directory
/// - `current_user` - The current logged in user
/// - `user_cache` - A vector of all detected users
/// - `directories` - A hashmap of all detected directories and their associated games
/// - `games` - A json object of all detected games and their manifests
#[allow(unused)]
#[derive(Debug, Default, Clone)]
pub struct SteamModel {
    pub install_path: PathBuf,
    pub current_user: Option<SteamAccount>,
    pub user_cache: Vec<SteamAccount>,
    pub directories: HashMap<PathBuf, HashSet<i32>>,
    pub games: HashMap<AppID, serde_json::Value>, // GameID: Manifest
}

impl SteamModel {
    pub fn new() -> Result<Self> {
        // Get Steam install path
        let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::Read)?;
        let path = regkey.value("SteamPath")?;

        Ok(Self {
            install_path: PathBuf::from(path.to_string()),
            ..Default::default()
        })
    }

    /// Get the current logged in user
    /// 
    /// Returns the current logged in user as a [`SteamAccount`]
    /// 
    /// # Warning
    /// 
    /// This function requires [`SteamModel::detect_accounts`] to be called first
    pub fn get_current_user(&self) -> Result<SteamAccount> {
        let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::Read)?;
        let user_name = regkey.value("AutoLoginUser")?.to_string();

        // Convert username to SteamAccount
        let user = match self.user_cache.iter().find(|x| x.name == user_name) {
            Some(user) => user,
            None => {
                eprintln!("Failed to find user: {}", user_name);
                return Err(Box::new(LoginError::Other("Failed to find user".to_string())));
            },
        };

        Ok(user.clone())
    }

    /// Detect all accounts on the system
    /// 
    /// Returns a vector of [`SteamAccount`]s
    pub fn detect_accounts(&mut self) -> Result<&Vec<SteamAccount>> {
        let mut detected_accounts = Vec::new();

        let config_path = self.install_path.join("config");
        let loginusers_path = config_path.join("loginusers.vdf");

        if !loginusers_path.exists() {
            eprintln!("Path does not exist: {:?}", loginusers_path);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Loginusers path does not exist")));
        }

        // Parse loginusers.vdf
        let loginusers_data = match manifest::parse_manifest(loginusers_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to parse manifest: {:?}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse loginusers manifest")));
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
            let mut user_games: HashSet<i32> = HashSet::new();
            let user_path = self.install_path.join("userdata").join(format!("{}", steamid.id3));
            let localconfig_path = user_path.join("config").join("localconfig.vdf");

            if !localconfig_path.exists() {
                eprintln!("Path does not exist: {:?}", localconfig_path);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Localconfig path does not exist")));
            }

            let localconfig_data = match manifest::parse_manifest(localconfig_path) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to parse manifest: {:?}", e);
                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse localconfig manifest")));
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
                match appid.parse::<i32>() {
                    Ok(appid) => {
                        user_games.insert(appid);
                    },
                    Err(_) => {
                        eprintln!("Failed to parse appid: {}", appid);
                    },
                }
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

    /// Detect all installed games from the detected install path
    /// 
    /// Returns a hashset of [`AppID`]s
    pub fn detect_installs(&mut self) -> Result<HashSet<AppID>> {
        let mut detected_installs = HashSet::<AppID>::new();

        let steamapps_path = self.install_path.join("steamapps");
        let libfolder_path = steamapps_path.join("libraryfolders.vdf");

        if !libfolder_path.exists() {
            eprintln!("Path does not exist: {:?}", libfolder_path);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Libraryfolders path does not exist")));
        }

        // Parse libraryfolders.vdf
        let libfolder_data = match manifest::parse_manifest(libfolder_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to parse manifest: {:?}", e);
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse libraryfolders manifest")));
            }
        };

        // Add directories from libraryfolders.vdf
        let re = Regex::new(r"^[0-9]+$").unwrap();
        for (key, value) in libfolder_data.as_object().unwrap() {
            if re.is_match(key) {
                let path = value.get("path").unwrap().as_str().unwrap();
                let apps: HashSet<i32> = match value.get("apps").unwrap().as_object() {
                    Some(data) => {
                        if !data.keys().next().unwrap().is_empty() {
                            data.keys().map(|x| x.parse().unwrap()).collect()
                        } else {
                            Default::default()
                        }
                    },
                    None => Default::default(),
                };
                if apps.is_empty() {
                    eprintln!("No games detected for path {}", path);
                    continue;
                }
                
                self.directories.insert(PathBuf::from(path), apps);

                // Load paths game manifests
                let steamapps_path = PathBuf::from(format!("{}\\steamapps", path));
                for entry in std::fs::read_dir(steamapps_path.clone())? {
                    let entry_path = entry?.path();
                    if let Some(ext) = entry_path.extension() {
                        if ext != "acf" {
                            continue;
                        }
                        let game_manifest = match manifest::parse_manifest(entry_path) {
                            Ok(data) => data,
                            Err(e) => {
                                eprintln!("Failed to parse manifest: {:?}", e);
                                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse game manifest")));
                            }
                        };
                        let game_id = game_manifest.get("appid").unwrap().as_str().unwrap().parse::<i32>().unwrap();
                        let game_name = game_manifest.get("name").unwrap().as_str().unwrap();
                        let install_dir = game_manifest.get("installdir").unwrap().as_str().unwrap();
                        let last_played = match game_manifest.get("LastPlayed") {
                            Some(data) => data.as_str().unwrap().parse::<u64>().ok(),
                            None => None,
                        };

                        let app: AppID = AppID {
                            id: game_id,
                            name: game_name.to_string(),
                            location: steamapps_path.join("common").join(install_dir),
                            last_played,
                        };

                        detected_installs.insert(app.clone());
                        self.games.insert(app, game_manifest);
                    }
                }
            }
        }
        Ok(detected_installs)
    }

    /// Get all installed apps
    /// 
    /// Returns a vector of [`AppID`]s
    pub fn get_installed_apps(&self) -> Vec<AppID> {
        self.games.keys().cloned().collect()
    }

    pub fn get_installed_apps_with_manifests(&self) -> &HashMap<AppID, serde_json::Value> {
        &self.games
    }

    pub fn get_app_manifest(&self, appid: &AppID) -> Option<&serde_json::Value> {
        self.games.get(appid)
    }

    /// Get the thumbnail for a game
    /// 
    /// Returns a [`Thumbnail`] struct containing the portrait and landscape paths
    pub fn game_thumbnail(&self, appid: &i32) -> Result<Thumbnail> {
        // println!("Getting thumbnail for appid: {}", appid);
        let librarycache_path = self.install_path.join("appcache").join("librarycache");
        let mut thumbnail: Thumbnail = Default::default();

        let portrait = format!("{}_library_600x900", appid);
        let landscape = format!("{}_header", appid);

        for file_type in ["jpg", "png"].iter() {
            let portrait_path = librarycache_path.join(format!("{}.{}", portrait, file_type));
            let landscape_path = librarycache_path.join(format!("{}.{}", landscape, file_type));

            if portrait_path.exists() {
                thumbnail.portrait = Some(portrait_path);
            }
            if landscape_path.exists() {
                thumbnail.landscape = Some(landscape_path);
            }
            if thumbnail.portrait.is_some() && thumbnail.landscape.is_some() {
                break;
            }
        }

        Ok(thumbnail)
    }

    /// Set the login account in registry
    /// 
    /// Sets the AutoLoginUser and RememberPassword values in the registry
    pub fn set_login_account(&self, account: &SteamAccount) -> Result<()> {
        let regkey = Hive::CurrentUser.open(STEAM_ROOT, Security::AllAccess)?;

        if regkey.value("AutoLoginUser")?.to_string() == account.name {
            return Err(Box::new(LoginError::AlreadyLoggedIn));
        }

        // Set AutoLoginUser and RememberPassword
        let user_data: Data = Data::String(utfx::WideCString::from_str(&account.name).unwrap().into());
        regkey.set_value("AutoLoginUser", &user_data)?;
        regkey.set_value("RememberPassword", &Data::U32(1))?;

        Ok(())
    }

    /// Run steam with optional arguments
    pub fn restart(&self, args: Option<Vec<String>>, exit_after: bool) -> Result<()> {
        let steam_exe = self.install_path.join("steam.exe");

        // Spawn thread to start steam
        std::thread::spawn(move || {
            let mut system = System::new_all();

            // Close steam if running
            if system.processes_by_exact_name("steam.exe".as_ref()).count() > 0 {
                eprintln!("Steam is running, closing...");
                steam_command!(&steam_exe, ["-exitsteam"]).output().expect("Failed to close Steam");

                // Wait for steam to close
                while system.processes_by_exact_name("steam.exe".as_ref()).count() > 0 {
                    eprintln!("Waiting for Steam to close...");
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    system = System::new_all(); // system.refresh_all() just doesn't work ig ¯\_(ツ)_/¯
                }
                eprintln!("Steam closed");
            }

            // Start steam
            eprintln!("Starting Steam with args: {:?}", args.clone().unwrap_or(vec![]));
            steam_command!(&steam_exe, args.unwrap_or(vec![])).spawn().expect("Failed to start Steam");

            // Exit the application
            if exit_after { std::process::exit(0); }
        });

        Ok(())
    }
    
    /// Initiate a login with the provided account
    /// 
    /// this function will set the login account and start steam
    pub fn login(&self, account: &SteamAccount, exit_after: bool) -> Result<()> {
        match self.set_login_account(account) {
            Ok(_) => (),
            Err(e) => {
                return Err(e);
            },
        }

        eprintln!("Successfully set login account: {}", account.name);

        self.restart(None, exit_after)?;

        Ok(())
    }

    /// Launch a game with the provided account and appid
    /// 
    /// this function will login to the account and start the game
    pub fn launch_game(&self, account: &SteamAccount, appid: &i32, close_after: bool) -> Result<()> {
        let args = vec![
            "-applaunch".to_string(),
            appid.to_string(),
        ];

        match self.set_login_account(account) {
            Ok(_) => (),
            Err(e) => {
                if e.downcast_ref::<LoginError>().is_some_and(|e| e == &LoginError::AlreadyLoggedIn) {
                    std::process::Command::new(self.install_path.join("steam.exe"))
                        .args(args)
                        .spawn().unwrap();
                    if close_after { std::process::exit(0); }
                    return Ok(());
                }
                return Err(e);
            },
        }

        self.restart(Some(args), close_after)?;

        Ok(())
    }
}