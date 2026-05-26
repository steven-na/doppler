use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};

use crate::dlib::{playlist::*, song::*};

const SONGS_FILE_PATH: &str = "./data/songs.json";
const PLAYLISTS_FILE_PATH: &str = "./data/playlists.json";

#[derive(Debug)]
pub struct DopplerInfo {
    pub songs: Vec<SongInfo>,
    pub playlists: Vec<Playlist>,
    pub indices: HashMap<u32, usize>,
    pub removed: HashSet<u32>,
    pub max_id: Option<u32>,
}

impl DopplerInfo {
    pub fn new() -> std::io::Result<Self> {
        let mut songs = Self::read_songs_from_file()?;
        songs.iter_mut().for_each(|s| {
            s.file_entry_up_to_date = true;
        });
        let mut playlists = Self::read_playlists_from_file()?;
        playlists.iter_mut().for_each(|p| {
            p.file_entry_up_to_date = true;
        });
        let indices = Self::indices_from_song_list(&songs);
        let max_id = songs.iter().map(|s| s.id.unwrap_or(0)).max();

        Ok(DopplerInfo {
            songs,
            playlists,
            indices,
            removed: HashSet::new(),
            max_id,
        })
    }

    pub fn play_song(&self, id: u32) -> std::io::Result<()> {
        let song = self
            .indices
            .get(&id)
            .and_then(|&idx| self.songs.get(idx))
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No song with this id exists",
            ))?;

        let duration_str = crate::util::time_util::seconds_to_base60_string(song.duration);
        println!(
            "Now playing (0:00/{duration_str}): {0} ({1}) by {2}",
            song.name, song.album, song.artist
        );
        Ok(())
    }
    pub fn sort(&mut self) {
        self.songs.sort_unstable_by_key(|s| s.id);
        self.indices = Self::indices_from_song_list(&self.songs);
    }

    fn indices_from_song_list(v: &[SongInfo]) -> HashMap<u32, usize> {
        let mut map = HashMap::new();
        v.iter().enumerate().for_each(|(idx, s)| {
            if let Some(id) = s.id {
                map.insert(id, idx);
            };
        });
        map
    }

    fn read_songs_from_file() -> std::io::Result<Vec<SongInfo>> {
        let song_file = fs::OpenOptions::new().read(true).open(SONGS_FILE_PATH)?;

        let mut reader = BufReader::new(&song_file);
        let mut buf = String::new();
        let mut songs = Vec::new();
        loop {
            buf.clear();
            let byte_count = reader.read_line(&mut buf)?;
            if byte_count == 0 {
                break; // EOF
            }
            if let Ok(song) = serde_json::from_str::<SongInfo>(&buf) {
                songs.push(song);
            }
        }
        Ok(songs)
    }

    fn read_playlists_from_file() -> std::io::Result<Vec<Playlist>> {
        let playlist_file = fs::OpenOptions::new().read(true).open(SONGS_FILE_PATH)?;

        let mut reader = BufReader::new(&playlist_file);
        let mut buf = String::new();
        let mut playlists = Vec::new();
        loop {
            buf.clear();
            let byte_count = reader.read_line(&mut buf)?;
            if byte_count == 0 {
                break; // EOF
            }
            if let Ok(song) = serde_json::from_str::<Playlist>(&buf) {
                playlists.push(song);
            }
        }
        Ok(playlists)
    }

    pub fn add_song(&mut self, song: SongInfo) -> std::io::Result<()> {
        match song.id {
            Some(i) => {
                if i < self.max_id.unwrap_or(0)
                    || self.songs.iter().any(|s| s.id.unwrap_or(u32::MAX) == i)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "song with this id already exists",
                    ));
                } else {
                    let mut song = song;
                    song.file_entry_up_to_date = false;
                    self.songs.push(song);
                    self.max_id = if i > self.max_id.unwrap_or(0) {
                        Some(i)
                    } else {
                        self.max_id
                    };
                    return Ok(());
                }
            }
            None => {
                let mut song = song;
                song.id = Some(self.max_id.unwrap_or(0) + 1);
                song.file_entry_up_to_date = false;
                self.max_id = song.id;
                self.songs.push(song);
            }
        }
        self.indices = Self::indices_from_song_list(&self.songs);
        Ok(())
    }

    pub fn remove_song(&mut self, id: u32) -> std::io::Result<()> {
        if !self.songs.iter().any(|s| match s.id {
            Some(o_id) => id == o_id,
            None => false,
        }) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Song with this id doesnt exist",
            ));
        } else if self.removed.contains(&id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Song already removed",
            ));
        }

        self.removed.insert(id);
        Ok(())
    }

    pub fn update_song(&mut self, song: SongInfo) -> std::io::Result<()> {
        let id = match song.id {
            Some(i) => i,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "input song has no ID",
                ));
            }
        };
        match self.songs.iter_mut().find(|s| s.id.unwrap_or(0) == id) {
            Some(i) => {
                *i = song;
                i.file_entry_up_to_date = false;
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Song with this id doesnt exist",
                ));
            }
        }

        Ok(())
    }

    pub fn update_songs_file(&mut self) -> std::io::Result<()> {
        let song_file = fs::OpenOptions::new().read(true).open(SONGS_FILE_PATH)?;

        let mut reader = BufReader::new(&song_file);
        let mut buf = String::new();
        loop {
            buf.clear();
            let byte_count = reader.read_line(&mut buf)?;
            if byte_count == 0 {
                break; // EOF
            }
            if let Ok(existing) = serde_json::from_str::<SongInfo>(&buf)
                && self.songs.iter().any(|s| s.id != existing.id)
            {
                let _ = self.add_song(existing);
            }
        }

        let temp_file_path = format!("{}.temp", SONGS_FILE_PATH);
        let temp_song_file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&temp_file_path)?;
        let mut temp_file_writer = BufWriter::new(temp_song_file);

        self.sort();
        self.songs.iter_mut().for_each(|song| {
            let i = match song.id {
                Some(t) => t,
                None => {
                    println!("Attempted to write song with no id");
                    dbg!(song);
                    return;
                }
            };

            if self.removed.contains(&i) {
                return;
            }

            let json = serde_json::to_string(song);
            match json {
                Ok(j) => {
                    let _ = temp_file_writer.write_all(j.as_bytes());
                    let _ = temp_file_writer.write_all(b"\n");
                }
                Err(err) => {
                    println!("Error writing to file. ({})", err);
                }
            }
        });

        fs::rename(temp_file_path, SONGS_FILE_PATH)?;

        self.songs.retain(|s| match s.id {
            Some(i) => !self.removed.contains(&i),
            None => true,
        });

        self.removed.clear();

        self.songs.iter_mut().for_each(|s| {
            s.file_entry_up_to_date = true;
        });

        Ok(())
    }
}
