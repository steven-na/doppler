use std::{env, io};

use crate::tui::app::App;

pub mod cli;
pub mod dlib;
pub mod tui;
pub mod util;

fn main() -> io::Result<()> {
    let mut handle =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open audio sink");
    handle.log_on_drop(false);
    let player = rodio::Player::connect_new(handle.mixer());
    player.set_volume(0.1);

    match env::args()
        .collect::<Vec<String>>()
        .get(1)
        .expect("Must specity tui or cli")
        .as_str()
        .trim()
    {
        "tui" => tui_main(player),
        "cli" => cli_main(&player),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Must specify tui or cli",
        )),
    }
}

fn cli_main(player: &rodio::Player) -> io::Result<()> {
    cli::parser::command_parser::main_loop(player)
}

fn tui_main(player: rodio::Player) -> io::Result<()> {
    let mut term = ratatui::init();
    let app = App::new(player);

    let o = match app {
        Ok(mut app) => app.main_loop(&mut term),
        Err(err) => Err(err),
    };

    ratatui::restore();

    o
}
