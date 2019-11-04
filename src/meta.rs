use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Artist {
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Artists {
    pub artists: Vec<Artist>,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub url: String,
    pub title: String,
    pub artists: Vec<Artist>,
    pub year: u16,
    pub version: Option<String>,
}

impl Album {
    #[allow(unused)]
    pub fn id(&self) -> u32 {
        self.url.split('/').nth(1).unwrap().parse().unwrap()
    }
}

#[derive(Debug)]
pub struct Albums {
    pub albums: Vec<Album>,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub album_id: u32,
    pub track_id: u32,
    pub name: String,
    pub artists: Arc<Vec<Artist>>,
}

#[derive(Debug)]
pub struct Tracks {
    pub tracks: Vec<Track>,
}
