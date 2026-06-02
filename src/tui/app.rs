use std::{cell::RefCell, cmp::min, io, sync::mpsc, thread, time::Duration};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Paragraph, TableState, Wrap},
};

pub const TABLE_SELECTED_STYLE: Style = Style::new().underlined().bold();
pub const TABLE_UNSELECTED_STYLE: Style = Style::new();
pub const MAX_ALLOWED_SONG_LEN: usize = 50;

use super::tables::{TableData, draw_table};
use crate::{
    dlib::{doppler_info::DopplerInfo, playlist::PlaylistInfo, song::SongInfo},
    util::{
        print_util::{seek_bar_string, truncate_string_and_add_suffix},
        time_util::seconds_to_base60_string,
    },
};

enum CurrentMenu {
    Songs,
    Playlists,
    PlaylistEditor,
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

enum TextInputDestination {
    Search,
    SongName,
    SongAlbum,
    SongArtist,
    SongDuration,
    PlaylistNameUpdate,
    PlaylistNameAdd,
}

struct AppTextInputs {
    search_query: String,
    song_name: String,
    song_album: String,
    song_artist: String,
    song_duration: (String, Option<u32>),
    playlist_name: String,
}

pub struct App {
    event_receiver: mpsc::Receiver<AppEvent>,
    should_exit: bool,

    queue_open: bool,
    lyrics_open: bool,
    editing_playlist: bool,
    current_menu: CurrentMenu,

    text_input_queue: Vec<TextInputDestination>,
    text_inputs: AppTextInputs,
    messages: Vec<(String, u32)>,

    dinfo: DopplerInfo,

    song_table_state: TableData,
    playlist_table_state: TableData,
    playlist_editor_table_state: TableData,
    queue_table_state: TableData,
    need_rebuild: bool,
}

fn send_input_to_app(tx: mpsc::Sender<AppEvent>) {
    loop {
        if let crossterm::event::Event::Key(k) = crossterm::event::read().unwrap() {
            tx.send(AppEvent::Input(k)).unwrap()
        }
    }
}

impl App {
    pub fn new(base_directory: Option<String>, player: rodio::Player) -> io::Result<Self> {
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

        let dinfo = DopplerInfo::new(
            base_directory.unwrap_or("./data".to_string()),
            player,
            event_tx,
        )?;
        let table_state = TableState::default().with_selected(0);
        Ok(App {
            event_receiver: event_rx,
            should_exit: false,
            queue_open: false,
            lyrics_open: false,
            editing_playlist: false,
            current_menu: CurrentMenu::Songs,
            text_input_queue: Vec::new(),
            text_inputs: AppTextInputs {
                search_query: String::new(),
                song_name: String::new(),
                song_album: String::new(),
                song_artist: String::new(),
                song_duration: (String::new(), None),
                playlist_name: String::new(),
            },
            messages: Vec::new(),
            dinfo,
            song_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
            playlist_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
            playlist_editor_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
            queue_table_state: TableData {
                state: RefCell::new(table_state),
                rows: Vec::new(),
            },
            need_rebuild: true,
        })
    }

