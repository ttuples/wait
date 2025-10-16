use core::fmt;
use std::{collections::HashSet, path::PathBuf};

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct SteamID {
    pub id3: i64,
    pub id64: i64,
}

impl From<i64> for SteamID {
    fn from(id64: i64) -> Self {
        Self {
            id3: (id64 & 0xFFFFFFFF),
            id64,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct SteamAccount {
    pub name: String,
    pub id: Option<SteamID>,
    pub games: HashSet<i32>,
}

impl fmt::Display for SteamAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl SteamAccount {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct AppID {
    pub id: i32,
    pub name: String,
    pub location: PathBuf,
    pub last_played: Option<u64>,
}

impl std::hash::Hash for AppID {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Thumbnail {
    pub portrait: Option<PathBuf>,
    pub landscape: Option<PathBuf>,
}

impl From<(Option<PathBuf>, Option<PathBuf>)> for Thumbnail {
    fn from((portrait, landscape): (Option<PathBuf>, Option<PathBuf>)) -> Self {
        Self {
            portrait,
            landscape,
        }
    }
}