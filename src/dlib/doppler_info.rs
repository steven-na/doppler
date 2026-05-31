use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufReader, BufWriter, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::{fs, io};

use crate::dlib::playlist_iter::PlaylistIter;
use crate::dlib::{playlist::*, song::*};
use crate::tui::app::AppEvent;
use crate::tui::tables::QueueEntry;
use crate::util::search_utli::search;

pub const SONGS_FILE_PATH: &str = "./data/songs.json";
pub const PLAYLISTS_FILE_PATH: &str = "./data/playlists.json";

pub struct DopplerInfo {
    pub songs: Arc<RwLock<Vec<SongInfo>>>,
    pub playlists: Vec<PlaylistInfo>,

    pub song_indices: Arc<RwLock<HashMap<u32, usize>>>,
    pub playlist_indices: HashMap<u32, usize>,

    removed_songs: HashSet<u32>,
    removed_playlists: HashSet<u32>,

    max_song_id: Option<u32>,
    max_playlist_id: Option<u32>,

    pub enqueued_playlist: Arc<Mutex<Option<PlaylistIter>>>,
    pub queue_dirty: Arc<AtomicBool>,
    pub currently_playing: Arc<Mutex<Option<u32>>>,
    pub player: Arc<Mutex<rodio::Player>>,
    app_notifier: mpsc::Sender<AppEvent>,
}

impl DopplerInfo {
    pub fn new(player: rodio::Player, sender: mpsc::Sender<AppEvent>) -> io::Result<Self> {
        let mut songs = read_songs_from_file()?;
        songs.iter_mut().for_each(|s| {
            s.file_entry_up_to_date = true;
        });
        let mut playlists = read_playlists_from_file()?;
        playlists.iter_mut().for_each(|p| {
            p.file_entry_up_to_date = true;
        });

        let song_indices = indices_from_song_list(&songs);
        let max_song_id = songs.iter().map(|s| s.id.unwrap_or(0)).max();

        let playlist_indices = indices_from_playlist_list(&playlists);
        let max_playlist_id = playlists.iter().map(|s| s.id.unwrap_or(0)).max();

        Ok(DopplerInfo {
            songs: Arc::new(RwLock::new(songs)),
            playlists,
            song_indices: Arc::new(RwLock::new(song_indices)),
            playlist_indices,
            removed_songs: HashSet::new(),
            removed_playlists: HashSet::new(),
            max_song_id,
            max_playlist_id,
            enqueued_playlist: Arc::new(Mutex::new(None)),
            queue_dirty: Arc::new(AtomicBool::new(true)),
            currently_playing: Arc::new(Mutex::new(None)),
            player: Arc::new(Mutex::new(player)),
            app_notifier: sender,
        })
    }

