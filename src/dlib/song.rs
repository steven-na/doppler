use core::fmt;
use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader},
};

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
        write!(f, "{} on {} by {}", self.name, self.album, self.artist,)
    }
}

impl Default for SongInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_songs_from_file(base_directory: &str) -> io::Result<Vec<SongInfo>> {
    let song_file = fs::OpenOptions::new()
        .read(true)
        .open(format!("{base_directory}/songs.json"))?;
    let mut reader = BufReader::new(&song_file);
    let mut buf = String::new();
    let mut songs = Vec::new();
    loop {
        buf.clear();
        let byte_count = reader.read_line(&mut buf)?;
        if byte_count == 0 {
            break;
        }
        if let Ok(song) = serde_json::from_str::<SongInfo>(&buf) {
            songs.push(song);
        }
    }
    Ok(songs)
}

pub fn indices_from_song_list(v: &[SongInfo]) -> HashMap<u32, usize> {
    let mut map = HashMap::new();
    v.iter().enumerate().for_each(|(idx, s)| {
        if let Some(id) = s.id {
            map.insert(id, idx);
        };
    });
    map
}
