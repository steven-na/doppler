use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub songs: Vec<u32>,
    #[serde(skip)]
    pub file_entry_up_to_date: bool,
}

impl Playlist {
    pub fn new() -> Self {
        Playlist {
            name: "".into(),
            songs: Vec::new(),
            file_entry_up_to_date: false,
        }
    }
}
