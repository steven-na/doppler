use crate::cli::parser::command_parser::{self, CommandOutcome, ProgramState, get_input};
use crate::dlib::doppler_info::DopplerInfo;

pub mod cli;
pub mod dlib;
pub mod util;

fn main() -> std::io::Result<()> {
    let mut songs = DopplerInfo::new()?;

    let mut prog_state: ProgramState = ProgramState::new();
    while let Ok(i) = command_parser::handle(get_input(), &mut songs, &mut prog_state) {
        match i {
            CommandOutcome::Carryon => (),
            CommandOutcome::Exit => break,
        }
    }

    Ok(())
}
