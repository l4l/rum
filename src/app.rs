use std::sync::{mpsc, Arc};

use snafu::ResultExt;
use tokio::stream::StreamExt;

use crate::config::Config;
use crate::draw;
use crate::key::{Action, Context as KeyContext};
use crate::meta::{Album, Artist, Track};
use crate::player::{self, Command};
use crate::providers::Provider;

#[derive(Debug, Clone)]
pub struct ArtistSearch {
    pub insert_buffer: String,
    pub cached_artists: Vec<Artist>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct AlbumSearch {
    pub insert_buffer: String,
    pub cached_albums: Vec<Album>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct TrackSearch {
    pub insert_buffer: String,
}

#[derive(Debug, Clone)]
pub struct TrackList {
    pub cached_tracks: Vec<Track>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub tracks: Vec<Track>,
    pub current: usize,
    prev_view: Box<View>,
}

#[derive(Debug, Clone)]
pub enum View {
    ArtistSearch(ArtistSearch),
    AlbumSearch(AlbumSearch),
    TrackSearch(TrackSearch),
    TrackList(TrackList),
    Playlist(Playlist),
}

impl Default for View {
    fn default() -> Self {
        View::AlbumSearch(AlbumSearch {
            insert_buffer: String::with_capacity(256),
            cached_albums: vec![],
            cursor: 0,
        })
    }
}

struct State {
    provider: Provider,
    player_state: player::State,
    prev_view: Option<View>,
    view: View,
}

impl State {
    fn new(provider: Provider, player_state: player::State) -> Self {
        Self {
            provider,
            player_state,
            prev_view: None,
            view: View::default(),
        }
    }

    fn pointer_down(&mut self) {
        use View::*;
        match &mut self.view {
            ArtistSearch(search) => {
                if search.cached_artists.len() > search.cursor + 1 {
                    search.cursor += 1;
                }
            }
            AlbumSearch(search) => {
                if search.cached_albums.len() > search.cursor + 1 {
                    search.cursor += 1;
                }
            }
            TrackSearch(_) | Playlist(_) => {}
            TrackList(search) => {
                if search.cached_tracks.len() > search.cursor + 1 {
                    search.cursor += 1;
                }
            }
        }
    }
    fn pointer_up(&mut self) {
        use View::*;
        match &mut self.view {
            ArtistSearch(self::ArtistSearch { cursor, .. })
            | AlbumSearch(self::AlbumSearch { cursor, .. })
            | TrackList(self::TrackList { cursor, .. }) => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            TrackSearch(_) | Playlist(_) => {}
        }
    }

    fn push_char(&mut self, c: char) {
        use View::*;
        match &mut self.view {
            ArtistSearch(self::ArtistSearch { insert_buffer, .. })
            | AlbumSearch(self::AlbumSearch { insert_buffer, .. })
            | TrackSearch(self::TrackSearch { insert_buffer }) => insert_buffer.push(c),
            TrackList(_) | Playlist(_) => {}
        }
    }

    fn backspace(&mut self) {
        use View::*;
        match &mut self.view {
            ArtistSearch(self::ArtistSearch { insert_buffer, .. })
            | AlbumSearch(self::AlbumSearch { insert_buffer, .. })
            | TrackSearch(self::TrackSearch { insert_buffer }) => {
                insert_buffer.pop();
            }
            TrackList(_) => {
                if let Some(previous) = self.prev_view.take() {
                    self.view = previous;
                }
            }
            Playlist(_) => {}
        }
    }

    #[allow(clippy::single_match)]
    async fn switch_to_album_search(&mut self) -> Result<(), crate::providers::Error> {
        match &mut self.view {
            View::ArtistSearch(search) => {
                if let Some(artist) = search.cached_artists.get(search.cursor) {
                    self.prev_view = Some(View::ArtistSearch(search.clone()));
                    let albums = self.provider.artist_albums(&artist).await?.albums;
                    self.view = View::AlbumSearch(AlbumSearch {
                        insert_buffer: String::with_capacity(256),
                        cached_albums: albums,
                        cursor: 0,
                    });
                } else {
                    search.cursor = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[allow(clippy::single_match)]
    async fn switch_to_track_search(&mut self) -> Result<(), crate::providers::Error> {
        match &mut self.view {
            View::ArtistSearch(search) => {
                if let Some(artist) = search.cached_artists.get(search.cursor) {
                    self.prev_view = Some(View::ArtistSearch(search.clone()));
                    let tracks = self
                        .provider
                        .artist_tracks(&artist)
                        .await?
                        .tracks
                        .into_iter()
                        .map(|mut track| {
                            Arc::get_mut(&mut track.artists)
                                .unwrap()
                                .insert(0, artist.clone());
                            track
                        })
                        .collect();
                    self.view = View::TrackList(TrackList {
                        cached_tracks: tracks,
                        cursor: 0,
                    });
                } else {
                    search.cursor = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn switch_to_artist(&mut self) -> Result<(), crate::providers::Error> {
        match &mut self.view {
            View::AlbumSearch(search) => {
                if let Some(album) = search.cached_albums.get(search.cursor) {
                    self.prev_view = Some(View::AlbumSearch(search.clone()));
                    self.view = View::ArtistSearch(ArtistSearch {
                        insert_buffer: std::mem::replace(&mut search.insert_buffer, String::new()),
                        cached_artists: album.artists.clone(),
                        cursor: 0,
                    })
                } else {
                    search.cursor = 0;
                }
            }
            View::TrackList(list) => {
                if let Some(track) = list.cached_tracks.get(list.cursor) {
                    self.prev_view = Some(View::TrackList(list.clone()));
                    self.view = View::ArtistSearch(ArtistSearch {
                        insert_buffer: String::new(),
                        cached_artists: track.artists.to_vec(),
                        cursor: 0,
                    })
                } else {
                    list.cursor = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn action(&mut self) -> Result<Option<Command>, crate::providers::Error> {
        use View::*;
        match &mut self.view {
            ArtistSearch(search) if !search.insert_buffer.is_empty() => {
                search.cached_artists = self
                    .provider
                    .artists_search(&search.insert_buffer)
                    .await?
                    .artists;
                search.insert_buffer.clear();
            }
            AlbumSearch(search) if !search.insert_buffer.is_empty() => {
                search.cached_albums = self
                    .provider
                    .album_search(&search.insert_buffer)
                    .await?
                    .albums;
                search.insert_buffer.clear();
            }
            AlbumSearch(search)
                if search.insert_buffer.is_empty() && !search.cached_albums.is_empty() =>
            {
                self.prev_view = Some(AlbumSearch(search.clone()));
                let album = &search.cached_albums[search.cursor];
                self.view = TrackList(self::TrackList {
                    cached_tracks: self
                        .provider
                        .album_tracks(&album)
                        .await?
                        .tracks
                        .into_iter()
                        .map(|mut track| {
                            let track_artists = Arc::get_mut(&mut track.artists).unwrap();
                            // XXX: quadratic complexity here, though maybe ok due to small sizes
                            for album_artist in album.artists.iter() {
                                if !track_artists.iter().any(|x| x.name == album_artist.name) {
                                    track_artists.push(album_artist.clone());
                                }
                            }
                            track
                        })
                        .collect(),
                    cursor: 0,
                });
            }
            TrackSearch(search) => {
                let tracks = self
                    .provider
                    .track_search(&search.insert_buffer)
                    .await?
                    .tracks;
                if !tracks.is_empty() {
                    self.prev_view = Some(TrackSearch(search.clone()));
                    self.view = TrackList(self::TrackList {
                        cached_tracks: tracks,
                        cursor: 0,
                    })
                }
            }
            TrackList(search) => {
                let track = search.cached_tracks[search.cursor].clone();
                let url = self.provider.get_track_url(&track).await?;
                return Ok(Some(Command::Enqueue { track, url }));
            }
            _ => {}
        }
        Ok(None)
    }
}

#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("player error at {:?}: {}", action, source))]
    PlayerCommandError {
        action: Action,
        source: mpsc::SendError<Command>,
    },
    #[snafu(display("draw error at {}: {}", case, source))]
    Drawer {
        case: &'static str,
        source: std::io::Error,
    },
}

pub struct App {
    config: Config,
    provider: Provider,
    player_commands: mpsc::Sender<Command>,
    player_state: player::State,
}

impl App {
    pub fn create(
        config: Config,
        provider: Provider,
        player_commands: mpsc::Sender<Command>,
        player_state: player::State,
    ) -> Result<Self, Error> {
        Ok(Self {
            config,
            provider,
            player_commands,
            player_state,
        })
    }

    pub async fn run(self) -> Result<(), Error> {
        let App {
            config,
            provider,
            player_commands,
            player_state,
        } = self;

        let mut state = State::new(provider, player_state);
        let mut drawer = draw::Drawer::new().context(Drawer {
            case: "create context",
        })?;

        drawer.redraw(&state.view).context(Drawer {
            case: "initial draw",
        })?;

        let (mut events, current_context) = config.binding.actions();

        while let Some(action) = events.next().await {
            match action {
                Action::PointerUp => state.pointer_up(),
                Action::PointerDown => state.pointer_down(),
                Action::NextTrack => player_commands
                    .send(Command::NextTrack)
                    .context(PlayerCommandError { action })?,
                Action::PrevTrack => player_commands
                    .send(Command::PrevTrack)
                    .context(PlayerCommandError { action })?,
                Action::Quit => return Ok(()),
                Action::Pause => player_commands
                    .send(Command::Pause)
                    .context(PlayerCommandError { action })?,
                Action::Forward5 => player_commands
                    .send(Command::Forward5)
                    .context(PlayerCommandError { action })?,
                Action::Backward5 => player_commands
                    .send(Command::Backward5)
                    .context(PlayerCommandError { action })?,
                Action::Stop => player_commands
                    .send(Command::Stop)
                    .context(PlayerCommandError { action })?,
                Action::AddAll => {
                    if let View::TrackList(ref search) = state.view {
                        for track in search.cached_tracks.iter().cloned() {
                            match state.provider.get_track_url(&track).await {
                                Ok(url) => {
                                    player_commands
                                        .send(Command::Enqueue { track, url })
                                        .context(PlayerCommandError { action })?;
                                }
                                Err(err) => {
                                    log::error!("cannot get track {:?} url: {}", track, err);
                                }
                            }
                        }
                    }
                }
                Action::ShowPlaylist => {
                    if let View::Playlist(view) = state.view {
                        state.view = *view.prev_view;
                    } else {
                        let player_state = state.player_state.lock().unwrap();
                        state.view = View::Playlist(Playlist {
                            tracks: player_state.playlist().cloned().collect(),
                            current: player_state.current(),
                            prev_view: Box::new(state.view),
                        });
                    }
                }
                Action::SwitchToAlbums => {
                    if let Err(err) = state.switch_to_album_search().await {
                        log::error!("cannot switch to album search: {}", err);
                    }
                }
                Action::SwitchToTracks => {
                    if let Err(err) = state.switch_to_track_search().await {
                        log::error!("cannot switch to track search: {}", err);
                    }
                }
                Action::SwitchToArtists => {
                    if let Err(err) = state.switch_to_artist().await {
                        log::error!("cannot switch to artist: {}", err);
                    }
                }
                Action::Enter => match state.action().await {
                    Ok(Some(cmd)) => {
                        player_commands
                            .send(cmd)
                            .context(PlayerCommandError { action })?;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("cannot perform action {}", err);
                    }
                },
                Action::SwitchView => {
                    state.view = match state.view {
                        View::AlbumSearch(search) => View::TrackSearch(TrackSearch {
                            insert_buffer: search.insert_buffer,
                        }),
                        View::TrackSearch(search) => View::ArtistSearch(ArtistSearch {
                            insert_buffer: search.insert_buffer,
                            cached_artists: vec![],
                            cursor: 0,
                        }),
                        View::ArtistSearch(search) => View::AlbumSearch(AlbumSearch {
                            insert_buffer: search.insert_buffer,
                            cached_albums: vec![],
                            cursor: 0,
                        }),
                        _ => continue,
                    }
                }
                Action::Char(c) => state.push_char(c),
                Action::Backspace => state.backspace(),
                _ => {
                    continue;
                }
            }

            *current_context.lock().unwrap() = match state.view {
                View::AlbumSearch(_) | View::TrackSearch(_) | View::ArtistSearch(_) => {
                    KeyContext::search()
                }
                View::TrackList(_) => KeyContext::tracklist(),
                View::Playlist(_) => KeyContext::playlist(),
            };

            drawer.redraw(&state.view).context(Drawer {
                case: "loop update state",
            })?;
        }
        Ok(())
    }
}
