use std::{
    fs,
    io::{self, BufRead, BufReader},
};

use serde::{Deserialize, Serialize};

use crate::dlib::doppler_info::LYRICS_FILE_PATH;

#[derive(Debug, Serialize, Deserialize)]
pub struct LyricInfo {
    pub song_id: Option<u32>,
    pub is_synced: bool,
    pub timings: Option<Vec<u32>>,
    pub lyrics: Vec<String>,
}

impl LyricInfo {
    pub fn new() -> Self {
        Self {
            song_id: None,
            is_synced: false,
            timings: None,
            lyrics: Vec::new(),
        }
    }

    pub fn new_synced() -> Self {
        Self {
            is_synced: true,
            timings: Some(Vec::new()),
            ..Default::default()
        }
    }

    pub fn get_lyric_at_time(&self, time: u32) -> Option<String> {
        if !self.is_synced {
            return None;
        }
        let time = time.saturating_sub(1);
        for (index, &timing) in self.timings.clone().unwrap().iter().enumerate() {
            if timing <= time {
                return self.lyrics.get(index).cloned();
            }
        }
        None
    }

    pub fn get_lyrics_around_time(
        &self,
        time: u32,
        match_pos: usize,
        line_count: usize,
    ) -> Option<Vec<String>> {
        if !self.is_synced {
            return None;
        }
        let time = time.saturating_sub(1);
        let pos = self
            .timings
            .clone()
            .unwrap()
            .iter()
            .position(|&timing| timing <= time);
        if let Some(pos) = pos {
            return Some(
                self.lyrics
                    .iter()
                    .skip(pos.saturating_sub(match_pos))
                    .take(line_count)
                    .cloned()
                    .collect(),
            );
        }
        None
    }
}

impl Default for LyricInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_lyrics_from_file() -> io::Result<Vec<LyricInfo>> {
    let lyrics_file = fs::OpenOptions::new().read(true).open(LYRICS_FILE_PATH)?;
    let mut reader = BufReader::new(&lyrics_file);
    let mut buf = String::new();
    let mut lyrics = Vec::new();
    loop {
        buf.clear();
        let byte_count = reader.read_line(&mut buf)?;
        if byte_count == 0 {
            break;
        }
        if let Ok(pl) = serde_json::from_str::<LyricInfo>(&buf) {
            lyrics.push(pl);
        }
    }
    Ok(lyrics)
}
