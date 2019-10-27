use termion::raw::{IntoRawMode, RawTerminal};
use tui::backend::TermionBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::terminal::Frame;
use tui::widgets::{Block, Borders, List, Paragraph, Text, Widget};
use tui::Terminal;

type Backend = TermionBackend<RawTerminal<std::io::Stdout>>;

pub struct Drawer {
    state: DrawState,
    terminal: Terminal<Backend>,
}

impl Drawer {
    pub fn new() -> Result<Self, std::io::Error> {
        let stdout = std::io::stdout().into_raw_mode()?;
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
            DrawState::AlbumSearch(search) => search.cursor = pointer,
            DrawState::TracksList(search) => search.cursor = pointer,
        }
    }

    pub fn update_state(&mut self, state: &app::State) -> Result<(), std::io::Error> {
        self.update_pointer(state.pointer);
        match &state.view {
            app::View::Start(buffer) => {
                let mut s = AlbumSearch::new();
                s.insert_buffer = buffer.clone();
                self.state = DrawState::AlbumSearch(s);
            }
            app::View::AlbumSearch(search) => {
                self.reset_to_album_search().update_albums(&search);
            }
            app::View::TrackSearch(search) => {
                self.reset_to_track_search().update_tracks(&search);
            }
        }
        self.draw()
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

    fn reset_to_track_search(&mut self) -> &mut TracksList {
        if let DrawState::TracksList(ref mut this) = self.state {
            return this;
        }
        self.state = DrawState::TracksList(TracksList::new());
        if let DrawState::TracksList(ref mut this) = &mut self.state {
            this
        } else {
            unreachable!()
        }
    }

    pub fn draw(&mut self) -> Result<(), std::io::Error> {
        match &self.state {
            DrawState::AlbumSearch(state) => self.terminal.draw(|mut f| state.draw(&mut f)),
            DrawState::TracksList(state) => self.terminal.draw(|mut f| state.draw(&mut f)),
        }
    }
}

enum DrawState {
    AlbumSearch(AlbumSearch),
    TracksList(TracksList),
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

    fn update_albums(&mut self, albums: &app::AlbumSearch) {
        self.insert_buffer = albums.insert_buffer.clone();
        self.album_infos = albums
            .cached_albums
            .iter()
            .map(|album| {
                format!(
                    "{}: {} (year: {})",
                    album.artist,
                    album.title,
                    album
                        .year
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
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
            &chunks[1],
        ))
        .block(Block::default().title("Albums").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

struct TracksList {
    cursor: usize,
    tracks: Vec<String>,
}

impl TracksList {
    fn new() -> Self {
        Self {
            cursor: 0,
            tracks: Vec::new(),
        }
    }

    fn update_tracks(&mut self, tracks: &app::TrackSearch) {
        self.tracks = tracks
            .cached_tracks
            .iter()
            .map(|track| track.name.clone())
            .collect();
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());

        List::new(cursored_line(self.tracks.iter(), self.cursor, &chunks[1]))
            .block(Block::default().title("Tracks").borders(Borders::ALL))
            .render(&mut frame, chunks[1]);
    }
}

fn cursored_line(
    iter: impl IntoIterator<Item = impl ToString>,
    cursor_pos: usize,
    chunk: &Rect,
) -> impl Iterator<Item = Text<'static>> {
    let half = usize::from(chunk.height) / 2;
    let skip = cursor_pos.checked_sub(half).unwrap_or(0);
    iter.into_iter()
        .skip(skip)
        .enumerate()
        .map(move |(i, line)| {
            if i + skip == cursor_pos {
                Text::styled(
                    line.to_string(),
                    Style::default()
                        .bg(Color::Gray)
                        .fg(Color::Black)
                        .modifier(Modifier::BOLD),
                )
            } else {
                Text::styled(line.to_string(), Default::default())
            }
        })
}
