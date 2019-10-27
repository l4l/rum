use std::sync::mpsc;

use termion::event::Key;
use termion::input::TermRead;

use crate::draw::Drawer;
use crate::player::Command;
use crate::providers::{Album, Provider, Track};

#[derive(Debug, Clone)]
pub struct AlbumSearch {
    pub insert_buffer: String,
    pub cached_albums: Vec<Album>,
}

#[derive(Debug, Clone)]
pub struct TrackSearch {
    pub cached_tracks: Vec<Track>,
}

pub struct State {
    provider: Provider,
    prev_view: Option<(usize, View)>,
    pub view: View,
    pub pointer: usize,
}

impl State {
    fn new(provider: Provider) -> Self {
        Self {
            provider,
            prev_view: None,
            view: View::default(),
            pointer: 0,
        }
    }

    fn pointer_down(&mut self) {
        use View::*;
        let len = match &self.view {
            AlbumSearch(search) => search.cached_albums.len(),
            TrackSearch(search) => search.cached_tracks.len(),
            _ => return,
        };
        if len != 0 && self.pointer < len - 1 {
            self.pointer += 1;
        }
    }
    fn pointer_up(&mut self) {
        use View::*;
        match &self.view {
            AlbumSearch(_) | TrackSearch(_) => {
                if self.pointer > 0 {
                    self.pointer -= 1;
                }
            }
            _ => {}
        }
    }

    fn push_char(&mut self, c: char) {
        use View::*;
        match &mut self.view {
            Start(buffer)
            | AlbumSearch(self::AlbumSearch {
                insert_buffer: buffer,
                ..
            }) => buffer.push(c),
            TrackSearch(_) => {}
        }
    }

    fn backspace(&mut self) {
        use View::*;
        match &mut self.view {
            Start(buffer)
            | AlbumSearch(self::AlbumSearch {
                insert_buffer: buffer,
                ..
            }) => {
                buffer.pop();
            }
            TrackSearch(_) => {
                if let Some((pointer, previous)) = self.prev_view.take() {
                    self.pointer = pointer;
                    self.view = previous;
                }
            }
        }
    }

    async fn action(&mut self) -> Result<Option<Command>, crate::providers::Error> {
        use View::*;
        match &mut self.view {
            Start(buffer) => {
                self.pointer = 0;
                self.view = AlbumSearch(self::AlbumSearch {
                    insert_buffer: String::new(),
                    cached_albums: self.provider.text_search(&buffer).await?.albums,
                });
            }
            AlbumSearch(search) if !search.insert_buffer.is_empty() => {
                search.cached_albums = self
                    .provider
                    .text_search(&search.insert_buffer)
                    .await?
                    .albums;
                search.insert_buffer.clear();
            }
            AlbumSearch(search)
                if search.insert_buffer.is_empty() && !search.cached_albums.is_empty() =>
            {
                self.prev_view = Some((self.pointer, View::AlbumSearch(search.clone())));
                let album = &search.cached_albums[self.pointer];
                self.pointer = 0;
                self.view = TrackSearch(self::TrackSearch {
                    cached_tracks: self.provider.album_tracks(&album).await?.tracks,
                });
            }
            TrackSearch(search) => {
                let track = &search.cached_tracks[self.pointer];
                let url = self.provider.get_track_url(&track).await?;
                return Ok(Some(Command::Play(url)));
            }
            _ => {}
        }
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub enum View {
    Start(String),
    AlbumSearch(AlbumSearch),
    TrackSearch(TrackSearch),
}

impl Default for View {
    fn default() -> Self {
        View::Start(String::with_capacity(256))
    }
}

pub struct App {
    provider: Provider,
    player_commands: mpsc::Sender<Command>,
}

impl App {
    pub fn create(
        provider: Provider,
        player_commands: mpsc::Sender<Command>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            provider,
            player_commands,
        })
    }

    fn events() -> mpsc::Receiver<Key> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || -> Result<(), std::io::Error> {
            // eventually switch to tokio::io::stdin(), but need to parse keys manually
            for key in std::io::stdin().keys() {
                let key: Key = key?;
                if tx.send(key).is_err() {
                    break;
                }
            }
            Ok(())
        });
        rx
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let App {
            provider,
            player_commands,
        } = self;

        let mut state = State::new(provider);
        let mut drawer = Drawer::new()?;

        for ev in App::events() {
            match ev {
                Key::Up => state.pointer_up(),
                Key::Down => state.pointer_down(),
                Key::Delete => return Ok(()),
                Key::Ctrl('r') => drawer.draw()?,
                Key::Ctrl('p') => player_commands.send(Command::ContinueOrStop)?,
                Key::Char('\n') => {
                    if let Some(cmd) = state.action().await? {
                        player_commands.send(cmd)?;
                    }
                }
                Key::Char(c) => state.push_char(c),
                Key::Backspace => state.backspace(),
                _ => {
                    continue;
                }
            }

            drawer.update_state(&state)?;
        }
        Ok(())
    }
}
