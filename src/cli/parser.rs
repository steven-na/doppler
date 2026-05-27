pub mod command_parser {
    use crate::dlib::{doppler_info::DopplerInfo, playlist::PlaylistInfo, song::SongInfo};
    use std::io::Write;

    fn input(buf: &mut String, prompt: &str) -> std::io::Result<usize> {
        buf.clear();
        print!("{prompt}");
        let _ = std::io::stdout().flush();
        std::io::stdin().read_line(buf)
    }

    #[derive(Debug)]
    pub enum CommandOutcome {
        Carryon,
        Exit,
    }

    #[derive(Debug)]
    pub struct ProgramState {
        selected_id: Option<u32>,
    }

    impl ProgramState {
        pub fn new() -> Self {
            ProgramState { selected_id: None }
        }
    }

    impl Default for ProgramState {
        fn default() -> Self {
            Self::new()
        }
    }

    pub fn get_input() -> String {
        let mut user_input = String::new();
        let _ = input(&mut user_input, "> ");
        user_input
    }

    pub fn handle(
        user_input: String,
        dinfo: &mut DopplerInfo,
        c: &mut ProgramState,
    ) -> std::io::Result<CommandOutcome> {
        match user_input.as_str().trim() {
            "add" => {
                let mut s = SongInfo::new();
                let mut add_input = String::new();

                let _ = input(&mut add_input, "Name> ");
                s.name = add_input.trim().to_string();
                let _ = input(&mut add_input, "Album> ");
                s.album = add_input.trim().to_string();
                let _ = input(&mut add_input, "Artist> ");
                s.artist = add_input.trim().to_string();
                let _ = input(&mut add_input, "Duration (base 10 seconds)> ");
                s.duration = match add_input.trim().parse::<u32>() {
                    Ok(d) => d,
                    Err(e) => {
                        println!("Error parsing duration ({e})");
                        return Ok(CommandOutcome::Carryon);
                    }
                };
                dinfo.add_song(s)?;
            }
            "select" => {
                dinfo.songs.iter().for_each(|s| {
                    println!("[{}] {} by {}", s.id.unwrap_or(0), s.name, s.artist);
                });
                let mut select_input = String::new();
                let _ = input(&mut select_input, "Select [id]> ");
                c.selected_id = match select_input.trim().parse::<u32>() {
                    Ok(d) => Some(d),
                    Err(e) => {
                        c.selected_id = None;
                        println!("Error parsing id ({e})");
                        return Ok(CommandOutcome::Carryon);
                    }
                };
                let idx = dinfo.song_indices.get(&c.selected_id.unwrap_or(0));
                if let Some(idx) = idx
                    && let Some(s) = dinfo.songs.get(*idx)
                {
                    dbg!(s);
                }
            }
            "play" => {
                if let Some(id) = c.selected_id
                    && let Some(&idx) = dinfo.song_indices.get(&id)
                    && let Some(_) = dinfo.songs.get(idx)
                {
                    if let Err(e) = dinfo.play_song(id) {
                        println!("Failed to play song ({})", e);
                    }
                    return Ok(CommandOutcome::Carryon);
                }
                match &c.selected_id {
                    Some(id) => println!("Invalid song selected {}", id),
                    None => println!("No song selected"),
                };
            }
            "remove" => {
                if let Some(id) = c.selected_id {
                    match dinfo.remove_song(id) {
                        Ok(_) => {
                            println!("Removed song");
                            c.selected_id = None;
                        }
                        Err(err) => println!("Failed to remove song ({})", err),
                    }
                } else {
                    println!("No song selected");
                }
            }
            "update" => {
                if c.selected_id.is_none() {
                    println!("No song selected");
                    return Ok(CommandOutcome::Carryon);
                } else if !dinfo.songs.iter().any(|s| s.id == c.selected_id) {
                    println!("No song matches selected id");
                    return Ok(CommandOutcome::Carryon);
                } else if dinfo.songs.is_empty() {
                    println!("No songs in list");
                    return Ok(CommandOutcome::Carryon);
                }
                let mut s = dinfo.songs[dinfo
                    .songs
                    .iter()
                    .position(|s| s.id == c.selected_id)
                    .unwrap_or(0)]
                .clone();

                println!("Leave any field blank to not update");

                let mut update_input = String::new();
                let _ = input(&mut update_input, "Name> ");
                if !update_input.trim().is_empty() {
                    s.name = update_input.trim().to_string();
                }
                let _ = input(&mut update_input, "Album> ");
                if !update_input.trim().is_empty() {
                    s.album = update_input.trim().to_string();
                }
                let _ = input(&mut update_input, "Artist> ");
                if !update_input.trim().is_empty() {
                    s.artist = update_input.trim().to_string();
                }
                let _ = input(&mut update_input, "Duration (base 10 seconds)> ");

                if !update_input.trim().is_empty() {
                    s.duration = match update_input.trim().parse::<u32>() {
                        Ok(d) => d,
                        Err(e) => {
                            print!("Error parsing duration ({e})");
                            return Ok(CommandOutcome::Carryon);
                        }
                    };
                }
                let _ = dinfo.update_song(s);
            }
            "list" => {
                println!("All songs: ");
                dinfo.sort();
                dinfo.songs.iter().for_each(|s| {
                    let removed = if let Some(id) = &s.id {
                        if dinfo.removed_songs.contains(id) {
                            "~"
                        } else {
                            ""
                        }
                    } else {
                        ""
                    };
                    let modified = if s.file_entry_up_to_date { "" } else { "*" };
                    println!("{removed}{modified}{}", s);
                });
            }
            "write" => {
                match dinfo.update_songs_file() {
                    Ok(c) => println!("Wrote {} songs to file", c),
                    Err(err) => println!("Failed to write songs to file ({})", err),
                };
                match dinfo.update_playlist_file() {
                    Ok(c) => println!("Wrote {} playlists to file", c),
                    Err(err) => println!("Failed to write playlists to file ({})", err),
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
                return Ok(CommandOutcome::Exit);
            }
            "playlist" | "pl" => match handle_playlist(dinfo) {
                Ok(i) => return Ok(i),
                Err(err) => {
                    println!("Playlist interaction failed ({})", err);
                    return Ok(CommandOutcome::Carryon);
                }
            },
            _ => println!("Invalid input. try help or exit."),
        }
        Ok(CommandOutcome::Carryon)
    }

    fn handle_playlist(dinfo: &mut DopplerInfo) -> std::io::Result<CommandOutcome> {
        let mut selected_playlist_id = None;
        loop {
            let mut buf = String::new();
            let selected = if let Some(id) = selected_playlist_id {
                format!("{}", id)
            } else {
                "".to_string()
            };
            let _ = input(&mut buf, format!("{selected}$ ").as_str());
            match buf.as_str().trim() {
                "add" => {
                    let mut p = PlaylistInfo::new();
                    let mut add_input = String::new();

                    let _ = input(&mut add_input, "Name> ")?;
                    p.name = add_input.trim().to_string();
                    dinfo.add_playlist(p)?;
                }
                "select" => {
                    dinfo.playlists.iter().for_each(|p| {
                        println!("[{}] {}", p.id.unwrap_or(0), p.name);
                    });
                    let mut select_input = String::new();
                    let _ = input(&mut select_input, "Select [id]> ");
                    selected_playlist_id = match select_input.trim().parse::<u32>() {
                        Ok(d) => Some(d),
                        Err(e) => {
                            selected_playlist_id = None;
                            println!("Error parsing id ({e})");
                            continue;
                        }
                    };
                    let idx = dinfo
                        .playlist_indices
                        .get(&selected_playlist_id.unwrap_or(0));
                    if let Some(idx) = idx
                        && let Some(s) = dinfo.playlists.get(*idx)
                    {
                        dbg!(s);
                    }
                }
                "addsongs" => {
                    if selected_playlist_id.is_none() {
                        println!("No playlist selected");
                        continue;
                    } else if let Some(id) = selected_playlist_id
                        && let Some(&idx) = dinfo.playlist_indices.get(&id)
                        && let Some(p) = dinfo.playlists.get_mut(idx)
                    {
                        dinfo.songs.iter().for_each(|s| {
                            println!("[{}] {}", s.id.unwrap_or(0), s);
                        });

                        loop {
                            let inp = get_input();
                            if inp.trim() == "list" {
                                p.songs.iter().enumerate().for_each(|(i, id)| {
                                    if let Some(s) = dinfo
                                        .song_indices
                                        .get(id)
                                        .and_then(|&idx| dinfo.songs.get(idx))
                                    {
                                        println!("[{i}] {s}");
                                    }
                                });
                                continue;
                            } else if inp.trim().starts_with("r")
                                && let Ok(idx) = inp
                                    .as_str()
                                    .trim()
                                    .strip_prefix("r")
                                    .unwrap_or("")
                                    .parse::<usize>()
                                && idx < p.songs.len()
                            {
                                match p.remove_song(idx) {
                                    Ok(()) => {
                                        println!("Removed song at index {}", idx);
                                        continue;
                                    }
                                    Err(err) => {
                                        println!("Error removing song ({})", err);
                                        continue;
                                    }
                                }
                            }
                            match inp.as_str().trim().parse::<u32>() {
                                Ok(id) => {
                                    if !dinfo.songs.iter().any(|s| s.id == Some(id)) {
                                        println!("No songs with this id");
                                        continue;
                                    }
                                    match p.add_song(id) {
                                        Ok(()) => println!("Added song."),
                                        Err(err) => match err.kind() {
                                            std::io::ErrorKind::AlreadyExists => {
                                                println!("Song in playlist. Add anyways? (y/n)");
                                                match get_input().as_str().trim() {
                                                    "y" => p.force_add_song(id),
                                                    _ => continue,
                                                }
                                            }
                                            _ => {
                                                println!("Failed to add song ({})", err);
                                                break;
                                            }
                                        },
                                    }
                                }
                                Err(err) => {
                                    println!("Didn't add song ({})", err);
                                    break;
                                }
                            }
                        }
                    }
                }
                "remove" => match selected_playlist_id {
                    None => println!("No playlist selected"),
                    Some(id) => match dinfo.remove_playlist(id) {
                        Ok(()) => println!("Removed playlist. (write to save change)"),
                        Err(err) => println!("Error removing playlist ({})", err),
                    },
                },
                "list" => {
                    println!("All Playlists: ");
                    dinfo.sort();
                    dinfo.playlists.iter().enumerate().for_each(|(idx, p)| {
                        let removed = if let Some(id) = &p.id {
                            if dinfo.removed_playlists.contains(id) {
                                "~"
                            } else {
                                ""
                            }
                        } else {
                            ""
                        };
                        let modified = if p.file_entry_up_to_date { "" } else { "*" };
                        println!("{removed}{modified}{}", p.name);
                        p.songs.iter().for_each(|p| {
                            let s = dinfo
                                .song_indices
                                .get(p)
                                .and_then(|&idx| dinfo.songs.get(idx));

                            if let Some(s) = s {
                                println!("{}", s);
                            }
                        });
                        if dinfo.playlists.len() > 1 && idx < dinfo.playlists.len() - 1 {
                            println!("---");
                        }
                    });
                }
                "help" => {
                    fn print_help(cmd: &str, desc: &str) {
                        println!("\"{cmd}\"\n\t{desc}");
                    }
                    println!("Playlist help (type nothing to exit playlist mode):");
                    print_help("add", "adds a playlist");
                    print_help("select", "select a playlist");
                    print_help("addsongs", "add songs to selected playlist");
                    print_help("list", "list playlists and the songs in them");
                }
                _ => break,
            }
        }

        Ok(CommandOutcome::Carryon)
    }
}
