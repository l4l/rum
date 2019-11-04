use std::sync::mpsc::{self, TryRecvError};

use mpv::{MpvHandler, MpvHandlerBuilder, Result};

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
    Enqueue(String),
    Stop,
    NextTrack,
    PrevTrack,
    Pause,
    Forward5,
    Backward5,
}

pub struct Player {
    rx: mpsc::Receiver<Command>,
}

impl Player {
    pub fn new() -> (Self, mpsc::Sender<Command>) {
        let (tx, rx) = mpsc::channel();
        (Self { rx }, tx)
    }

    pub fn start_worker(self) -> std::thread::JoinHandle<Result<()>> {
        std::thread::spawn(move || {
            let mut worker = MediaWorker::new()?;
            loop {
                worker.poll_events()?;
                match self.rx.try_recv() {
                    Ok(Command::Enqueue(url)) => {
                        if let Err(err) = worker.loadfile(&url) {
                            log::error!("cannot perform loadfile: {}", err);
                        }
                    }
                    Ok(Command::Stop) => {
                        if let Err(err) = worker.stop() {
                            log::error!("cannot stop the track: {}", err);
                        }
                    }
                    Ok(Command::NextTrack) => {
                        if let Err(err) = worker.next() {
                            log::error!("cannot switch to next track: {}", err);
                        }
                    }
                    Ok(Command::PrevTrack) => {
                        if let Err(err) = worker.prev() {
                            log::error!("cannot switch to previous track: {}", err);
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
            }
        })
    }
}
