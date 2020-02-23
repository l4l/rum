use std::io::{stdout, Error, Stdout};

use termion::raw::{IntoRawMode, RawTerminal};
use tui::backend::TermionBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::terminal::Frame;
use tui::widgets::{Block, Borders, List, Paragraph, Text, Widget};
use tui::Terminal;

use crate::view;

type Backend = TermionBackend<RawTerminal<Stdout>>;

pub struct Drawer {
    terminal: Terminal<Backend>,
}

impl Drawer {
    pub fn new() -> Result<Self, Error> {
        let stdout = stdout().into_raw_mode()?;
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;
        terminal.hide_cursor()?;

        Ok(Self { terminal })
    }

    pub fn redraw<'a>(
        &mut self,
        main_view: &view::MainView,
        log_lines: impl Iterator<Item = &'a String>,
    ) -> Result<(), Error> {
        self.terminal.draw(|mut frame| {
            let constraints = if frame.size().height < 20 {
                [Constraint::Length(3), Constraint::Min(0)].as_ref()
            } else {
                &[
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(6),
                ]
            };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(constraints)
                .split(frame.size());
            let texts = [Text::styled(
                main_view.insert_buffer(),
                Style::default().fg(Color::Gray).modifier(Modifier::BOLD),
            )];
            Paragraph::new(texts.iter())
                .block(
                    Block::default()
                        .title(main_view.view().name())
                        .title_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                        .borders(Borders::ALL),
                )
                .alignment(Alignment::Center)
                .wrap(true)
                .render(&mut frame, chunks[0]);

            main_view.view().draw_at(&mut frame, chunks[1]);

            if chunks.len() >= 3 {
                let line = log_lines.last().map(|s| s.as_str()).unwrap_or("");

                Paragraph::new([Text::raw(line)].iter())
                    .wrap(true)
                    .block(
                        Block::default().title("Log").title_style(
                            Style::default().fg(Color::Magenta).modifier(Modifier::BOLD),
                        ),
                    )
                    .render(&mut frame, chunks[2]);
            }
        })
    }
}

impl view::View {
    fn draw_at(&self, frame: &mut Frame<Backend>, chunk: Rect) {
        match self {
            view::View::ArtistSearch(search) => search.draw_at(frame, chunk),
            view::View::AlbumSearch(search) => search.draw_at(frame, chunk),
            view::View::TrackList(list) => list.draw_at(frame, chunk),
            view::View::Playlist(playlist) => playlist.draw_at(frame, chunk),
        }
    }
}

impl view::ArtistSearch {
    fn draw_at(&self, mut frame: &mut Frame<Backend>, chunk: Rect) {
        List::new(cursored_line(
            self.cached_artists.iter().map(|album| &album.name),
            self.cursor,
            chunk,
        ))
        .block(Block::default().title("Artists").borders(Borders::ALL))
        .render(&mut frame, chunk);
    }
}

impl view::AlbumSearch {
    fn draw_at(&self, mut frame: &mut Frame<Backend>, chunk: Rect) {
        List::new(cursored_line(
            self.cached_albums.iter().map(|album| {
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
            }),
            self.cursor,
            chunk,
        ))
        .block(Block::default().title("Albums").borders(Borders::ALL))
        .render(&mut frame, chunk);
    }
}

impl view::TrackList {
    fn draw_at(&self, mut frame: &mut Frame<Backend>, chunk: Rect) {
        List::new(cursored_line(
            self.cached_tracks.iter().map(|track| {
                format!(
                    "{} ({})",
                    track.name,
                    itertools::join(track.artists.iter().map(|a| a.name.as_str()), ", ")
                )
            }),
            self.cursor,
            chunk,
        ))
        .block(Block::default().title("Found Tracks").borders(Borders::ALL))
        .render(&mut frame, chunk);
    }
}

impl view::Playlist {
    fn draw_at(&self, mut frame: &mut Frame<Backend>, chunk: Rect) {
        List::new(cursored_line(
            self.tracks.iter().map(|track| {
                format!(
                    "{} ({})",
                    track.name,
                    itertools::join(track.artists.iter().map(|a| a.name.as_str()), ", ")
                )
            }),
            self.current,
            chunk,
        ))
        .block(Block::default().title("Playlist").borders(Borders::ALL))
        .render(&mut frame, chunk);
    }
}

fn cursored_line<'a>(
    iter: impl IntoIterator<Item = impl Into<String>>,
    cursor_pos: usize,
    chunk: Rect,
) -> impl Iterator<Item = Text<'a>> {
    let half = usize::from(chunk.height) / 2;
    let skip = cursor_pos.saturating_sub(half);
    iter.into_iter()
        .enumerate()
        .skip(skip)
        .map(move |(i, line)| {
            let style = if i == cursor_pos {
                Style::default()
                    .bg(Color::Gray)
                    .fg(Color::Black)
                    .modifier(Modifier::BOLD)
            } else {
                Default::default()
            };
            Text::styled(line.into(), style)
        })
}
