use std::{io, process::exit};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    style::Color,
    widgets::{Row, Table, TableState},
};

use crate::dlib::doppler_info::DopplerInfo;

pub struct App {
    should_exit: bool,
    dinfo: DopplerInfo,
    song_table_state: TableState,
}

impl App {
    pub fn new() -> io::Result<Self> {
        let dinfo = DopplerInfo::new()?;
        let song_table_state = TableState::default().with_selected(0);
        Ok(App {
            should_exit: false,
            dinfo,
            song_table_state,
        })
    }

    pub fn main_loop(&mut self, term: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        while !self.should_exit {
            // Draw
            let _ = term.draw(|frame| {
                self.draw(frame);
            });

            // Input
            if let crossterm::event::Event::Key(k) = crossterm::event::read()? {
                self.handle_input(k)
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, k: KeyEvent) {
        if k.is_press() {
            match k.code {
                KeyCode::Char('a') => exit(0),
                KeyCode::Up => self.song_table_state.select_previous(),
                KeyCode::Down => self.song_table_state.select_next(),
                KeyCode::Char('q') => self.should_exit = true,
                _ => (),
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]);
        let [title_area, content_area] = layout.areas(area);

        Line::from("Doppler").bold().render(title_area, buf);

        let rows: Vec<_> = self
            .dinfo
            .songs
            .iter()
            .map(|s| Row::new([s.name.clone(), s.artist.clone(), s.album.clone()]))
            .collect();

        let widths = [
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(50),
        ];
        let table = Table::new(rows, widths)
            .style(Color::White)
            .row_highlight_style(Style::new().on_black().bold())
            .highlight_symbol("> ");

        let mut state = self.song_table_state;

        ratatui::widgets::StatefulWidget::render(table, content_area, buf, &mut state);
    }
}