    pub fn main_loop(&mut self, term: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        self.rebuild_songs();
        self.playlist_table_state
            .rebuild(self.dinfo.playlists.iter());

        while !self.should_exit {
            if self.need_rebuild {
                self.rebuild_songs();
                self.playlist_table_state
                    .rebuild(self.dinfo.playlists.iter());
                self.need_rebuild = false;
            }
            if self.editing_playlist {
                self.rebuild_playlist_editor();
            }

            self.messages.retain_mut(|(_, life)| {
                *life = life.saturating_sub(1);
                *life > 0
            });

            // Draw
            let _ = term.draw(|frame| {
                self.draw(frame);
            });

            // Handle channel data
            match self.event_receiver.recv().unwrap() {
                AppEvent::Input(k) => {
                    self.handle_input(k);
                }
                AppEvent::Song => {
                    self.queue_table_state
                        .rebuild(self.dinfo.queue_entries().iter());
                }
                AppEvent::Update => (),
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, k: KeyEvent) {
        if k.is_press() {
            if let Some(dest) = self.text_input_queue.first() {
                let inputs = &mut self.text_inputs;
                let dest_str = match dest {
                    TextInputDestination::Search => &mut inputs.search_query,
                    TextInputDestination::SongName => &mut inputs.song_name,
                    TextInputDestination::SongAlbum => &mut inputs.song_album,
                    TextInputDestination::SongArtist => &mut inputs.song_artist,
                    TextInputDestination::SongDuration => &mut inputs.song_duration.0,
                    TextInputDestination::PlaylistNameUpdate
                    | TextInputDestination::PlaylistNameAdd => &mut inputs.playlist_name,
                };
                if let Some(ch) = k.code.as_char() {
                    if matches!(dest, TextInputDestination::Search) {
                        self.song_table_state.state.borrow_mut().select_first();
                        self.need_rebuild = true;
                    }
                    dest_str.push(ch);
                }
                match k.code {
                    KeyCode::Esc => {
                        self.text_input_queue.clear();
                    }
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        dest_str.clear();
                    }
                    KeyCode::Enter => {
                        match dest {
                            TextInputDestination::SongDuration => {
                                match inputs.song_duration.0.parse::<u32>() {
                                    Ok(i) => {
                                        self.text_inputs.song_duration.1 = Some(i);
                                        self.finish_edit_song();
                                    }
                                    Err(err) => {
                                        self.emit_message(format!("Err handling input ({})", err));
                                        return;
                                    }
                                }
                            }
                            TextInputDestination::PlaylistNameUpdate => {
                                self.finish_edit_playlist();
                            }
                            TextInputDestination::PlaylistNameAdd => {
                                self.finish_add_playlist();
                            }
                            _ => (),
                        }
                        let _ = self.text_input_queue.remove(0);
                    }
                    KeyCode::Backspace => {
                        dest_str.pop();
                        self.need_rebuild = true;
                    }
                    _ => (),
                }
            } else {
                match self.current_menu {
                    // Song only
                    CurrentMenu::Songs => match k.code {
                        KeyCode::Char(' ') if self.editing_playlist => self.add_playlist_song(),
                        KeyCode::Char('i') if self.editing_playlist => self.insert_playlist_song(),
                        KeyCode::Char(' ') => {
                            self.play_song();
                            return;
                        }
                        KeyCode::Char('q') => {
                            self.enqueue_song();
                            return;
                        }
                        KeyCode::Char('/') if self.text_input_queue.is_empty() => {
                            self.text_input_queue.push(TextInputDestination::Search);
                        }
                        KeyCode::Char('u')
                            if self.text_input_queue.is_empty() && k.modifiers.is_empty() =>
                        {
                            self.start_edit_song();
                            return;
                        }
                        KeyCode::Esc => {
                            self.text_inputs.search_query.clear();
                            self.need_rebuild = true;
                        }
                        _ => (),
                    },
                    // Playlist only
                    CurrentMenu::Playlists => match k.code {
                        KeyCode::Char(' ') => {
                            self.play_playlist();
                            return;
                        }
                        KeyCode::Char('u')
                            if self.text_input_queue.is_empty() && k.modifiers.is_empty() =>
                        {
                            self.start_edit_playlist();
                            return;
                        }
                        KeyCode::Char('r') => {
                            self.shuffle_play_playlist();
                            return;
                        }
                        KeyCode::Char('a') => {
                            self.start_add_playlist();
                            return;
                        }
                        _ => (),
                    },
                    CurrentMenu::PlaylistEditor => match k.code {
                        KeyCode::Char('r') if self.editing_playlist => self.remove_playlist_song(),
                        _ => (),
                    },
                    _ => (),
                }
                // General
                match k.code {
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.should_exit = true
                    }
                    KeyCode::Char('w') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.write_to_files()
                    }
                    KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        match self.dinfo.reload_from_files() {
                            Ok(()) => self.emit_message("Reloaded from files".to_string()),
                            Err(err) => self.emit_message(format!("Error reloading ({})", err)),
                        }
                        self.need_rebuild = true;
                    }
                    KeyCode::Char('V') => self.dinfo.change_volume(0.05),
                    KeyCode::Char('v') => self.dinfo.change_volume(-0.05),
                    KeyCode::Tab => self.cycle_menu(),
                    KeyCode::Char('k') => self.dinfo.skip_song(),
                    KeyCode::Char('p') => self.dinfo.toggle_pause(),
                    KeyCode::Char('Q') => self.toggle_queue(),
                    KeyCode::Char('L') => self.lyrics_open = !self.lyrics_open,
                    KeyCode::Char('f') => self.toggle_playlist_edit(),
                    KeyCode::Up => self.scroll_table(ScrollDirection::Up),
                    KeyCode::Down => self.scroll_table(ScrollDirection::Down),
                    _ => (),
                }
            }
        }
    }

    fn start_edit_song(&mut self) {
        if let Some(song_id) = self.song_table_state.selected_id()
            && let Some(song) = self.dinfo.get_song_by_id(song_id)
        {
            self.text_inputs.song_name = song.name.clone();
            self.text_inputs.song_album = song.album.clone();
            self.text_inputs.song_artist = song.artist.clone();
            self.text_inputs.song_duration.0 = song.duration.to_string();
            self.text_input_queue.clear();
            self.text_input_queue.push(TextInputDestination::SongName);
            self.text_input_queue.push(TextInputDestination::SongAlbum);
            self.text_input_queue.push(TextInputDestination::SongArtist);
            self.text_input_queue
                .push(TextInputDestination::SongDuration);
        } else {
            self.emit_message("Failed to edit song".to_string());
        }
    }

    fn start_edit_playlist(&mut self) {
        if let Some(playlist_id) = self.playlist_table_state.selected_id()
            && let Some(playlist) = self.dinfo.get_playlist_by_id(playlist_id)
        {
            self.text_inputs.playlist_name = playlist.name.clone();
            self.text_input_queue.clear();
            self.text_input_queue
                .push(TextInputDestination::PlaylistNameUpdate);
        } else {
            self.emit_message("Failed to edit playlist".to_string());
        }
    }

    fn start_add_playlist(&mut self) {
        self.text_inputs.playlist_name.clear();
        self.text_input_queue.clear();
        self.text_input_queue
            .push(TextInputDestination::PlaylistNameAdd);
    }

    fn finish_edit_song(&mut self) {
        if let Some(song_id) = self.song_table_state.selected_id()
            && let Some(song) = self.dinfo.get_song_by_id(song_id)
        {
            let dur = if let Some(d) = self.text_inputs.song_duration.1 {
                d
            } else {
                self.emit_message("Update failed (bad duration)".to_string());
                return;
            };
            match self.dinfo.update_song(SongInfo {
                name: self.text_inputs.song_name.clone(),
                artist: self.text_inputs.song_artist.clone(),
                album: self.text_inputs.song_album.clone(),
                duration: dur,
                file_entry_up_to_date: false,
                ..song
            }) {
                Ok(()) => self.emit_message("Successfully updated song".to_string()),
                Err(err) => self.emit_message(format!("Update failed ({})", err)),
            }
            self.need_rebuild = true;
        }
    }

    fn finish_edit_playlist(&mut self) {
        if let Some(playlist_id) = self.playlist_table_state.selected_id()
            && let Some(playlist) = self.dinfo.get_playlist_by_id(playlist_id)
        {
            match self.dinfo.update_playlist(PlaylistInfo {
                name: self.text_inputs.playlist_name.clone(),
                ..playlist.clone()
            }) {
                Ok(()) => self.emit_message("Successfully updated playlist".to_string()),
                Err(err) => self.emit_message(format!("Update failed ({})", err)),
            }
            self.need_rebuild = true;
        }
    }

    fn finish_add_playlist(&mut self) {
        let name = self.text_inputs.playlist_name.clone();
        if name.is_empty() {
            self.emit_message("Can't add playlist with no name".to_string());
        }
        match self.dinfo.add_playlist(PlaylistInfo {
            name,
            ..Default::default()
        }) {
            Ok(()) => self.emit_message("Added playlist".to_string()),
            Err(err) => self.emit_message(format!("Failed to add playlist ({})", err)),
        }
        self.need_rebuild = true;
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

    fn rebuild_playlist_editor(&mut self) {
        if let Some(playlist_id) = self.playlist_table_state.selected_id()
            && let Some(p) = self.dinfo.get_playlist_by_id(playlist_id)
        {
            let ids: Vec<SongInfo> = p
                .songs
                .iter()
                .filter_map(|&id| self.dinfo.get_song_by_id(id))
                .collect();
            self.playlist_editor_table_state.rebuild(ids.iter());
        }
    }

    fn rebuild_songs(&mut self) {
        let songs = if !self.text_inputs.search_query.is_empty() {
            self.dinfo
                .filter_songs(self.text_inputs.search_query.clone())
                .clone()
        } else {
            self.dinfo.get_linked_songs()
        };
        self.song_table_state.rebuild(songs.iter().rev());
    }

    fn add_playlist_song(&mut self) {
        if let Some(playlist_id) = self.playlist_table_state.selected_id()
            && let Some(p) = self.dinfo.get_playlist_by_id_mut(playlist_id)
            && let Some(song_id) = self.song_table_state.selected_id()
        {
            p.force_add_song(song_id);
            self.need_rebuild = true;
        }
    }

    fn insert_playlist_song(&mut self) {
        if let Some(playlist_id) = self.playlist_table_state.selected_id()
            && let Some(idx) = self.playlist_editor_table_state.selected_index()
            && let Some(p) = self.dinfo.get_playlist_by_id_mut(playlist_id)
            && let Some(song_id) = self.song_table_state.selected_id()
        {
            let _ = p.insert_song(idx, song_id);
            self.need_rebuild = true;
        }
    }

    fn remove_playlist_song(&mut self) {
        if let Some(playlist_id) = self.playlist_table_state.selected_id()
            && let Some(p) = self.dinfo.get_playlist_by_id_mut(playlist_id)
            && let Some(index) = self.playlist_editor_table_state.selected_index()
        {
            let _ = p.remove_song_by_index(index);
            self.need_rebuild = true;
        }
    }

    fn write_to_files(&mut self) {
        let song_outcome = self.dinfo.update_songs_file();
        self.emit_message(match song_outcome {
            Ok(n) => format!("Wrote {n} songs to file"),
            Err(_) => "Failed to write songs to file".to_string(),
        });
        let pl_outcome = self.dinfo.update_playlist_file();
        self.emit_message(match pl_outcome {
            Ok(n) => format!("Wrote {n} playlists to file"),
            Err(_) => "Failed to write playlists to file".to_string(),
        });
    }

    fn toggle_queue(&mut self) {
        if self.queue_open {
            self.queue_open = false;
            if matches!(self.current_menu, CurrentMenu::Queue) {
                self.current_menu = CurrentMenu::Songs;
            }
        } else {
            self.queue_open = true;
            self.editing_playlist = false;
        }
    }

    fn toggle_playlist_edit(&mut self) {
        if self.editing_playlist {
            self.editing_playlist = false;
            if matches!(self.current_menu, CurrentMenu::PlaylistEditor) {
                self.current_menu = CurrentMenu::Songs;
            }
        } else {
            self.rebuild_playlist_editor();
            self.playlist_editor_table_state
                .state
                .borrow_mut()
                .select_first();
            self.editing_playlist = true;
            self.queue_open = false;
        }
    }

    fn scroll_table(&mut self, sd: ScrollDirection) {
        let mut scroll_target = match self.current_menu {
            CurrentMenu::Songs => &mut self.song_table_state,
            CurrentMenu::PlaylistEditor => &mut self.playlist_editor_table_state,
            CurrentMenu::Queue => &mut self.queue_table_state,
            CurrentMenu::Playlists => &mut self.playlist_table_state,
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
            CurrentMenu::Songs if self.editing_playlist => CurrentMenu::PlaylistEditor,
            CurrentMenu::Songs => CurrentMenu::Playlists,
            CurrentMenu::Playlists => CurrentMenu::Songs,
            CurrentMenu::Queue | CurrentMenu::PlaylistEditor => CurrentMenu::Playlists,
        };
    }

    fn emit_message(&mut self, msg: String) {
        self.messages.push((msg, 100));
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

        let query = if self.text_inputs.search_query.is_empty() {
            "".to_string()
        } else {
            format!("<search:{}>", self.text_inputs.search_query)
        };

        let bottom_text = match self.text_input_queue.first() {
            Some(t) => match t {
                TextInputDestination::SongName => {
                    format!("Song name: {}", self.text_inputs.song_name)
                }
                TextInputDestination::SongAlbum => {
                    format!("Song album: {}", self.text_inputs.song_album)
                }
                TextInputDestination::SongArtist => {
                    format!("Song artist: {}", self.text_inputs.song_artist)
                }
                TextInputDestination::SongDuration => {
                    format!("Song duration: {}", self.text_inputs.song_duration.0)
                }
                _ => "".to_string(),
            },
            None => "".to_string(),
        };

        draw_table(
            area,
            buf,
            format!("Songs {}", query).as_str(),
            selected,
            &self.song_table_state,
            &widths,
            Some(bottom_text),
        );
    }

    fn draw_playlists(&self, area: Rect, buf: &mut Buffer) {
        let selected = matches!(self.current_menu, CurrentMenu::Playlists);
        let widths = [Constraint::Fill(1), Constraint::Min(10)];

        let bottom_text = if matches!(
            self.text_input_queue.first(),
            Some(TextInputDestination::PlaylistNameUpdate)
                | Some(TextInputDestination::PlaylistNameAdd)
        ) {
            format!("Playlist name: {}", self.text_inputs.playlist_name)
        } else {
            "".to_string()
        };

        draw_table(
            area,
            buf,
            "Playlists",
            selected,
            &self.playlist_table_state,
            &widths,
            Some(bottom_text),
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
            None,
        );
    }

    fn draw_playlist_editor(&self, area: Rect, buf: &mut Buffer) {
        let selected = matches!(self.current_menu, CurrentMenu::PlaylistEditor);
        let widths = [
            Constraint::Length(6),
            Constraint::Length(MAX_ALLOWED_SONG_LEN as u16),
            Constraint::Length(25),
            Constraint::Fill(1),
        ];

        let title = if let Some(id) = self.playlist_table_state.selected_id()
            && let Some(p) = self.dinfo.get_playlist_by_id(id)
        {
            format!(
                "Editing {}{}",
                if p.file_entry_up_to_date { "" } else { "*" },
                p.name
            )
        } else {
            "No playlist selected".to_string()
        };

        draw_table(
            area,
            buf,
            &title,
            selected,
            &self.playlist_editor_table_state,
            &widths,
            None,
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
        let mut seekbar_string = (if self.dinfo.is_paused() { "|> " } else { "|| " }).to_string();
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

            seekbar_string.push_str(
                seek_bar_string(
                    self.dinfo.player.lock().unwrap().get_pos().as_secs() as u32,
                    song.duration,
                    block_width as u32 - 6,
                )
                .unwrap_or("".to_string())
                .as_str(),
            );
        } else {
            song_string = "Nothing".to_string();
            album_string = "on Nothing".to_string();
            artist_string = "Nobody".to_string();
            duration_string = "0:00/0:00".to_string();
            seekbar_string.push_str(
                seek_bar_string(5, 10, block_width as u32 - 6)
                    .unwrap_or("".to_string())
                    .as_str(),
            );
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

    fn draw_lyrics(&self, area: Rect, buf: &mut Buffer) {
        let ly_block = ratatui::widgets::Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(Line::from("Lyrics"));
        let block_inner = ly_block.inner(area);
        ly_block.render(area, buf);
        let block_width = block_inner.width as usize;
        let block_height = block_inner.height as usize;

        let lyrics = {
            if let Some(current_song) = *self.dinfo.currently_playing.lock().unwrap() {
                self.dinfo.get_lyrics_from_song_id(current_song)
            } else {
                None
            }
        };

        const CURRENT_LYRIC_POS: usize = 1;
        let mut is_synced = false;
        let mut lyric_section_width = block_width;
        let lyric = if let Some(l) = lyrics {
            lyric_section_width = min(lyric_section_width, l.longest_lyric_in_song());
            if l.is_synced {
                is_synced = true;
                l.get_lyrics_around_time(
                    self.dinfo.get_song_progress(),
                    CURRENT_LYRIC_POS,
                    block_height,
                )
                .unwrap_or(vec!["Failed to get lyrics".to_string()])
            } else {
                l.lyrics.clone()
            }
        } else {
            vec!["No lyrics associated".to_string()]
        };

        let [_, lyric_area, _] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(lyric_section_width as u16),
            Constraint::Fill(1),
        ])
        .areas(block_inner);

        Paragraph::new(
            lyric
                .into_iter()
                .enumerate()
                .map(|(idx, s)| {
                    if !is_synced {
                        return Line::from(s);
                    }
                    if idx == CURRENT_LYRIC_POS {
                        Line::from(s).bold()
                    } else {
                        Line::from(s).dim()
                    }
                })
                .collect::<Vec<Line>>(),
        )
        .wrap(Wrap { trim: true })
        .render(lyric_area, buf);
    }

    fn draw_messages(&self, area: Rect, buf: &mut Buffer) {
        let msg_block = ratatui::widgets::Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(Line::from("Log"));
        let block_inner = msg_block.inner(area);
        msg_block.render(area, buf);
        let max_msg = block_inner.height;
        let words: Vec<Line> = self
            .messages
            .iter()
            .map(|(s, _)| Line::from(s.as_str()))
            .take(max_msg.into())
            .collect();
        Paragraph::new(words).render(block_inner, buf);
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
            Constraint::Fill(2),
            Constraint::Fill(if self.queue_open { 1 } else { 0 }),
            Constraint::Fill(if self.editing_playlist { 2 } else { 0 }),
        ]);
        let left_layout = Layout::vertical([
            Constraint::Length(7),
            Constraint::Fill(1),
            Constraint::Length(if self.messages.is_empty() { 0 } else { 10 }),
        ]);
        let song_and_lyric_layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(if self.lyrics_open { 8 } else { 0 }),
        ]);
        let [content_area, controls_area] = app_layout.areas(area);
        let [
            left_area,
            song_and_lyrics_area,
            queue_area,
            playlist_editor_area,
        ] = content_layout.areas(content_area);
        let [now_playing_area, playlist_area, message_area] = left_layout.areas(left_area);
        let [song_area, lyrics_area] = song_and_lyric_layout.areas(song_and_lyrics_area);

        let typing_text = if self.text_input_queue.is_empty() {
            " "
        } else {
            "(Typing, <Esc> to leave, <Enter> to submit, <Ctrl-c> to clear)"
        };
        let help_text = {
            let mut s = String::new();
            if self.text_input_queue.is_empty() {
                s = "<ctrl-c> quit | ↑/↓ nav | <V/v>ol up/down | [tab] next pane | <ctrl-w> write | <ctrl-u> reload | <p>ause | s<k>ip"
                    .to_string();
                if !self.editing_playlist {
                    let q_text = if self.queue_open { "close" } else { "open" };
                    s.push_str(format!(" | <shft-q> {q_text} queue").as_str());
                    match self.current_menu {
                        CurrentMenu::Songs => {
                            s.push_str(" | < / > search | [space] play | en<q>ueue | <u>pdate")
                        }
                        CurrentMenu::Playlists => {
                            s.push_str(" | [space] play | <r> shuffle play | <u>pdate")
                        }
                        _ => (),
                    }
                } else {
                    match self.current_menu {
                        CurrentMenu::Songs => {
                            s.push_str(" | [space] append | <i>nsert at selectd idx")
                        }
                        CurrentMenu::PlaylistEditor => s.push_str(" | <r>emove selected"),
                        _ => (),
                    }
                }
            }

            s
        };

        let [left, right] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(typing_text.len() as u16),
        ])
        .areas(controls_area);
        Line::from(help_text).dim().render(left, buf);
        Line::from(typing_text.to_string())
            .bold()
            .right_aligned()
            .render(right, buf);

        if !self.messages.is_empty() {
            self.draw_messages(message_area, buf);
        }

        self.draw_now_playing(now_playing_area, buf);

        self.draw_songs(song_area, buf);

        if self.lyrics_open {
            self.draw_lyrics(lyrics_area, buf);
        }

        self.draw_playlists(playlist_area, buf);

        if self.queue_open {
            self.draw_queue(queue_area, buf);
        } else if self.editing_playlist {
            self.draw_playlist_editor(playlist_editor_area, buf);
        }
    }
}
