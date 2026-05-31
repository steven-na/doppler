use std::io;

use crate::tui::app::App;

pub mod dlib;
pub mod tui;
pub mod util;

fn main() -> io::Result<()> {
    let mut handle =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open audio sink");
    handle.log_on_drop(false);
    let player = rodio::Player::connect_new(handle.mixer());
    player.set_volume(0.1);

    tui_main(player)
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
