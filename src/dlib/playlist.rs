use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader},
};

use serde::{Deserialize, Serialize};

use crate::dlib::playlist_iter::PlaylistIter;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaylistInfo {
    pub name: String,
    pub songs: Vec<u32>,
    pub id: Option<u32>,
    #[serde(skip)]
    pub file_entry_up_to_date: bool,
}

impl PlaylistInfo {
    pub fn new() -> Self {
        PlaylistInfo {
            name: "".into(),
            songs: Vec::new(),
            id: None,
            file_entry_up_to_date: false,
        }
    }

    pub fn add_song(&mut self, id: u32) -> std::io::Result<()> {
        if self.songs.contains(&id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Song already in playlist",
            ));
        }

        self.songs.push(id);
        self.file_entry_up_to_date = false;

        Ok(())
    }

    pub fn insert_song(&mut self, idx: usize, id: u32) -> std::io::Result<()> {
        if idx > self.songs.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
        self.songs.insert(idx, id);
        Ok(())
    }

    pub fn remove_song_by_index(&mut self, idx: usize) -> std::io::Result<()> {
        if idx < self.songs.len() {
            self.songs.remove(idx);
            self.file_entry_up_to_date = false;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ))
        }
    }

    pub fn force_add_song(&mut self, id: u32) {
        self.songs.push(id);
        self.file_entry_up_to_date = false;
    }

    pub fn into_dynamic_iter(self) -> PlaylistIter {
        PlaylistIter::new(&self.songs)
    }

    pub fn dynamic_iter(&self) -> PlaylistIter {
        PlaylistIter::new(&self.clone().songs)
    }
}

impl Default for PlaylistInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_playlists_from_file(base_directory: &str) -> io::Result<Vec<PlaylistInfo>> {
    let playlist_file = fs::OpenOptions::new()
        .read(true)
        .open(format!("{base_directory}/songs.json"))?;
    let mut reader = BufReader::new(&playlist_file);
    let mut buf = String::new();
    let mut playlists = Vec::new();
    loop {
        buf.clear();
        let byte_count = reader.read_line(&mut buf)?;
        if byte_count == 0 {
            break;
        }
        if let Ok(pl) = serde_json::from_str::<PlaylistInfo>(&buf) {
            playlists.push(pl);
        }
    }
    Ok(playlists)
}

pub fn indices_from_playlist_list(v: &[PlaylistInfo]) -> HashMap<u32, usize> {
    let mut map = HashMap::new();
    v.iter().enumerate().for_each(|(idx, s)| {
        if let Some(id) = s.id {
            map.insert(id, idx);
        };
    });
    map
}
