use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};

use mpv::{MpvHandler, MpvHandlerBuilder, Result};

use crate::meta::Track;

struct MediaWorker {
    handler: MpvHandler,
    is_paused: bool,
}

impl MediaWorker {
    fn new() -> Result<Self> {
        let handler = MpvHandlerBuilder::new()?.build()?;
        Ok(Self {
            handler,
            is_paused: false,
        })
    }

    fn loadfile(&mut self, url: &str) -> Result<()> {
        self.handler.command(&["loadfile", &url, "append-play"])?;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.handler.command(&["stop"])?;
        Ok(())
    }

    fn next(&mut self) -> Result<()> {
        self.handler.command(&["playlist-next"])?;
        Ok(())
    }

    fn prev(&mut self) -> Result<()> {
        self.handler.command(&["playlist-prev"])?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.is_paused ^= true;
        self.handler.set_property("pause", self.is_paused)?;
        Ok(())
    }

    fn time_seek(&mut self, f: impl FnOnce(i64) -> i64) -> Result<()> {
        let pos: i64 = self.handler.get_property("time-pos")?;
        self.handler.set_property("time-pos", f(pos))?;
        Ok(())
    }

    fn playlist_pos(&self) -> Result<usize> {
        let pos: i64 = self.handler.get_property("playlist-pos")?;
        Ok(pos as usize)
    }

    fn poll_events(&mut self) -> Result<bool> {
        while let Some(ev) = self.handler.wait_event(0.1) {
            match ev {
                mpv::Event::Shutdown | mpv::Event::Idle => {
                    return Ok(false);
                }
                mpv::Event::FileLoaded => {
                    log::debug!("mpv: file loaded");
                }
                _ => {}
            }
        }
        Ok(true)
    }
}

#[derive(Debug)]
pub enum Command {
    Enqueue { track: Track, url: String },
    Stop,
    NextTrack,
    PrevTrack,
    Pause,
    Forward5,
    Backward5,
}

#[derive(Debug)]
pub struct PlayerState {
    playlist: Vec<Track>,
    current_position: usize,
}

impl PlayerState {
    fn new() -> Self {
        Self {
            playlist: vec![],
            current_position: 0,
        }
    }

    pub fn playlist(&self) -> impl Iterator<Item = &'_ Track> {
        self.playlist.iter()
    }

    pub fn current(&self) -> usize {
        self.current_position
    }
}

pub type State = Arc<Mutex<PlayerState>>;

pub struct Player {
    rx: mpsc::Receiver<Command>,
    state: State,
}

impl Player {
    pub fn new() -> (Self, mpsc::Sender<Command>) {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(PlayerState::new()));
        (Self { rx, state }, tx)
    }

    pub fn start_worker(self) -> (State, std::thread::JoinHandle<Result<()>>) {
        let state = self.state.clone();

        let handle = std::thread::spawn(move || {
            let mut worker = MediaWorker::new()?;
            loop {
                worker.poll_events()?;
                match self.rx.try_recv() {
                    Ok(Command::Enqueue { track, url }) => {
                        if let Err(err) = worker.loadfile(&url) {
                            log::error!("cannot load {}: {}, url: {}", track.name, err, url);
                        } else {
                            self.state.lock().unwrap().playlist.push(track);
                        }
                    }
                    Ok(Command::Stop) => {
                        if let Err(err) = worker.stop() {
                            log::error!("cannot stop the track: {}", err);
                        } else {
                            let mut state = self.state.lock().unwrap();
                            state.playlist.clear();
                            state.current_position = 0;
                        }
                    }
                    Ok(Command::NextTrack) => {
                        if let Err(err) = worker.next() {
                            log::error!("cannot switch to next track: {}", err);
                        } else {
                            self.state.lock().unwrap().current_position += 1;
                        }
                    }
                    Ok(Command::PrevTrack) => {
                        if let Err(err) = worker.prev() {
                            log::error!("cannot switch to previous track: {}", err);
                        } else {
                            self.state.lock().unwrap().current_position -= 1;
                        }
                    }
                    Ok(Command::Pause) => {
                        if let Err(err) = worker.pause() {
                            log::error!("cannot pause track: {}", err);
                        }
                    }
                    Ok(Command::Forward5) => {
                        if let Err(err) = worker.time_seek(|pos| pos + 5) {
                            log::error!("cannot seek time in forward (5 secs): {}", err);
                        }
                    }
                    Ok(Command::Backward5) => {
                        if let Err(err) = worker.time_seek(|pos| pos - 5) {
                            log::error!("cannot seek time in backward (5 secs): {}", err);
                        }
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        log::warn!("player command stream disconnected, finishing");
                        return Ok(());
                    }
                }

                if let Ok(pos) = worker.playlist_pos() {
                    let mut state = self.state.lock().unwrap();
                    state.current_position = pos;
                } // TODO: else will be triggered on empty playlist
            }
        });

        (state, handle)
    }
}
