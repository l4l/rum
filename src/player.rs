use std::sync::mpsc::{self, TryRecvError};

use mpv::{MpvHandler, MpvHandlerBuilder, Result};

struct MediaWorker {
    handler: MpvHandler,
}

impl MediaWorker {
    pub fn new() -> Result<Self> {
        let handler = MpvHandlerBuilder::new()?.build()?;
        Ok(Self { handler })
    }

    #[cold]
    pub fn loadfile(&mut self, url: &str) -> Result<()> {
        self.handler.command(&["loadfile", &url])?;
        Ok(())
    }

    pub fn poll_events(&mut self) -> Result<bool> {
        while let Some(ev) = self.handler.wait_event(0.0) {
            match ev {
                mpv::Event::Shutdown | mpv::Event::Idle => {
                    return Ok(false);
                }
                _ => {}
            }
        }
        Ok(true)
    }
}

pub enum Command {
    Play(String),
    ContinueOrStop,
}

pub struct Player {
    rx: mpsc::Receiver<Command>,
    should_poll: bool,
}

impl Player {
    pub fn new() -> (Self, mpsc::Sender<Command>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                rx,
                should_poll: true,
            },
            tx,
        )
    }

    pub fn start_worker(mut self) -> std::thread::JoinHandle<Result<()>> {
        std::thread::spawn(move || {
            let mut worker = MediaWorker::new()?;
            loop {
                if self.should_poll {
                    worker.poll_events()?;
                }
                match self.rx.try_recv() {
                    Ok(Command::Play(url)) => {
                        let _ = worker.loadfile(&url);
                    }
                    Ok(Command::ContinueOrStop) => {
                        self.should_poll ^= true;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
        })
    }
}
