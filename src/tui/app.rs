use std::{cell::RefCell, io};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    style::Color,
    widgets::{Block, Row, Table, TableState},
};

const TABLE_SELECTED_STYLE: Style = Style::new().underlined().bold();
const TABLE_UNSELECTED_STYLE: Style = Style::new();

use crate::{
    dlib::{doppler_info::DopplerInfo, playlist::PlaylistInfo, song::SongInfo},
    util::{print_util::truncate_string_and_add_suffix, time_util::seconds_to_base60_string},
};

enum CurrentMenu {
    Songs,
    Playlists,
    Queue,
}

enum ScrollDirection {
    Up,
    Down,
}

pub struct App {
    should_exit: bool,
    queue_open: bool,
    current_menu: CurrentMenu,
    dinfo: DopplerInfo,
    player: rodio::Player,
    song_table_state: TableData,
    playlist_table_state: TableData,
    queue_table_state: TableData,
}

#[derive(Debug, Clone)]
struct TableData {
    pub state: RefCell<TableState>,
    pub rows: Vec<(u32, Row<'static>)>,
}

const MAX_ALLOWED_SONG_LEN: usize = 50;

impl TableData {
    pub fn rebuild_songs(&mut self, s: &[SongInfo]) {
        self.rows = s
            .iter()
            .map(|p| {
                (
                    p.id.unwrap_or(0),
                    Row::new([
                        seconds_to_base60_string(p.duration),
                        truncate_string_and_add_suffix(p.name.as_str(), MAX_ALLOWED_SONG_LEN, None),
                        p.album.clone(),
                        p.artist.clone(),
                    ]),
                )
            })
            .collect();
    }

    pub fn rebuild_playlists(&mut self, p: &[PlaylistInfo]) {
        self.rows = p
            .iter()
            .map(|p| {
                (
                    p.id.unwrap_or(0),
                    Row::new([p.name.clone(), format!("{} Songs", p.songs.len())]),
                )
            })
            .collect();
    }

    pub fn selected_id(&self) -> Option<u32> {
        self.state
            .borrow()
            .selected()
            .and_then(|idx| self.rows.get(idx).map(|(id, _)| *id))
    }
}

impl App {
    pub fn new(player: rodio::Player) -> io::Result<Self> {
        let dinfo = DopplerInfo::new()?;
        let table_state = TableState::default().with_selected(0);
        Ok(App {
            should_exit: false,
            queue_open: false,
            current_menu: CurrentMenu::Songs,
            dinfo,
            player,
            song_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
            playlist_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
            queue_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
        })
    }

    pub fn main_loop(&mut self, term: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        self.song_table_state.rebuild_songs(&self.dinfo.songs);
        self.playlist_table_state
            .rebuild_playlists(&self.dinfo.playlists);

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
                KeyCode::Char(' ') if matches!(self.current_menu, CurrentMenu::Songs) => {
                    self.play_song();
                }
                KeyCode::Char(' ') if matches!(self.current_menu, CurrentMenu::Playlists) => {
                    self.play_playlist();
                }
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_exit = true;
                }
                KeyCode::Tab => self.cycle_menu(),
                KeyCode::Char('q') => self.toggle_queue(),
                KeyCode::Up => self.scroll_table(ScrollDirection::Up),
                KeyCode::Down => self.scroll_table(ScrollDirection::Down),
                _ => (),
            }
        }
    }

    fn play_song(&mut self) {
        if let Some(id) = self.song_table_state.selected_id() {
            let _ = self.dinfo.play_song(id, &self.player);
        }
    }

    fn play_playlist(&mut self) {
        if let Some(id) = self.playlist_table_state.selected_id() {
            let _ = self.dinfo.play_playlist(id, &self.player);
        }
    }

    fn toggle_queue(&mut self) {
        if self.queue_open {
            self.queue_open = false;
            if matches!(self.current_menu, CurrentMenu::Queue) {
                self.current_menu = CurrentMenu::Songs;
            }
        } else {
            self.queue_open = true;
            self.current_menu = CurrentMenu::Queue;
        }
    }

    fn scroll_table(&mut self, sd: ScrollDirection) {
        let mut scroll_target = match self.current_menu {
            CurrentMenu::Songs => &mut self.song_table_state,
            CurrentMenu::Playlists => &mut self.playlist_table_state,
            CurrentMenu::Queue => &mut self.queue_table_state,
        }
        .state
        .borrow_mut();

        match sd {
            ScrollDirection::Up => scroll_target.select_previous(),
            ScrollDirection::Down => scroll_target.select_next(),
        }
    }

    fn cycle_menu(&mut self) {
        self.current_menu = match self.current_menu {
            CurrentMenu::Songs if self.queue_open => CurrentMenu::Queue,
            CurrentMenu::Songs => CurrentMenu::Playlists,
            CurrentMenu::Playlists => CurrentMenu::Songs,
            CurrentMenu::Queue => CurrentMenu::Playlists,
        };
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn draw_table(
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        selected: bool,
        table_data: &TableData,
        widths: &[Constraint],
    ) {
        let mut state = table_data.state.borrow_mut();
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(Line::from(title).style(if selected {
                TABLE_SELECTED_STYLE
            } else {
                TABLE_UNSELECTED_STYLE
            }));
        let block_inner = block.inner(area);
        block.render(area, buf);

        let rows: Vec<_> = table_data.rows.iter().map(|(_, row)| row.clone()).collect();

        let table = Table::new(rows, widths)
            .style(Color::White)
            .row_highlight_style(Style::new().on_black().bold())
            .highlight_symbol("> ");

        ratatui::widgets::StatefulWidget::render(table, block_inner, buf, &mut state);
    }

    fn draw_songs(&self, area: Rect, buf: &mut Buffer) {
        let selected = matches!(self.current_menu, CurrentMenu::Songs);
        let widths = [
            Constraint::Length(6),
            Constraint::Length(MAX_ALLOWED_SONG_LEN as u16),
            Constraint::Length(25),
            Constraint::Fill(1),
        ];

        App::draw_table(
            area,
            buf,
            "Songs",
            selected,
            &self.song_table_state,
            &widths,
        );
    }

    fn draw_playlists(&self, area: Rect, buf: &mut Buffer) {
        let selected = matches!(self.current_menu, CurrentMenu::Playlists);
        let widths = [Constraint::Fill(1), Constraint::Min(10)];

        App::draw_table(
            area,
            buf,
            "Playlists",
            selected,
            &self.playlist_table_state,
            &widths,
        );
    }

    fn draw_queue(&self, area: Rect, buf: &mut Buffer) {
        let selected = matches!(self.current_menu, CurrentMenu::Queue);
        let widths = [
            Constraint::Length(0),
            Constraint::Fill(1),
            Constraint::Length(0),
            Constraint::Length(0),
        ];

        App::draw_table(
            area,
            buf,
            "Queue",
            selected,
            &self.queue_table_state,
            &widths,
        );
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let app_layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]);
        let content_layout = Layout::horizontal([
            Constraint::Percentage(15),
            Constraint::Fill(1),
            Constraint::Percentage(if self.queue_open { 25 } else { 0 }),
        ]);
        let [content_area, controls_area] = app_layout.areas(area);
        let [playlist_area, song_area, queue_area] = content_layout.areas(content_area);

        Line::from("↑/↓ navigate | [tab] change pane | q queue | <ctrl-c> quit")
            .dim()
            .render(controls_area, buf);

        self.draw_songs(song_area, buf);

        self.draw_playlists(playlist_area, buf);

        if self.queue_open {
            self.draw_queue(queue_area, buf);
        }
    }
}
