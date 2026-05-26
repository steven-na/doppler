pub mod command_parser {
    use crate::dlib::{doppler_info::DopplerInfo, song::SongInfo};
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
        songs: &mut DopplerInfo,
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
                songs.add_song(s)?;
            }
            "select" => {
                songs.songs.iter().for_each(|s| {
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
                let idx = songs.indices.get(&c.selected_id.unwrap_or(0));
                if let Some(idx) = idx
                    && let Some(s) = songs.songs.get(*idx)
                {
                    dbg!(s);
                }
            }
            "play" => {
                if let Some(id) = c.selected_id
                    && let Some(&idx) = songs.indices.get(&id)
                    && let Some(_) = songs.songs.get(idx)
                {
                    if let Err(e) = songs.play_song(id) {
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
                    match songs.remove_song(id) {
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
                } else if !songs.songs.iter().any(|s| s.id == c.selected_id) {
                    println!("No song matches selected id");
                    return Ok(CommandOutcome::Carryon);
                } else if songs.songs.is_empty() {
                    println!("No songs in list");
                    return Ok(CommandOutcome::Carryon);
                }
                let mut s = songs.songs[songs
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
                        crate::util::time_util::seconds_to_base60_string(s.duration),
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
                return Ok(CommandOutcome::Exit);
            }
            _ => println!("Invalid input. try help or exit."),
        }
        Ok(CommandOutcome::Carryon)
    }
}
