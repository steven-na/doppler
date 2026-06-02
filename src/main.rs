use std::{env, fs, io};

use crate::tui::app::App;

pub mod dlib;
pub mod tui;
pub mod util;

fn main() -> io::Result<()> {
    let base_dir: Option<String> = env::args().collect::<Vec<String>>().get(1).cloned();
    if let Some(ref path) = base_dir {
        match fs::metadata(path) {
            Ok(md) => {
                if md.is_file() {
                    return Err(io::Error::other("Input base dir was file not directory"));
                }
            }
            Err(err) => return Err(err),
        }
    }

    let mut handle =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open audio sink");
    handle.log_on_drop(false);
    let player = rodio::Player::connect_new(handle.mixer());
    player.set_volume(0.1);

    tui_main(base_dir, player)
}

fn tui_main(base_directory: Option<String>, player: rodio::Player) -> io::Result<()> {
    let mut term = ratatui::init();
    let app = App::new(base_directory, player);

    let o = match app {
        Ok(mut app) => app.main_loop(&mut term),
        Err(err) => Err(err),
    };

    ratatui::restore();

    o
}
