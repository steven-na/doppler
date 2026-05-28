pub mod song {
    use crate::{
        cli::parser::command_parser::{ProgramState, get_input, input},
        dlib::{doppler_info::DopplerInfo, song::SongInfo},
        util::print_util::print_help,
    };

    pub fn add(dinfo: &mut DopplerInfo) {
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
                return;
            }
        };
        if let Err(err) = dinfo.add_song(s) {
            println!("Error adding song ({})", err);
        } else {
            println!("Added song");
        }
    }

    pub fn select(dinfo: &mut DopplerInfo, c: &mut ProgramState) {
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
                return;
            }
        };
        let idx = dinfo.song_indices.get(&c.selected_id.unwrap_or(0));
        if let Some(idx) = idx
            && let Some(s) = dinfo.songs.get(*idx)
        {
            dbg!(s);
        }
    }

    pub fn play(dinfo: &mut DopplerInfo, c: &mut ProgramState, player: &rodio::Player) {
        if c.selected_id.is_none() {
            println!("No song selected");
            return;
        }
        if let Some(id) = c.selected_id {
            if let Err(e) = dinfo.play_song(id, player) {
                println!("Failed to play song ({})", e);
            } else {
                if let Some(song) = dinfo.get_song_by_id(id) {
                    let duration_str =
                        crate::util::time_util::seconds_to_base60_string(song.duration);
                    let playback_time = player.get_pos().as_secs() as u32;
                    let playback_time =
                        crate::util::time_util::seconds_to_base60_string(playback_time);
                    println!("Now playing ({playback_time}/{duration_str}): {}", song);
                }
            }
        }
    }

    pub fn remove(dinfo: &mut DopplerInfo, c: &mut ProgramState) {
        if let Some(id) = c.selected_id {
            match dinfo.remove_song(id) {
                Ok(()) => {
                    println!("Removed song");
                    c.selected_id = None;
                }
                Err(err) => println!("Failed to remove song ({})", err),
            }
        } else {
            println!("No song selected");
        }
    }

    pub fn update(dinfo: &mut DopplerInfo, c: &mut ProgramState) {
        if c.selected_id.is_none() {
            println!("No song selected");
            return;
        } else if !dinfo.songs.iter().any(|s| s.id == c.selected_id) {
            println!("No song matches selected id");
            return;
        } else if dinfo.songs.is_empty() {
            println!("No songs in list");
            return;
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
                    return;
                }
            };
        }
        match dinfo.update_song(s) {
            Ok(()) => println!("Updated song"),
            Err(err) => println!("Failed to update song ({})", err),
        }
    }

    pub fn list(dinfo: &mut DopplerInfo) {
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

    pub fn search(dinfo: &DopplerInfo) {
        let query = get_input(Some("Search> ".to_string()));
        let matches = dinfo.search_song(query);

        matches.iter().for_each(|(w, id)| {
            if let Some(s) = dinfo.get_song_by_id(*id) {
                println!("{:.2} [{}] {}", w, id, s);
            }
        });
    }

    pub fn help() {
        println!("Song help:");
        print_help("(p)lay", "play selected song");
        print_help("(x)un/pause", "toggle paused state");
        print_help("(a)dd", "adds a song to the system.");
        print_help("(l)ist", "list all songs");
        print_help("(s)elect", "select a song for updating or removal");
        print_help("(ss)earch", "search for a song");
        print_help("c|update", "update fields for selected song");
        print_help(
            "(r)emove",
            "remove selected song from songs.\n\tDoes not update until changes are written",
        );
        print_help("(w)rite", "write changes to file");
        print_help("(pl)aylist", "enter playlist edit mode");
    }
}
