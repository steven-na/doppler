pub mod command_parser {
    use crate::{
        cli::{playlist_procs::playlist, song_procs::song},
        dlib::doppler_info::DopplerInfo,
    };
    use std::io::Write;

    pub fn main_loop(player: &rodio::Player) -> std::io::Result<()> {
        let mut dinfo = DopplerInfo::new()?;
        let mut c: ProgramState = ProgramState::new();
        let mut prompt = Some("> ".to_string());

        println!("Welcome to Doppler. Type \"help\" for help");

        while let Ok(i) = handle(get_input(prompt), &mut dinfo, &mut c, player) {
            match i {
                CommandOutcome::Carryon => (),
                CommandOutcome::Exit => break,
            }
            prompt = c.selected_id.map(|id| format!("{}> ", id));
        }
        Ok(())
    }

    pub fn input(buf: &mut String, prompt: &str) -> std::io::Result<usize> {
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
        pub selected_id: Option<u32>,
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

    pub fn get_input(prompt: Option<String>) -> String {
        let mut user_input = String::new();
        let _ = input(&mut user_input, &prompt.unwrap_or("> ".to_string()));
        user_input
    }

    fn handle(
        user_input: String,
        dinfo: &mut DopplerInfo,
        c: &mut ProgramState,
        player: &rodio::Player,
    ) -> std::io::Result<CommandOutcome> {
        match user_input.as_str().trim() {
            "a" | "add" => song::add(dinfo),
            "s" | "select" => song::select(dinfo, c),
            "p" | "play" => song::play(dinfo, c, player),
            "r" | "remove" => song::remove(dinfo, c),
            "c" | "update" => song::update(dinfo, c),
            "l" | "list" => song::list(dinfo),
            "w" | "write" => write_to_files(dinfo),
            "h" | "?" | "help" => song::help(),
            "e" | "exit" => {
                println!("Exiting...");
                return Ok(CommandOutcome::Exit);
            }
            "ss" | "search" => song::search(dinfo),
            "x" => {
                if player.is_paused() {
                    player.play();
                } else {
                    player.pause();
                }
            }
            "status" => match *dinfo.currently_playing.lock().unwrap() {
                Some(id) => match dinfo.get_song_by_id(id) {
                    Some(song) => {
                        let duration_str =
                            crate::util::time_util::seconds_to_base60_string(song.duration);
                        let playback_time = player.get_pos().as_secs() as u32;
                        let playback_time =
                            crate::util::time_util::seconds_to_base60_string(playback_time);
                        println!("Now playing ({playback_time}/{duration_str}): {}", song);
                    }
                    None => println!("Failed to get current song"),
                },
                None => println!("No song currently playing"),
            },
            "playlist" | "pl" => match handle_playlist(dinfo, player) {
                Ok(()) => println!("Exiting playlist mode with success"),
                Err(err) => {
                    println!("Playlist interaction failed ({})", err);
                }
            },
            _ => println!("Invalid input. try help or exit."),
        }
        Ok(CommandOutcome::Carryon)
    }

    fn handle_playlist(dinfo: &mut DopplerInfo, player: &rodio::Player) -> std::io::Result<()> {
        let mut c = ProgramState::new();
        loop {
            let mut buf = String::new();
            let selected = if let Some(id) = c.selected_id {
                format!("{}", id)
            } else {
                "".to_string()
            };
            let _ = input(&mut buf, format!("{selected}$ ").as_str());
            match buf.as_str().trim() {
                "p" | "play" => playlist::play(dinfo, &c, player),
                "c" | "shuffle" => playlist::shuffle_play(dinfo, &c, player),
                "a" | "add" => playlist::add(dinfo),
                "u" | "update" => playlist::update(dinfo, &c),
                "s" | "select" => playlist::select(dinfo, &mut c),
                "r" | "remove" => playlist::remove(dinfo, &c),
                "l" | "list" => playlist::list(dinfo),
                "h" | "?" | "help" => playlist::help(),
                "as" | "addsongs" => playlist::add_songs(dinfo, &c),
                _ => break,
            }
        }
        Ok(())
    }

    fn write_to_files(dinfo: &mut DopplerInfo) {
        match dinfo.update_songs_file() {
            Ok(c) => println!("Wrote {} songs to file", c),
            Err(err) => println!("Failed to write songs to file ({})", err),
        };
        match dinfo.update_playlist_file() {
            Ok(c) => println!("Wrote {} playlists to file", c),
            Err(err) => println!("Failed to write playlists to file ({})", err),
        };
    }
}
