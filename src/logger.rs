use std::fmt::Display;

use log::Level;

const MAX_TTL: usize = 4;

#[derive(Default)]
pub struct Logger {
    line: Option<String>,
    ticks_lived: usize,
}

impl Logger {
    pub fn log(&mut self, level: Level, context: &str, line: impl Display) {
        self.ticks_lived = 0;
        log::log!(level, "{}: {}", context, line);
        self.line = Some(format!("{}", line));
    }

    pub fn log_lines(&mut self) -> impl Iterator<Item = &String> {
        self.ticks_lived += 1;
        if self.ticks_lived > MAX_TTL {
            self.line.take();
        }

        self.line.as_ref().into_iter()
    }
}
