use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::sync::{Arc, Mutex};

use crate::dlib::{playlist::*, song::*};
use crate::util::search_utli::search;

const SONGS_FILE_PATH: &str = "./data/songs.json";
const PLAYLISTS_FILE_PATH: &str = "./data/playlists.json";

#[derive(Debug)]
pub struct DopplerInfo {
    pub songs: Vec<SongInfo>,
    pub playlists: Vec<PlaylistInfo>,

    pub song_indices: HashMap<u32, usize>,
    pub playlist_indices: HashMap<u32, usize>,

    pub removed_songs: HashSet<u32>,
    pub removed_playlists: HashSet<u32>,

    pub max_song_id: Option<u32>,
    pub max_playlist_id: Option<u32>,

    pub currently_playing: Arc<Mutex<Option<u32>>>,
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

        let song_indices = Self::indices_from_song_list(&songs);
        let max_song_id = songs.iter().map(|s| s.id.unwrap_or(0)).max();

        let playlist_indices = Self::indices_from_playlist_list(&playlists);
        let max_playlist_id = playlists.iter().map(|s| s.id.unwrap_or(0)).max();

        Ok(DopplerInfo {
            songs,
            playlists,
            song_indices,
            playlist_indices,
            removed_songs: HashSet::new(),
            removed_playlists: HashSet::new(),
            max_song_id,
            max_playlist_id,
            currently_playing: Arc::new(Mutex::new(None)),
        })
    }

    pub fn play_song(&mut self, id: u32, player: &rodio::Player) -> std::io::Result<()> {
        let song = self.get_song_by_id(id).ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No song with this id exists",
        ))?;

        let path = match &song.filename {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Song has no file associated",
                ));
            }
        };

        let audio_file = BufReader::new(fs::File::open(path)?);
        let source = rodio::Decoder::try_from(audio_file).expect("Failed to decode audio file");

        player.clear();
        player.append(source);

        let currently_playing = Arc::clone(&self.currently_playing);
        player.append(rodio::source::EmptyCallback::new(Box::new(move || {
            let mut lock = currently_playing.lock().unwrap();
            *lock = None;
        })));

        player.play();
        let mut lock = self.currently_playing.lock().unwrap();
        *lock = Some(id);
        Ok(())
    }

    pub fn enqueue_song(&mut self, id: u32, player: &rodio::Player) -> std::io::Result<()> {
        let song = self
            .get_song_by_id(id)
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No song with this id exists",
            ))?
            .clone();

        let path = match &song.filename {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Song has no file associated",
                ));
            }
        };

        let audio_file = BufReader::new(fs::File::open(path)?);
        let source = rodio::Decoder::try_from(audio_file).expect("Failed to decode audio file");

        // If this is the only song, don't give it the precallback. For some reason (wrong sample
        // rate that doesn't get updated?) it makes the audio very slow. Just do it synchronously
        if player.len() != 0 {
            let currently_playing = Arc::clone(&self.currently_playing);
            let precallback = rodio::source::EmptyCallback::new(Box::new(move || {
                let mut lock = currently_playing.lock().unwrap();
                *lock = Some(id);

                let duration_str = crate::util::time_util::seconds_to_base60_string(song.duration);
                println!("\nNow playing (0:00/{duration_str}): {}", song);
            }));
            player.append(precallback);
        } else {
            let mut lock = self.currently_playing.lock().unwrap();
            *lock = Some(id);
            drop(lock);

            let duration_str = crate::util::time_util::seconds_to_base60_string(song.duration);
            println!("\nNow playing (0:00/{duration_str}): {}", song);
        }

        player.append(source);

        let currently_playing = Arc::clone(&self.currently_playing);
        player.append(rodio::source::EmptyCallback::new(Box::new(move || {
            let mut lock = currently_playing.lock().unwrap();
            *lock = None;
        })));

        Ok(())
    }

    pub fn search_song(&self, query: String) -> Vec<(f64, u32)> {
        let possible = self
            .songs
            .iter()
            .filter_map(|a| a.id.map(|i| (a.name.clone(), i)));

        search(&query, possible, 10)
    }

    pub fn get_song_by_id(&self, id: u32) -> Option<&SongInfo> {
        self.song_indices
            .get(&id)
            .and_then(|&idx| self.songs.get(idx))
    }

    pub fn get_playlist_by_id(&self, id: u32) -> Option<&PlaylistInfo> {
        self.playlist_indices
            .get(&id)
            .and_then(|&idx| self.playlists.get(idx))
    }

    pub fn sort(&mut self) {
        self.songs.sort_unstable_by_key(|s| s.id);
        self.song_indices = Self::indices_from_song_list(&self.songs);

        self.playlists.sort_unstable_by_key(|p| p.id);
        self.playlist_indices = Self::indices_from_playlist_list(&self.playlists);
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

    fn indices_from_playlist_list(v: &[PlaylistInfo]) -> HashMap<u32, usize> {
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

    fn read_playlists_from_file() -> std::io::Result<Vec<PlaylistInfo>> {
        let playlist_file = fs::OpenOptions::new()
            .read(true)
            .open(PLAYLISTS_FILE_PATH)?;

        let mut reader = BufReader::new(&playlist_file);
        let mut buf = String::new();
        let mut playlists = Vec::new();
        loop {
            buf.clear();
            let byte_count = reader.read_line(&mut buf)?;
            if byte_count == 0 {
                break; // EOF
            }
            if let Ok(song) = serde_json::from_str::<PlaylistInfo>(&buf) {
                playlists.push(song);
            }
        }
        Ok(playlists)
    }

    pub fn add_song(&mut self, song: SongInfo) -> std::io::Result<()> {
        match song.id {
            Some(i) => {
                if i < self.max_song_id.unwrap_or(0)
                    || self.songs.iter().any(|s| s.id.unwrap_or(u32::MAX) == i)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "Song with this id already exists",
                    ));
                } else {
                    let mut song = song;
                    song.file_entry_up_to_date = false;
                    self.songs.push(song);
                    self.max_song_id = if i > self.max_song_id.unwrap_or(0) {
                        Some(i)
                    } else {
                        self.max_song_id
                    };
                    return Ok(());
                }
            }
            None => {
                let mut song = song;
                song.id = Some(self.max_song_id.unwrap_or(0) + 1);
                song.file_entry_up_to_date = false;
                self.max_song_id = song.id;
                self.songs.push(song);
            }
        }
        self.song_indices = Self::indices_from_song_list(&self.songs);
        Ok(())
    }

    pub fn add_playlist(&mut self, playlist: PlaylistInfo) -> std::io::Result<()> {
        match playlist.id {
            Some(i) => {
                if i < self.max_playlist_id.unwrap_or(0)
                    || self.playlists.iter().any(|s| s.id.unwrap_or(u32::MAX) == i)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "Playlist with this id already exists",
                    ));
                } else {
                    let mut playlist = playlist;
                    playlist.file_entry_up_to_date = false;
                    self.playlists.push(playlist);
                    self.max_playlist_id = if i > self.max_playlist_id.unwrap_or(0) {
                        Some(i)
                    } else {
                        self.max_playlist_id
                    };
                    return Ok(());
                }
            }
            None => {
                let mut playlist = playlist;
                playlist.id = Some(self.max_playlist_id.unwrap_or(0) + 1);
                playlist.file_entry_up_to_date = false;
                self.max_playlist_id = playlist.id;
                self.playlists.push(playlist);
            }
        }
        self.playlist_indices = Self::indices_from_playlist_list(&self.playlists);
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
        } else if self.removed_songs.contains(&id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Song already removed",
            ));
        }

        self.removed_songs.insert(id);
        Ok(())
    }

    pub fn remove_playlist(&mut self, id: u32) -> std::io::Result<()> {
        if !self.playlists.iter().any(|s| match s.id {
            Some(o_id) => id == o_id,
            None => false,
        }) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Playlist with this id doesnt exist",
            ));
        } else if self.removed_playlists.contains(&id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Playlist already removed",
            ));
        }

        self.removed_playlists.insert(id);
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

    pub fn update_playlist(&mut self, playlist: PlaylistInfo) -> std::io::Result<()> {
        let id = match playlist.id {
            Some(i) => i,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "input playlist has no ID",
                ));
            }
        };
        match self.playlists.iter_mut().find(|s| s.id.unwrap_or(0) == id) {
            Some(i) => {
                *i = playlist;
                i.file_entry_up_to_date = false;
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Playlist with this id doesnt exist",
                ));
            }
        }

        Ok(())
    }

    pub fn update_songs_file(&mut self) -> std::io::Result<u32> {
        let mut songs = Self::read_songs_from_file()?;
        songs.retain(|s| self.songs.iter().any(|s2| s.id != s2.id));
        songs.into_iter().for_each(|s| {
            let _ = self.add_song(s);
        });

        let temp_file_path = format!("{}.temp", SONGS_FILE_PATH);
        let temp_song_file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&temp_file_path)?;
        let mut temp_file_writer = BufWriter::new(temp_song_file);

        self.sort();
        let mut count = 0;
        self.songs.iter_mut().for_each(|song| {
            let i = match song.id {
                Some(t) => t,
                None => {
                    println!("Attempted to write song with no id");
                    dbg!(song);
                    return;
                }
            };

            if self.removed_songs.contains(&i) {
                return;
            }

            let json = serde_json::to_string(song);
            match json {
                Ok(j) => {
                    let _ = temp_file_writer.write_all(j.as_bytes());
                    let _ = temp_file_writer.write_all(b"\n");
                    count += 1;
                }
                Err(err) => {
                    println!("Error writing to file. ({})", err);
                }
            }
        });

        fs::rename(temp_file_path, SONGS_FILE_PATH)?;

        self.songs.retain(|s| match s.id {
            Some(i) => !self.removed_songs.contains(&i),
            None => true,
        });

        self.removed_songs.clear();

        self.songs.iter_mut().for_each(|s| {
            s.file_entry_up_to_date = true;
        });

        Ok(count)
    }

    pub fn update_playlist_file(&mut self) -> std::io::Result<u32> {
        let temp_file_path = format!("{}.temp", PLAYLISTS_FILE_PATH);
        let temp_playlist_file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&temp_file_path)?;
        let mut temp_file_writer = BufWriter::new(temp_playlist_file);

        let mut count = 0;
        self.playlists.iter_mut().for_each(|pl| {
            let i = match pl.id {
                Some(t) => t,
                None => {
                    println!("Attempted to write playlist with no id");
                    dbg!(pl);
                    return;
                }
            };

            if self.removed_playlists.contains(&i) {
                return;
            }

            let json = serde_json::to_string(pl);
            match json {
                Ok(j) => {
                    let _ = temp_file_writer.write_all(j.as_bytes());
                    let _ = temp_file_writer.write_all(b"\n");
                    count += 1;
                }
                Err(err) => {
                    println!("Error writing to file. ({})", err);
                }
            }
        });

        fs::rename(temp_file_path, PLAYLISTS_FILE_PATH)?;

        self.playlists.retain(|s| match s.id {
            Some(i) => !self.removed_playlists.contains(&i),
            None => true,
        });

        self.removed_playlists.clear();

        self.playlists.iter_mut().for_each(|s| {
            s.file_entry_up_to_date = true;
        });

        Ok(count)
    }
}
