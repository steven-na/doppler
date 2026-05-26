use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SongInfo {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    pub filename: Option<String>,
    pub id: Option<u32>,
    #[serde(skip)]
    pub file_entry_up_to_date: bool,
}

impl SongInfo {
    pub fn new() -> Self {
        SongInfo {
            name: "".into(),
            artist: "".into(),
            album: "".into(),
            duration: 0,
            filename: None,
            id: None,
            file_entry_up_to_date: false,
        }
    }
}
