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

    pub fn redraw(&mut self, view: &view::View) -> Result<(), Error> {
        match &view {
            view::View::ArtistSearch(search) => self.terminal.draw(|mut frame| {
                search.draw(&mut frame);
            }),
            view::View::AlbumSearch(search) => self.terminal.draw(|mut frame| {
                search.draw(&mut frame);
            }),
            view::View::TrackList(list) => self.terminal.draw(|mut frame| {
                list.draw(&mut frame);
            }),
            view::View::Playlist(playlist) => self.terminal.draw(|mut frame| {
                playlist.draw(&mut frame);
            }),
        }
    }
}

impl view::ArtistSearch {
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
            self.cached_artists.iter().map(|album| &album.name),
            self.cursor,
            chunks[1],
        ))
        .block(Block::default().title("Artists").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

impl view::AlbumSearch {
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
            chunks[1],
        ))
        .block(Block::default().title("Albums").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

impl view::TrackList {
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
        List::new(cursored_line(
            self.cached_tracks.iter().map(|track| {
                format!(
                    "{} ({})",
                    track.name,
                    itertools::join(track.artists.iter().map(|a| a.name.as_str()), ", ")
                )
            }),
            self.cursor,
            chunks[1],
        ))
        .block(Block::default().title("Found Tracks").borders(Borders::ALL))
        .render(&mut frame, chunks[1]);
    }
}

impl view::Playlist {
    fn draw(&self, mut frame: &mut Frame<Backend>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(frame.size());

        List::new(cursored_line(
            self.tracks.iter().map(|track| {
                format!(
                    "{} ({})",
                    track.name,
                    itertools::join(track.artists.iter().map(|a| a.name.as_str()), ", ")
                )
            }),
            self.current,
            chunks[0],
        ))
        .block(Block::default().title("Playlist").borders(Borders::ALL))
        .render(&mut frame, chunks[0]);
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
