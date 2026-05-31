use std::cell::RefCell;

use ratatui::prelude::*;

use crate::{
    dlib::{playlist::PlaylistInfo, song::SongInfo},
    tui::app::{TABLE_SELECTED_STYLE, TABLE_UNSELECTED_STYLE},
};

#[derive(Debug)]
pub struct TableData {
    pub state: RefCell<ratatui::widgets::TableState>,
    pub rows: Vec<(u32, ratatui::widgets::Row<'static>)>,
}

impl TableData {
    pub fn rebuild<T: super::tables::IntoTableRow + Copy>(
        &mut self,
        items: impl Iterator<Item = T>,
    ) {
        self.rows = items
            .filter_map(|item| Some((item.row_id()?, item.into_row())))
            .collect();
    }

    pub fn selected_id(&self) -> Option<u32> {
        self.state
            .borrow()
            .selected()
            .and_then(|idx| self.rows.get(idx).map(|(id, _)| *id))
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.state.borrow().selected()
    }
}

pub fn draw_table(
    area: Rect,
    buf: &mut Buffer,
    title: &str,
    selected: bool,
    table_data: &TableData,
    widths: &[Constraint],
) {
    let mut state = table_data.state.borrow_mut();
    let block = ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(Line::from(title).style(if selected {
            TABLE_SELECTED_STYLE
        } else {
            TABLE_UNSELECTED_STYLE
        }));
    let block_inner = block.inner(area);
    block.render(area, buf);

    let rows: Vec<_> = table_data.rows.iter().map(|(_, row)| row.clone()).collect();

    let table = ratatui::widgets::Table::new(rows, widths)
        .style(Color::White)
        .row_highlight_style(Style::new().on_black().bold())
        .highlight_symbol("> ");

    ratatui::widgets::StatefulWidget::render(table, block_inner, buf, &mut state);
}

pub trait IntoTableRow: Copy {
    fn row_id(&self) -> Option<u32>;
    fn into_row(self) -> ratatui::widgets::Row<'static>;
}

impl IntoTableRow for &SongInfo {
    fn row_id(&self) -> Option<u32> {
        self.id
    }

    fn into_row(self) -> ratatui::widgets::Row<'static> {
        ratatui::widgets::Row::new([
            crate::util::time_util::seconds_to_base60_string(self.duration),
            crate::util::print_util::truncate_string_and_add_suffix(
                self.name.as_str(),
                super::app::MAX_ALLOWED_SONG_LEN,
                None,
            ),
            self.album.clone(),
            self.artist.clone(),
        ])
    }
}

impl IntoTableRow for &PlaylistInfo {
    fn row_id(&self) -> Option<u32> {
        self.id
    }

    fn into_row(self) -> ratatui::widgets::Row<'static> {
        ratatui::widgets::Row::new([self.name.clone(), format!("{} Songs", self.songs.len())])
    }
}

#[derive(Debug)]
pub struct QueueEntry {
    pub position: usize,
    pub song_id: u32,
    pub song_name: String,
    pub duration: u32,
}

impl IntoTableRow for &QueueEntry {
    fn row_id(&self) -> Option<u32> {
        Some(self.song_id)
    }

    fn into_row(self) -> ratatui::widgets::Row<'static> {
        ratatui::widgets::Row::new([
            format!("{}.", self.position),
            crate::util::time_util::seconds_to_base60_string(self.duration),
            self.song_name.clone(),
        ])
    }
}
