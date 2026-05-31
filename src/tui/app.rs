use std::{cell::RefCell, io, sync::mpsc, thread, time::Duration};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Paragraph, TableState},
};

pub const TABLE_SELECTED_STYLE: Style = Style::new().underlined().bold();
pub const TABLE_UNSELECTED_STYLE: Style = Style::new();
pub const MAX_ALLOWED_SONG_LEN: usize = 50;

use super::tables::{TableData, draw_table};
use crate::{
    dlib::doppler_info::DopplerInfo,
    util::{
        print_util::{seek_bar_string, truncate_string_and_add_suffix},
        time_util::seconds_to_base60_string,
    },
};

enum CurrentMenu {
    Songs,
    Playlists,
    Queue,
}

pub enum AppEvent {
    Input(crossterm::event::KeyEvent),
    Song,
    Update,
}

enum ScrollDirection {
    Up,
    Down,
}

pub struct App {
    event_receiver: mpsc::Receiver<AppEvent>,
    should_exit: bool,
    queue_open: bool,
    current_menu: CurrentMenu,
    dinfo: DopplerInfo,
    song_table_state: TableData,
    playlist_table_state: TableData,
    queue_table_state: TableData,
}

fn send_input_to_app(tx: mpsc::Sender<AppEvent>) {
    loop {
        if let crossterm::event::Event::Key(k) = crossterm::event::read().unwrap() {
            tx.send(AppEvent::Input(k)).unwrap()
        }
    }
}

