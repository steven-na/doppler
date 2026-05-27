use core::fmt;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SongInfo {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    pub filename: Option<String>,
    pub id: Option<u32>,
    #[serde(skip)]
    pub file_entry_up_to_date: bool,
}

impl SongInfo {
    pub fn new() -> Self {
        SongInfo {
            name: "".into(),
            artist: "".into(),
            album: "".into(),
            duration: 0,
            filename: None,
            id: None,
            file_entry_up_to_date: false,
        }
    }
}

impl fmt::Display for SongInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\x1b[1m{}\x1b[0m on \x1b[3m{}\x1b[0m by {} ({})",
            self.name,
            self.album,
            self.artist,
            crate::util::time_util::seconds_to_base60_string(self.duration)
        )
    }
}

impl Default for SongInfo {
    fn default() -> Self {
        Self::new()
    }
}
