use std::sync::mpsc::{self};
use std::time::Duration;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use tui::backend::TermionBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::terminal::Frame;
use tui::widgets::{Block, Borders, List, Paragraph, Text, Widget};
use tui::Terminal;

use crate::player::Command;
use crate::providers::{Album, Provider, Track};

type Backend = TermionBackend<RawTerminal<std::io::Stdout>>;

#[derive(Default)]
pub struct DataState {
    current_albums: Option<Vec<Album>>,
    current_tracks: Option<Vec<Track>>,
}

pub struct Interafce {
    terminal: Terminal<Backend>,
    insert_dirty: bool,
    state: DrawState,
    cached_data: DataState,
    provider: Provider,
    player_commands: mpsc::Sender<Command>,
}

impl Interafce {
    pub fn create(
        provider: Provider,
        player_commands: mpsc::Sender<Command>,
    ) -> Result<Self, std::io::Error> {
        let stdout = std::io::stdout().into_raw_mode()?;
        let backend = TermionBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            insert_dirty: false,
            state: DrawState::new(),
            cached_data: Default::default(),
            provider,
            player_commands,
        })
    }

    pub fn unmark_dirty(&mut self) -> bool {
        let prev = self.insert_dirty;
        self.insert_dirty = false;
        prev
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for key in std::io::stdin().keys() {
                if tx.send(key).is_err() {
                    break;
                }
            }
        });

        self.terminal.clear()?;
        self.terminal.show_cursor()?;
        self.terminal.set_cursor(1, 1)?;
        let mut no_update = false;
        loop {
            if !no_update {
                self.state.draw(&mut self.terminal)?;
            } else {
                // hacky way for preventing a high cpu load
                // better way will be a reaction to events, rather than looping
                std::thread::sleep(Duration::from_millis(50));
            }

            no_update = true;
            while let Ok(key) = rx.try_recv() {
                no_update = false;
                match key? {
                    Key::Up => {
                        *self.state.scrolled_mut() =
                            self.state.scrolled_mut().checked_sub(1).unwrap_or(0);
                    }
                    Key::Down => {
                        *self.state.scrolled_mut() += 1;
                    }
                    Key::Delete => return Ok(()),
                    Key::Char('\n') | Key::Insert => {
                        self.insert_dirty = true;
                    }
                    Key::Char(c) => {
                        if let DrawState::AlbumSearch(ref mut state) = self.state {
                            state.insert_buf.push(c);
                        }
                    }
                    Key::Ctrl('p') => {
                        self.player_commands.send(Command::ContinueOrStop)?;
                    }
                    Key::Backspace => match &mut self.state {
                        DrawState::AlbumSearch(ref mut state) => {
                            state.insert_buf.pop();
                        }
                        DrawState::TracksList(_) => {
                            let mut state = AlbumSearch::new();
                            state.reset_albums(self.cached_data.current_albums.as_ref().unwrap());
                            self.state = DrawState::AlbumSearch(state);
                        }
                    },
                    _ => {}
                }
            }

            if !self.unmark_dirty() {
                continue;
            }
            match &mut self.state {
                DrawState::AlbumSearch(state) if !state.insert_buf.is_empty() => {
                    let albums = self.provider.text_search(&state.insert_buf).await?.albums;
                    state.insert_buf.clear();
                    self.cached_data.current_albums = Some(albums);
                    state.reset_albums(self.cached_data.current_albums.as_ref().unwrap());
                }
                DrawState::AlbumSearch(state) => {
                    if let Some(ref current_albums) = self.cached_data.current_albums {
                        if let Some(album_info) = current_albums.get(state.scrolled) {
                            let tracks = self.provider.album_tracks(&album_info).await?.tracks;
                            let list = TracksList::new(
                                tracks.iter().map(|track| track.name.clone()).collect(),
                            );
                            self.cached_data.current_tracks = Some(tracks);
                            self.state = DrawState::TracksList(list);
                        }
                    }
                }
                DrawState::TracksList(state) => {
                    if let Some(track) = self
                        .cached_data
                        .current_tracks
                        .as_ref()
                        .unwrap()
                        .get(state.scrolled)
                    {
                        let url = self.provider.get_track_url(&track).await?;
                        self.player_commands.send(Command::Play(url))?;
                    }
                }
            }
        }
    }
}

enum DrawState {
    AlbumSearch(AlbumSearch),
    TracksList(TracksList),
}

impl DrawState {
    fn new() -> Self {
        DrawState::AlbumSearch(AlbumSearch::new())
    }

    fn draw(&mut self, terminal: &mut Terminal<Backend>) -> Result<(), std::io::Error> {
        match self {
            DrawState::AlbumSearch(state) => terminal.draw(|mut f| state.draw(&mut f)),
            DrawState::TracksList(state) => terminal.draw(|mut f| state.draw(&mut f)),
        }
    }

    fn scrolled_mut(&mut self) -> &mut usize {
        match self {
            DrawState::AlbumSearch(state) => &mut state.scrolled,
            DrawState::TracksList(state) => &mut state.scrolled,
        }
    }
}

struct AlbumSearch {
    insert_buf: String,
    scrolled: usize,
    album_infos: Vec<String>,
}

impl AlbumSearch {
    fn reset_albums<'a>(&mut self, albums: impl IntoIterator<Item = &'a Album>) {
        self.scrolled = 0;
        self.album_infos = albums
            .into_iter()
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

    fn new() -> Self {
        Self {
            insert_buf: String::with_capacity(256),
            scrolled: 0,
            album_infos: Vec::new(),
        }
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());
        let texts = [Text::styled(
            &self.insert_buf,
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

        List::new(
            self.album_infos
                .iter()
                .skip(self.scrolled)
                .map(|line| Text::raw(line.to_string())),
        )
        .block(Block::default().title("Albums").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

struct TracksList {
    scrolled: usize,
    tracks: Vec<String>,
}

impl TracksList {
    fn new(tracks: Vec<String>) -> Self {
        Self {
            scrolled: 0,
            tracks,
        }
    }

    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(5), Constraint::Percentage(80)].as_ref())
            .split(frame.size());

        List::new(
            self.tracks
                .iter()
                .skip(self.scrolled)
                .map(|line| Text::raw(line.to_string())),
        )
        .block(Block::default().title("Tracks").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}
