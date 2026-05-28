pub mod cli;
pub mod dlib;
pub mod util;

fn main() -> std::io::Result<()> {
    let handle =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open audio handle");
    let player = rodio::Player::connect_new(&handle.mixer());
    player.set_volume(0.3);

    cli::parser::command_parser::main_loop(&player)
}
