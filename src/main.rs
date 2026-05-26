use std::io::Write;

use crate::dlib::{doppler_info::*, playlist::*, song::*};
pub mod dlib;

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
    todo!("Playlists and better command handling");

    let mut songs = DopplerInfo::new()?;

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
