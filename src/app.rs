use std::sync::{mpsc, Arc};

use snafu::ResultExt;
use tokio::stream::StreamExt;

use crate::config::Config;
use crate::draw;
use crate::key::{Action, Context as KeyContext};
use crate::player::{self, Command};
use crate::providers::Provider;
use crate::view::{AlbumSearch, ArtistSearch, Playlist, TrackList, TrackSearch, View};

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
        let len = self.view.len();

        if let Some(cursor) = self.view.cursor_mut() {
            if len > *cursor + 1 {
                *cursor += 1;
            }
        }
    }
    fn pointer_up(&mut self) {
        if let Some(cursor) = self.view.cursor_mut() {
            *cursor = cursor.saturating_sub(1);
        }
    }

    fn push_char(&mut self, c: char) {
        if let Some(insert_buffer) = self.view.insert_buffer_mut() {
            insert_buffer.push(c);
        }
    }

    fn backspace(&mut self) {
        if let Some(insert_buffer) = self.view.insert_buffer_mut() {
            insert_buffer.pop();
        } else {
            self.restore_view(); // awkward
        }
    }

    fn restore_view(&mut self) {
        if let Some(view) = self.prev_view.take() {
            self.view = view;
        }
    }

    fn update_view(&mut self, new_view: impl Into<View>) {
        self.prev_view = Some(std::mem::replace(&mut self.view, new_view.into()));
    }

    #[allow(clippy::single_match)]
    async fn switch_to_album_search(&mut self) -> Result<(), crate::providers::Error> {
        match &mut self.view {
            View::ArtistSearch(search) => {
                if let Some(artist) = search.cached_artists.get(search.cursor) {
                    let albums = self.provider.artist_albums(&artist).await?.albums;

                    self.update_view(AlbumSearch::from(albums));
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

                    self.update_view(TrackList::create(tracks));
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
                    let insert_buffer = std::mem::replace(&mut search.insert_buffer, String::new());
                    let artists = album.artists.clone();

                    self.update_view(ArtistSearch::create(insert_buffer, artists));
                } else {
                    search.cursor = 0;
                }
            }
            View::TrackList(list) => {
                if let Some(track) = list.cached_tracks.get(list.cursor) {
                    let artists = track.artists.to_vec();

                    self.update_view(ArtistSearch::from(artists));
                } else {
                    list.cursor = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn action(&mut self) -> Result<Option<Command>, crate::providers::Error> {
        match &mut self.view {
            View::ArtistSearch(search) if !search.insert_buffer.is_empty() => {
                search.cached_artists = self
                    .provider
                    .artists_search(&search.insert_buffer)
                    .await?
                    .artists;
                search.insert_buffer.clear();
            }
            View::AlbumSearch(search) if !search.insert_buffer.is_empty() => {
                search.cached_albums = self
                    .provider
                    .album_search(&search.insert_buffer)
                    .await?
                    .albums;
                search.insert_buffer.clear();
            }
            View::AlbumSearch(search)
                if search.insert_buffer.is_empty() && !search.cached_albums.is_empty() =>
            {
                let album = &search.cached_albums[search.cursor];
                let tracks = self
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
                    .collect();

                self.update_view(TrackList::create(tracks));
            }
            View::TrackSearch(search) => {
                let tracks = self
                    .provider
                    .track_search(&search.insert_buffer)
                    .await?
                    .tracks;
                if !tracks.is_empty() {
                    self.update_view(TrackList::create(tracks));
                }
            }
            View::TrackList(search) => {
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
                Action::FlipPause => player_commands
                    .send(Command::FlipPause)
                    .context(PlayerCommandError { action })?,
                Action::Forward5 => player_commands
                    .send(Command::Seek(5))
                    .context(PlayerCommandError { action })?,
                Action::Backward5 => player_commands
                    .send(Command::Seek(-5))
                    .context(PlayerCommandError { action })?,
                Action::Stop => player_commands
                    .send(Command::Stop)
                    .context(PlayerCommandError { action })?,
                Action::AddAll => {
                    if let View::TrackList(ref search) = state.view {
                        for track in search.cached_tracks.iter() {
                            match state.provider.get_track_url(&track).await {
                                Ok(url) => {
                                    let track = track.clone();
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
                    if let View::Playlist(_) = state.view {
                        state.restore_view();
                    } else {
                        let player_state = state.player_state.lock().unwrap();
                        let tracks = player_state.playlist().cloned().collect();
                        let current = player_state.current();
                        drop(player_state);

                        state.update_view(Playlist::create(tracks, current));
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
                Action::SwitchView => match state.view.clone() {
                    View::AlbumSearch(search) => {
                        state.update_view(TrackSearch::create(search.insert_buffer))
                    }
                    View::TrackSearch(search) => {
                        state.update_view(ArtistSearch::create(search.insert_buffer, vec![]))
                    }
                    View::ArtistSearch(search) => {
                        state.update_view(AlbumSearch::create(search.insert_buffer, vec![]))
                    }
                    _ => continue,
                },
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