    pub fn play_song(&mut self, id: u32) -> io::Result<()> {
        if self.get_song_by_id(id).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No song with this id exists",
            ));
        }

        let ss = Arc::clone(&self.songs);
        let si = Arc::clone(&self.song_indices);
        let song_source = audio_source_from_song_id(id, &ss, &si).ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "Song has no associated file",
        ))?;

        let p_lock = self.player.lock().unwrap();
        p_lock.clear();
        p_lock.append(song_source);
        p_lock.play();
        drop(p_lock);
        let mut cs_lock = self.currently_playing.lock().unwrap();
        *cs_lock = Some(id);
        drop(cs_lock);
        self.queue_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.app_notifier.send(AppEvent::Song).unwrap();

        enqueue_next_callback(
            self.player.clone(),
            self.enqueued_playlist.clone(),
            self.currently_playing.clone(),
            self.songs.clone(),
            self.song_indices.clone(),
            self.queue_dirty.clone(),
            self.app_notifier.clone(),
        );

        Ok(())
    }

    pub fn enqueue_song(&mut self, id: u32) -> io::Result<()> {
        if self.get_song_by_id(id).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No song with this id exists",
            ));
        }

        let mut q_lock = self.enqueued_playlist.lock().unwrap();
        if q_lock.is_none() {
            *q_lock = Some(PlaylistIter {
                queue: VecDeque::new(),
                last_song: None,
            })
        }
        let q = q_lock.as_mut().ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "Failed to insert queue",
        ))?;
        q.insert_back(id);
        drop(q_lock);
        self.queue_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        self.app_notifier.send(AppEvent::Song).unwrap();

        if self.player.lock().unwrap().len() == 0 {
            enqueue_next_callback(
                self.player.clone(),
                self.enqueued_playlist.clone(),
                self.currently_playing.clone(),
                self.songs.clone(),
                self.song_indices.clone(),
                self.queue_dirty.clone(),
                self.app_notifier.clone(),
            );
        }

        Ok(())
    }

    pub fn skip_song(&mut self) {
        self.player.lock().unwrap().skip_one();
    }

    pub fn play_playlist(&mut self, id: u32, shuffle: bool) -> io::Result<()> {
        let mut pi = match self.get_playlist_by_id(id) {
            Some(p) => p.dynamic_iter(),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No playlist with this id exists",
                ));
            }
        };

        if shuffle {
            pi.shuffle();
        }

        let first_song = pi.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "No songs in playlist")
        })?;

        self.play_song(first_song)?;

        enqueue_next_callback(
            self.player.clone(),
            self.enqueued_playlist.clone(),
            self.currently_playing.clone(),
            self.songs.clone(),
            self.song_indices.clone(),
            self.queue_dirty.clone(),
            self.app_notifier.clone(),
        );

        // Store the iterator
        let mut lock = self.enqueued_playlist.lock().unwrap();
        *lock = Some(pi);

        Ok(())
    }

    pub fn queue_entries(&self) -> Vec<QueueEntry> {
        let lock = self.enqueued_playlist.lock().unwrap();
        let Some(ref pi) = *lock else { return vec![] };
        let songs = self.songs.read().unwrap();
        let indices = self.song_indices.read().unwrap();

        pi.queue
            .iter()
            .enumerate()
            .filter_map(|(pos, &(id, _))| {
                let &idx = indices.get(&id)?;
                let song = songs.get(idx)?;
                Some(QueueEntry {
                    position: pos + 1,
                    song_id: id,
                    song_name: song.name.clone(),
                    duration: song.duration,
                })
            })
            .collect()
    }

    pub fn search_song(&self, query: String) -> Vec<(f64, u32)> {
        let songs = self.songs.read().unwrap();
        let possible = songs
            .iter()
            .filter_map(|a| a.id.map(|i| (a.name.clone(), i)));
        search(&query, possible, 10)
    }

    pub fn get_song_by_id(&self, id: u32) -> Option<SongInfo> {
        let songs = self.songs.read().unwrap();
        let indices = self.song_indices.read().unwrap();
        indices.get(&id).and_then(|&idx| songs.get(idx)).cloned()
    }

    pub fn get_playlist_by_id(&self, id: u32) -> Option<&PlaylistInfo> {
        self.playlist_indices
            .get(&id)
            .and_then(|&idx| self.playlists.get(idx))
    }

    pub fn get_song_progress(&self) -> u32 {
        self.player.lock().unwrap().get_pos().as_secs() as u32
    }

    pub fn sort(&mut self) {
        {
            let mut songs = self.songs.write().unwrap();
            songs.sort_unstable_by_key(|s| s.id);
            let new_indices = indices_from_song_list(&songs);
            drop(songs);
            *self.song_indices.write().unwrap() = new_indices;
        }

        self.playlists.sort_unstable_by_key(|p| p.id);
        self.playlist_indices = indices_from_playlist_list(&self.playlists);
    }

    pub fn add_song(&mut self, song: SongInfo) -> io::Result<()> {
        let mut songs = self.songs.write().unwrap();
        match song.id {
            Some(i) => {
                if i < self.max_song_id.unwrap_or(0)
                    || songs.iter().any(|s| s.id.unwrap_or(u32::MAX) == i)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "Song with this id already exists",
                    ));
                } else {
                    let mut song = song;
                    song.file_entry_up_to_date = false;
                    self.max_song_id = if i > self.max_song_id.unwrap_or(0) {
                        Some(i)
                    } else {
                        self.max_song_id
                    };
                    songs.push(song);
                    return Ok(());
                }
            }
            None => {
                let mut song = song;
                song.id = Some(self.max_song_id.unwrap_or(0) + 1);
                song.file_entry_up_to_date = false;
                self.max_song_id = song.id;
                songs.push(song);
            }
        }
        let new_indices = indices_from_song_list(&songs);
        drop(songs);
        *self.song_indices.write().unwrap() = new_indices;
        Ok(())
    }

    pub fn add_playlist(&mut self, playlist: PlaylistInfo) -> io::Result<()> {
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
                    self.max_playlist_id = if i > self.max_playlist_id.unwrap_or(0) {
                        Some(i)
                    } else {
                        self.max_playlist_id
                    };
                    self.playlists.push(playlist);
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
        self.playlist_indices = indices_from_playlist_list(&self.playlists);
        Ok(())
    }

    pub fn remove_song(&mut self, id: u32) -> io::Result<()> {
        let songs = self.songs.read().unwrap();
        if !songs.iter().any(|s| s.id == Some(id)) {
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
        drop(songs);
        self.removed_songs.insert(id);
        Ok(())
    }

    pub fn remove_playlist(&mut self, id: u32) -> io::Result<()> {
        if !self.playlists.iter().any(|s| s.id == Some(id)) {
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

    pub fn update_song(&mut self, song: SongInfo) -> io::Result<()> {
        let id = song.id.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "input song has no ID")
        })?;
        let mut songs = self.songs.write().unwrap();
        match songs.iter_mut().find(|s| s.id.unwrap_or(0) == id) {
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

    pub fn update_playlist(&mut self, playlist: PlaylistInfo) -> io::Result<()> {
        let id = playlist.id.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "input playlist has no ID")
        })?;
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

    pub fn update_songs_file(&mut self) -> io::Result<u32> {
        let file_songs = read_songs_from_file()?;
        {
            let songs = self.songs.read().unwrap();
            let to_add: Vec<SongInfo> = file_songs
                .into_iter()
                .filter(|s| !songs.iter().any(|s2| s.id == s2.id))
                .collect();
            drop(songs);
            for s in to_add {
                let _ = self.add_song(s);
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

        let mut count = 0;
        {
            let mut songs = self.songs.write().unwrap();
            songs.iter_mut().for_each(|song| {
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
                match serde_json::to_string(song) {
                    Ok(j) => {
                        let _ = temp_file_writer.write_all(j.as_bytes());
                        let _ = temp_file_writer.write_all(b"\n");
                        count += 1;
                    }
                    Err(err) => println!("Error writing to file. ({})", err),
                }
            });

            fs::rename(&temp_file_path, SONGS_FILE_PATH)?;

            songs.retain(|s| match s.id {
                Some(i) => !self.removed_songs.contains(&i),
                None => true,
            });
            songs.iter_mut().for_each(|s| {
                s.file_entry_up_to_date = true;
            });
        }

        self.removed_songs.clear();
        let songs = self.songs.read().unwrap();
        *self.song_indices.write().unwrap() = indices_from_song_list(&songs);

        Ok(count)
    }

    pub fn update_playlist_file(&mut self) -> io::Result<u32> {
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
            match serde_json::to_string(pl) {
                Ok(j) => {
                    let _ = temp_file_writer.write_all(j.as_bytes());
                    let _ = temp_file_writer.write_all(b"\n");
                    count += 1;
                }
                Err(err) => println!("Error writing to file. ({})", err),
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

fn enqueue_next_callback(
    player: Arc<Mutex<rodio::Player>>,
    queue: Arc<Mutex<Option<PlaylistIter>>>,
    current_song: Arc<Mutex<Option<u32>>>,
    songs: Arc<RwLock<Vec<SongInfo>>>,
    song_indices: Arc<RwLock<HashMap<u32, usize>>>,
    queue_dirty: Arc<AtomicBool>,
    sender: mpsc::Sender<AppEvent>,
) {
    let p = Arc::clone(&player);
    let q = Arc::clone(&queue);
    let cs = Arc::clone(&current_song);
    let ss = Arc::clone(&songs);
    let si = Arc::clone(&song_indices);
    let qd = Arc::clone(&queue_dirty);

    sender.send(AppEvent::Song).unwrap();

    player
        .lock()
        .unwrap()
        .append(rodio::source::EmptyCallback::new(Box::new(move || {
            qd.store(true, std::sync::atomic::Ordering::Relaxed);
            let mut cs_lock = cs.lock().unwrap();
            *cs_lock = None;

            let mut q_lock = q.lock().unwrap();
            let next = q_lock.as_mut().and_then(|queue| queue.next());

            if let Some(next_id) = next {
                *cs_lock = Some(next_id);
                drop(cs_lock);
                drop(q_lock);
                match audio_source_from_song_id(next_id, &ss, &si) {
                    Some(source) => {
                        p.lock().unwrap().append(source);

                        enqueue_next_callback(
                            Arc::clone(&p),
                            Arc::clone(&q),
                            Arc::clone(&cs),
                            Arc::clone(&ss),
                            Arc::clone(&si),
                            Arc::clone(&qd),
                            sender.clone(),
                        );
                    }
                    None => {
                        eprintln!("Could not load song id={next_id}, skipping");
                    }
                }
            }
        })));
}

fn audio_source_from_song_id(
    id: u32,
    songs: &Arc<RwLock<Vec<SongInfo>>>,
    song_indices: &Arc<RwLock<HashMap<u32, usize>>>,
) -> Option<rodio::Decoder<BufReader<fs::File>>> {
    let songs = songs.read().unwrap();
    let indices = song_indices.read().unwrap();
    let idx = indices.get(&id)?;
    let song = songs.get(*idx)?;
    let path = song.filename.as_ref()?;
    let audio_file = BufReader::new(fs::File::open(path).ok()?);
    rodio::Decoder::try_from(audio_file).ok()
}