impl App {
    pub fn new(player: rodio::Player) -> io::Result<Self> {
        let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

        let tx_for_input = event_tx.clone();
        thread::spawn(move || {
            send_input_to_app(tx_for_input);
        });
        let tx_for_annoy = event_tx.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs_f32(0.5));
                tx_for_annoy.send(AppEvent::Update).unwrap();
            }
        });

        let dinfo = DopplerInfo::new(player, event_tx)?;
        let table_state = TableState::default().with_selected(0);
        Ok(App {
            event_receiver: event_rx,
            should_exit: false,
            queue_open: false,
            current_menu: CurrentMenu::Songs,
            dinfo,
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
        self.song_table_state
            .rebuild(self.dinfo.songs.read().unwrap().iter());
        self.playlist_table_state
            .rebuild(self.dinfo.playlists.iter());

        while !self.should_exit {
            if self
                .dinfo
                .queue_dirty
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                self.queue_table_state
                    .rebuild(self.dinfo.queue_entries().iter());
            }

            // Draw
            let _ = term.draw(|frame| {
                self.draw(frame);
            });

            match self.event_receiver.recv().unwrap() {
                AppEvent::Input(k) => self.handle_input(k),
                AppEvent::Song => (),
                AppEvent::Update => (),
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
                KeyCode::Char('e') if matches!(self.current_menu, CurrentMenu::Songs) => {
                    self.enqueue_song();
                }
                KeyCode::Char(' ') if matches!(self.current_menu, CurrentMenu::Playlists) => {
                    self.play_playlist();
                }
                KeyCode::Char('r') if matches!(self.current_menu, CurrentMenu::Playlists) => {
                    self.shuffle_play_playlist();
                }
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_exit = true;
                }
                KeyCode::Tab => self.cycle_menu(),
                KeyCode::Char('k') => self.dinfo.skip_song(),
                KeyCode::Char('q') => self.toggle_queue(),
                KeyCode::Up => self.scroll_table(ScrollDirection::Up),
                KeyCode::Down => self.scroll_table(ScrollDirection::Down),
                _ => (),
            }
        }
    }

    fn play_song(&mut self) {
        if let Some(id) = self.song_table_state.selected_id() {
            let _ = self.dinfo.play_song(id);
        }
    }

    fn enqueue_song(&mut self) {
        if let Some(id) = self.song_table_state.selected_id() {
            let _ = self.dinfo.enqueue_song(id);
        }
    }

    fn play_playlist(&mut self) {
        if let Some(id) = self.playlist_table_state.selected_id() {
            let _ = self.dinfo.play_playlist(id, false);
        }
    }

    fn shuffle_play_playlist(&mut self) {
        if let Some(id) = self.playlist_table_state.selected_id() {
            let _ = self.dinfo.play_playlist(id, true);
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

    fn draw_songs(&self, area: Rect, buf: &mut Buffer) {
        let selected = matches!(self.current_menu, CurrentMenu::Songs);
        let widths = [
            Constraint::Length(6),
            Constraint::Length(MAX_ALLOWED_SONG_LEN as u16),
            Constraint::Length(25),
            Constraint::Fill(1),
        ];

        draw_table(
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

        draw_table(
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
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Fill(1),
        ];

        let q_lock = self.dinfo.enqueued_playlist.lock().unwrap();
        let qd_songs: Vec<u32> = q_lock
            .as_ref()
            .map(|q| q.queue.iter().map(|&(id, _)| id).collect())
            .unwrap_or_default();
        let duration = seconds_to_base60_string(qd_songs.iter().fold(0, |acc, &id| {
            acc + self
                .dinfo
                .get_song_by_id(id)
                .map(|s| s.duration)
                .unwrap_or(0)
        }));

        draw_table(
            area,
            buf,
            format!("({}, {}) Queue", qd_songs.len(), duration).as_str(),
            selected,
            &self.queue_table_state,
            &widths,
        );
    }

    fn draw_now_playing(&self, area: Rect, buf: &mut Buffer) {
        let np_block = ratatui::widgets::Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(Line::from("Now playing"));
        let block_inner = np_block.inner(area);
        np_block.render(area, buf);
        let block_width = block_inner.width.into();
        let current_song_id = *self.dinfo.currently_playing.lock().unwrap();
        let current_song = if let Some(id) = current_song_id {
            self.dinfo.get_song_by_id(id)
        } else {
            None
        };

        let song_string;
        let album_string;
        let artist_string;
        let duration_string;
        let seekbar_string;
        if let Some(song) = current_song {
            song_string = truncate_string_and_add_suffix(&song.name, block_width, None);
            album_string = truncate_string_and_add_suffix(
                format!("on {}", song.album).as_str(),
                block_width,
                None,
            );
            artist_string = truncate_string_and_add_suffix(&song.artist, block_width, None);
            duration_string = format!(
                "{}/{}",
                seconds_to_base60_string(self.dinfo.get_song_progress()),
                seconds_to_base60_string(song.duration)
            );

            seekbar_string = seek_bar_string(
                self.dinfo.player.lock().unwrap().get_pos().as_secs() as u32,
                song.duration,
                block_width as u32 - 4,
            )
            .unwrap_or("".to_string());
        } else {
            song_string = "Nothing".to_string();
            album_string = "on Nothing".to_string();
            artist_string = "Nobody".to_string();
            duration_string = "0:00/0:00".to_string();
            seekbar_string =
                seek_bar_string(5, 10, block_width as u32 - 4).unwrap_or("".to_string());
        }
        Paragraph::new(vec![
            Line::from(song_string),
            Line::from(album_string),
            Line::from(artist_string),
            Line::from(duration_string),
            Line::from(seekbar_string),
        ])
        .centered()
        .render(block_inner, buf);
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let app_layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]);
        let content_layout = Layout::horizontal([
            Constraint::Percentage(20),
            Constraint::Fill(1),
            Constraint::Percentage(if self.queue_open { 25 } else { 0 }),
        ]);
        let left_layout = Layout::vertical([Constraint::Length(7), Constraint::Fill(1)]);
        let [content_area, controls_area] = app_layout.areas(area);
        let [left_area, song_area, queue_area] = content_layout.areas(content_area);
        let [now_playing_area, playlist_area] = left_layout.areas(left_area);

        Line::from("↑/↓ navigate | [tab] change pane | q queue | <ctrl-c> quit")
            .dim()
            .render(controls_area, buf);

        self.draw_now_playing(now_playing_area, buf);

        self.draw_songs(song_area, buf);

        self.draw_playlists(playlist_area, buf);

        if self.queue_open {
            self.draw_queue(queue_area, buf);
        }
    }
}
