use std::collections::VecDeque;

use rand::{rng, seq::SliceRandom};

#[derive(Debug, PartialEq)]
pub enum ItemType {
    OriginalItem,
    DynamicItem,
}

#[derive(Debug)]
pub struct PlaylistIter {
    pub queue: VecDeque<(u32, ItemType)>,
    pub last_song: Option<u32>,
}

impl PlaylistIter {
    pub fn new(songs: &[u32]) -> Self {
        PlaylistIter {
            queue: songs
                .iter()
                .copied()
                .map(|id| (id, ItemType::OriginalItem))
                .collect(),
            last_song: None,
        }
    }

    pub fn get_previous(&self) -> Option<u32> {
        self.last_song
    }

    pub fn shuffle(&mut self) {
        let mut rng = rng();
        self.queue.make_contiguous().shuffle(&mut rng);
    }

    pub fn insert_front(&mut self, id: u32) {
        self.queue.push_front((id, ItemType::DynamicItem));
    }

    pub fn insert_back(&mut self, id: u32) {
        self.queue.push_back((id, ItemType::DynamicItem));
    }

    pub fn insert(&mut self, idx: usize, id: u32) -> std::io::Result<()> {
        if idx >= self.queue.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
        self.queue.insert(idx, (id, ItemType::DynamicItem));
        Ok(())
    }

    pub fn insert_dynamics(&mut self, ids: &[u32], frequency: usize) -> std::io::Result<()> {
        let mut idx = 0;
        let mut rng = rng();
        let mut songs = ids.to_vec();
        songs.shuffle(&mut rng);
        songs.retain(|id1| !self.queue.contains(&(*id1, ItemType::OriginalItem)));
        let mut songs = songs.iter();

        while idx < self.queue.len() {
            idx += frequency;
            if let Some(&id) = songs.next() {
                let _ = self.insert(idx, id);
            }
        }
        Ok(())
    }

    pub fn remove_dynamics(&mut self) {
        self.queue
            .retain(|(_, t)| matches!(t, ItemType::OriginalItem));
    }
}

impl Iterator for PlaylistIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        self.queue.pop_front().map(|(id, _)| {
            self.last_song = Some(id);
            id
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.queue.len();
        (len, Some(len))
    }
}
