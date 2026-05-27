use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaylistInfo {
    pub name: String,
    pub songs: Vec<u32>,
    pub id: Option<u32>,
    #[serde(skip)]
    pub file_entry_up_to_date: bool,
}

impl PlaylistInfo {
    pub fn new() -> Self {
        PlaylistInfo {
            name: "".into(),
            songs: Vec::new(),
            id: None,
            file_entry_up_to_date: false,
        }
    }

    pub fn add_song(&mut self, id: u32) -> std::io::Result<()> {
        if self.songs.contains(&id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Song already in playlist",
            ));
        }

        self.songs.push(id);
        self.file_entry_up_to_date = false;

        Ok(())
    }

    pub fn remove_song(&mut self, idx: usize) -> std::io::Result<()> {
        if idx < self.songs.len() {
            self.songs.remove(idx);
            self.file_entry_up_to_date = false;
            return Ok(());
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
    }

    pub fn force_add_song(&mut self, id: u32) {
        self.songs.push(id);
        self.file_entry_up_to_date = false;
    }
}

impl Default for PlaylistInfo {
    fn default() -> Self {
        Self::new()
    }
}
