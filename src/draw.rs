use std::io::{stdout, Error, Stdout};

use termion::raw::{IntoRawMode, RawTerminal};
use tui::backend::TermionBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::terminal::Frame;
use tui::widgets::{Block, Borders, List, Paragraph, Text, Widget};
use tui::Terminal;

type Backend = TermionBackend<RawTerminal<Stdout>>;

pub struct Drawer {
    state: DrawState,
    terminal: Terminal<Backend>,
}

impl Drawer {
    pub fn new() -> Result<Self, Error> {
        let stdout = stdout().into_raw_mode()?;
        let backend = TermionBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let mut this = Self {
            state: DrawState::AlbumSearch(AlbumSearch::new()),
            terminal,
        };

        this.terminal.clear()?;
        this.terminal.hide_cursor()?;
        this.draw()?;

        Ok(this)
    }

    pub fn update_pointer(&mut self, pointer: usize) {
        match &mut self.state {
            DrawState::ArtistSearch(search) => search.cursor = pointer,
            DrawState::AlbumSearch(search) => search.cursor = pointer,
            DrawState::TrackSearch(_) => {}
            DrawState::TrackList(list) => list.cursor = pointer,
        }
    }

    pub fn update_state(&mut self, state: &app::State) -> Result<(), Error> {
        match &state.view {
            app::View::ArtistSearch(search) => {
                self.reset_to_artist_search().update(&search);
            }
            app::View::AlbumSearch(search) => {
                self.reset_to_album_search().update(&search);
            }
            app::View::TrackSearch(search) => {
                self.reset_to_track_search().update(&search);
            }
            app::View::TrackList(list) => {
                self.reset_to_track_list().update(&list);
            }
        }
        self.update_pointer(state.pointer);
        self.draw()
    }

    fn reset_to_artist_search(&mut self) -> &mut ArtistSearch {
        if let DrawState::ArtistSearch(ref mut this) = self.state {
            return this;
        }
        self.state = DrawState::ArtistSearch(ArtistSearch::new());
        if let DrawState::ArtistSearch(ref mut this) = &mut self.state {
            this
        } else {
            unreachable!()
        }
    }

    fn reset_to_album_search(&mut self) -> &mut AlbumSearch {
        if let DrawState::AlbumSearch(ref mut this) = self.state {
            return this;
        }
        self.state = DrawState::AlbumSearch(AlbumSearch::new());
        if let DrawState::AlbumSearch(ref mut this) = &mut self.state {
            this
        } else {
            unreachable!()
        }
    }

    fn reset_to_track_search(&mut self) -> &mut TrackSearch {
        if let DrawState::TrackSearch(ref mut this) = self.state {
            return this;
        }
        self.state = DrawState::TrackSearch(TrackSearch::new());
        if let DrawState::TrackSearch(ref mut this) = &mut self.state {
            this
        } else {
            unreachable!()
        }
    }

    fn reset_to_track_list(&mut self) -> &mut TrackList {
        if let DrawState::TrackList(ref mut this) = self.state {
            return this;
        }
        self.state = DrawState::TrackList(TrackList::new());
        if let DrawState::TrackList(ref mut this) = &mut self.state {
            this
        } else {
            unreachable!()
        }
    }

    pub fn draw(&mut self) -> Result<(), Error> {
        match &self.state {
            DrawState::ArtistSearch(state) => self.terminal.draw(|mut f| state.draw(&mut f)),
            DrawState::AlbumSearch(state) => self.terminal.draw(|mut f| state.draw(&mut f)),
            DrawState::TrackSearch(state) => self.terminal.draw(|mut f| state.draw(&mut f)),
            DrawState::TrackList(state) => self.terminal.draw(|mut f| state.draw(&mut f)),
        }
    }
}

enum DrawState {
    ArtistSearch(ArtistSearch),
    AlbumSearch(AlbumSearch),
    TrackSearch(TrackSearch),
    TrackList(TrackList),
}

struct ArtistSearch {
    insert_buffer: String,
    cursor: usize,
    artist_infos: Vec<String>,
}

impl ArtistSearch {
    fn new() -> Self {
        Self {
            insert_buffer: String::with_capacity(256),
            cursor: 0,
            artist_infos: Vec::new(),
        }
    }

