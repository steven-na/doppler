use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
};

use serde::{Deserialize, Serialize};

const SONGS_FILE_PATH: &str = "./data/songs.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SongInfo {
    name: String,
    artist: String,
    album: String,
    duration: u32,
    filename: Option<String>,
    id: Option<u32>,
    #[serde(skip)]
    file_entry_up_to_date: bool,
}

impl SongInfo {
    fn new() -> Self {
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

#[derive(Debug)]
struct AllSongs {
    songs: Vec<SongInfo>,
    indices: HashMap<u32, usize>,
    removed: HashSet<u32>,
    max_id: Option<u32>,
}

impl AllSongs {
    fn new() -> std::io::Result<Self> {
        let mut songs = Self::read_songs_from_file()?;
        songs.iter_mut().for_each(|s| {
            s.file_entry_up_to_date = true;
        });
        let indices = Self::indices_from_song_list(&songs);
        let max_id = songs.iter().map(|s| s.id.unwrap_or(0)).max();

        Ok(AllSongs {
            songs,
            indices,
            removed: HashSet::new(),
            max_id,
        })
    }

    fn sort(&mut self) {
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

    fn add_song(&mut self, song: SongInfo) -> std::io::Result<()> {
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

    fn remove_song(&mut self, id: u32) -> std::io::Result<()> {
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

    fn update_song(&mut self, song: SongInfo) -> std::io::Result<()> {
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

    fn update_songs_file(&mut self) -> std::io::Result<()> {
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

fn seconds_to_base60_string(s: u32) -> String {
    if s >= 60 * 60 {
        let mut minutes = s / 60;
        while minutes > 60 {
            minutes -= 60;
        }
        format!("{}:{:02}:{:02}", s / (60 * 60), minutes, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn play_song(song: &SongInfo) {
    let duration_str = seconds_to_base60_string(song.duration);
    println!(
        "Now playing (0:00/{duration_str}): {0} ({1}) by {2}",
        song.name, song.album, song.artist
    );
}

fn input(buf: &mut String, prompt: &str) -> std::io::Result<usize> {
    buf.clear();
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    std::io::stdin().read_line(buf)
}

fn main() -> std::io::Result<()> {
    let mut songs = AllSongs::new()?;

    let mut user_input = String::new();
    let mut selected_id: Option<u32> = None;
    loop {
        let prompt = match &selected_id {
            Some(i) => format!("{}> ", i),
            None => "> ".to_string(),
        };
        let _ = input(&mut user_input, &prompt);
        match user_input.as_str().trim() {
            "add" => {
                let mut s = SongInfo::new();

                let _ = input(&mut user_input, "Name> ");
                s.name = user_input.trim().to_string();
                let _ = input(&mut user_input, "Album> ");
                s.album = user_input.trim().to_string();
                let _ = input(&mut user_input, "Artist> ");
                s.artist = user_input.trim().to_string();
                let _ = input(&mut user_input, "Duration (base 10 seconds)> ");
                s.duration = match user_input.trim().parse::<u32>() {
                    Ok(d) => d,
                    Err(e) => {
                        println!("Error parsing duration ({e})");
                        continue;
                    }
                };
                songs.add_song(s)?;
            }
            "select" => {
                songs.songs.iter().for_each(|s| {
                    println!("[{}] {} by {}", s.id.unwrap_or(0), s.name, s.artist);
                });
                let _ = input(&mut user_input, "Select [id]> ");
                selected_id = match user_input.trim().parse::<u32>() {
                    Ok(d) => Some(d),
                    Err(e) => {
                        selected_id = None;
                        println!("Error parsing id ({e})");
                        continue;
                    }
                };
                let idx = songs.indices.get(&selected_id.unwrap_or(0));
                if let Some(idx) = idx
                    && let Some(s) = songs.songs.get(*idx)
                {
                    dbg!(s);
                }
            }
            "play" => {
                if let Some(id) = selected_id
                    && let Some(&idx) = songs.indices.get(&id)
                    && let Some(s) = songs.songs.get(idx)
                {
                    play_song(s);
                    continue;
                }
                match &selected_id {
                    Some(id) => println!("Invalid song selected {}", id),
                    None => println!("No song selected"),
                };
            }
            "remove" => {
                if let Some(id) = selected_id {
                    match songs.remove_song(id) {
                        Ok(_) => {
                            println!("Removed song");
                            selected_id = None;
                        }
                        Err(err) => println!("Failed to remove song ({})", err),
                    }
                } else {
                    println!("No song selected");
                }
            }
            "update" => {
                if selected_id.is_none() {
                    println!("No song selected");
                    continue;
                } else if !songs.songs.iter().any(|s| s.id == selected_id) {
                    println!("No song matches selected id");
                    continue;
                } else if songs.songs.is_empty() {
                    println!("No songs in list");
                    continue;
                }
                let mut s = songs.songs[songs
                    .songs
                    .iter()
                    .position(|s| s.id == selected_id)
                    .unwrap_or(0)]
                .clone();

                println!("Leave any field blank to not update");

                let _ = input(&mut user_input, "Name> ");
                if !user_input.trim().is_empty() {
                    s.name = user_input.trim().to_string();
                }
                let _ = input(&mut user_input, "Album> ");
                if !user_input.trim().is_empty() {
                    s.album = user_input.trim().to_string();
                }
                let _ = input(&mut user_input, "Artist> ");
                if !user_input.trim().is_empty() {
                    s.artist = user_input.trim().to_string();
                }
                let _ = input(&mut user_input, "Duration (base 10 seconds)> ");

                if !user_input.trim().is_empty() {
                    s.duration = match user_input.trim().parse::<u32>() {
                        Ok(d) => d,
                        Err(e) => {
                            print!("Error parsing duration ({e})");
                            continue;
                        }
                    };
                }
                let _ = songs.update_song(s);
            }
            "list" => {
                println!("All songs: ");
                songs.sort();
                songs.songs.iter().for_each(|s| {
                    let removed = if let Some(id) = &s.id {
                        if songs.removed.contains(id) { "~" } else { "" }
                    } else {
                        ""
                    };
                    let modified = if s.file_entry_up_to_date { "" } else { "*" };
                    println!(
                        "{removed}{modified}{} ({})\n\tby {} on {}",
                        s.name,
                        seconds_to_base60_string(s.duration),
                        s.artist,
                        s.album,
                    );
                });
            }
            "write" => {
                match songs.update_songs_file() {
                    Ok(()) => println!("Wrote songs to file"),
                    Err(err) => println!("Failed to write songs to file ({})", err),
                };
            }
            "help" => {
                fn print_help(cmd: &str, desc: &str) {
                    println!("\"{cmd}\"\n\t{desc}");
                }
                print_help("play", "play selected song");
                print_help("add", "adds a song to the system.");
                print_help("list", "list all songs");
                print_help("select", "select a song for updating or removal");
                print_help("update", "update fields for selected song");
                print_help(
                    "remove",
                    "remove selected song from songs.\n\tDoes not update until changes are written",
                );
                print_help("write", "write changes to file");
            }
            "exit" => {
                println!("Exiting...");
                break;
            }
            _ => println!("Invalid input. try help or exit."),
        }
    }

    Ok(())
}
