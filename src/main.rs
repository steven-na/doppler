pub mod cli;
pub mod dlib;
pub mod util;

fn main() -> std::io::Result<()> {
    let mut handle =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open audio sink");
    handle.log_on_drop(false);
    let player = rodio::Player::connect_new(handle.mixer());
    player.set_volume(0.2);

    cli::parser::command_parser::main_loop(&player)
}