    fn update(&mut self, artists: &app::ArtistSearch) {
        self.insert_buffer = artists.insert_buffer.clone();
        self.artist_infos = artists
            .cached_artists
            .iter()
            .map(|album| album.name.clone())
            .collect();
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());
        let texts = [Text::styled(
            &self.insert_buffer,
            Style::default().fg(Color::Gray).modifier(Modifier::BOLD),
        )];
        Paragraph::new(texts.iter())
            .block(
                Block::default()
                    .title("Artist Search String")
                    .title_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center)
            .wrap(true)
            .render(&mut frame, chunks[0]);

        List::new(cursored_line(
            self.artist_infos.iter(),
            self.cursor,
            chunks[1],
        ))
        .block(Block::default().title("Artists").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

struct AlbumSearch {
    insert_buffer: String,
    cursor: usize,
    album_infos: Vec<String>,
}

use crate::app;

impl AlbumSearch {
    fn new() -> Self {
        Self {
            insert_buffer: String::with_capacity(256),
            cursor: 0,
            album_infos: Vec::new(),
        }
    }

    fn update(&mut self, albums: &app::AlbumSearch) {
        self.insert_buffer = albums.insert_buffer.clone();
        self.album_infos = albums
            .cached_albums
            .iter()
            .map(|album| {
                if let Some(ref version) = album.version {
                    format!(
                        "{}: {} (year: {}, {})",
                        album
                            .artists
                            .get(0)
                            .map(|a| a.name.as_str())
                            .unwrap_or("unknown"),
                        album.title,
                        album.year,
                        version
                    )
                } else {
                    format!(
                        "{}: {} (year: {})",
                        album
                            .artists
                            .get(0)
                            .map(|a| a.name.as_str())
                            .unwrap_or("unknown"),
                        album.title,
                        album.year
                    )
                }
            })
            .collect();
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());
        let texts = [Text::styled(
            &self.insert_buffer,
            Style::default().fg(Color::Gray).modifier(Modifier::BOLD),
        )];
        Paragraph::new(texts.iter())
            .block(
                Block::default()
                    .title("Album Search String")
                    .title_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center)
            .wrap(true)
            .render(&mut frame, chunks[0]);

        List::new(cursored_line(
            self.album_infos.iter(),
            self.cursor,
            chunks[1],
        ))
        .block(Block::default().title("Albums").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

struct TrackSearch {
    insert_buffer: String,
}

impl TrackSearch {
    fn new() -> Self {
        Self {
            insert_buffer: String::with_capacity(256),
        }
    }

    fn update(&mut self, albums: &app::TrackSearch) {
        self.insert_buffer = albums.insert_buffer.clone();
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());
        let texts = [Text::styled(
            &self.insert_buffer,
            Style::default().fg(Color::Gray).modifier(Modifier::BOLD),
        )];
        Paragraph::new(texts.iter())
            .block(
                Block::default()
                    .title("Track Search String")
                    .title_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center)
            .wrap(true)
            .render(&mut frame, chunks[0]);
    }
}

struct TrackList {
    cursor: usize,
    tracks: Vec<String>,
}

impl TrackList {
    fn new() -> Self {
        Self {
            cursor: 0,
            tracks: Vec::new(),
        }
    }

    fn update(&mut self, tracks: &app::TrackList) {
        self.tracks = tracks
            .cached_tracks
            .iter()
            .map(|track| {
                format!(
                    "{} ({})",
                    track.name,
                    itertools::join(track.artists.iter().map(|a| a.name.as_str()), ", ")
                )
            })
            .collect();
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());

        List::new(cursored_line(self.tracks.iter(), self.cursor, chunks[1]))
            .block(Block::default().title("Found Tracks").borders(Borders::ALL))
            .render(&mut frame, chunks[1]);
    }
}

fn cursored_line(
    iter: impl IntoIterator<Item = impl ToString>,
    cursor_pos: usize,
    chunk: Rect,
) -> impl Iterator<Item = Text<'static>> {
    let half = usize::from(chunk.height) / 2;
    let skip = cursor_pos.saturating_sub(half);
    iter.into_iter()
        .skip(skip)
        .enumerate()
        .map(move |(i, line)| {
            let line = line.to_string();
            let style = if i + skip == cursor_pos {
                Style::default()
                    .bg(Color::Gray)
                    .fg(Color::Black)
                    .modifier(Modifier::BOLD)
            } else {
                Default::default()
            };
            Text::styled(line, style)
        })
}
