pub mod playlist {
    use crate::{
        cli::parser::command_parser::{ProgramState, get_input, input},
        dlib::{doppler_info::DopplerInfo, playlist::PlaylistInfo},
        util::print_util::print_help,
    };

    pub fn add(dinfo: &mut DopplerInfo) {
        let mut p = PlaylistInfo::new();
        let mut add_input = String::new();

        let _ = input(&mut add_input, "Name> ");
        p.name = add_input.trim().to_string();
        match dinfo.add_playlist(p) {
            Ok(()) => println!("Added playlist"),
            Err(err) => println!("Failed to add playlist ({})", err),
        }
    }

    pub fn update(dinfo: &mut DopplerInfo, c: &ProgramState) {
        if c.selected_id.is_none() {
            println!("No playlist selected");
            return;
        } else if !dinfo.playlists.iter().any(|s| s.id == c.selected_id) {
            println!("No playlists matches selected id");
            return;
        } else if dinfo.playlists.is_empty() {
            println!("No playlist in list");
            return;
        }
        let mut p = dinfo.playlists[dinfo
            .playlists
            .iter()
            .position(|s| s.id == c.selected_id)
            .unwrap_or(0)]
        .clone();

        println!("Leave any field blank to not update");

        let mut update_input = String::new();
        let _ = input(&mut update_input, "Name> ");
        if !update_input.trim().is_empty() {
            p.name = update_input.trim().to_string();
        }

        match dinfo.update_playlist(p) {
            Ok(()) => println!("Updated playlist"),
            Err(err) => println!("Failed to update playlist ({})", err),
        }
    }

    pub fn select(dinfo: &mut DopplerInfo, c: &mut ProgramState) {
        dinfo.playlists.iter().for_each(|p| {
            println!("[{}] {}", p.id.unwrap_or(0), p.name);
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
        let idx = dinfo.playlist_indices.get(&c.selected_id.unwrap_or(0));
        if let Some(idx) = idx
            && let Some(s) = dinfo.playlists.get(*idx)
        {
            dbg!(s);
        }
    }

    pub fn add_songs(dinfo: &mut DopplerInfo, c: &ProgramState) {
        if c.selected_id.is_none() {
            println!("No playlist selected");
        } else if let Some(id) = c.selected_id
            && let Some(&idx) = dinfo.playlist_indices.get(&id)
            && let Some(p) = dinfo.playlists.get_mut(idx)
        {
            dinfo.songs.iter().for_each(|s| {
                println!("[{}] {}", s.id.unwrap_or(0), s);
            });

            loop {
                let inp = get_input(None);
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
                    match p.remove_song_by_index(idx) {
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
                                    match get_input(Some("y/n> ".to_string())).as_str().trim() {
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

    pub fn remove(dinfo: &mut DopplerInfo, c: &ProgramState) {
        match c.selected_id {
            None => println!("No playlist selected"),
            Some(id) => match dinfo.remove_playlist(id) {
                Ok(()) => println!("Removed playlist. (write to save change)"),
                Err(err) => println!("Error removing playlist ({})", err),
            },
        }
    }

    pub fn list(dinfo: &mut DopplerInfo) {
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

    pub fn help() {
        println!("Playlist help (type nothing to exit playlist mode):");
        print_help("(a)dd", "adds a playlist");
        print_help("(u)pdate", "update selected playlist");
        print_help("(s)elect", "select a playlist");
        print_help("(a)dd(s)ongs", "add songs to selected playlist");
        print_help("(r)emove", "remove selected playlist");
        print_help("(l)ist", "list playlists and the songs in them");
    }
}
