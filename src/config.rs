use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use snafu::ResultExt;
use termion::event::{Event as InnerEvent, Key};

use crate::key::BindingConfig;
use crate::key::{Action, Context, ContextedAction};

struct Event(InnerEvent);

#[derive(Debug)]
pub struct UnknownEvent;

impl fmt::Display for UnknownEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnknownEvent")
    }
}

impl std::error::Error for UnknownEvent {}

impl FromStr for Event {
    type Err = UnknownEvent;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ArrowUp" => Ok(Event(InnerEvent::Key(Key::Up))),
            "ArrowDown" => Ok(Event(InnerEvent::Key(Key::Down))),
            "ArrowRight" => Ok(Event(InnerEvent::Key(Key::Right))),
            "ArrowLeft" => Ok(Event(InnerEvent::Key(Key::Left))),
            "Del" => Ok(Event(InnerEvent::Key(Key::Delete))),
            "Backspace" => Ok(Event(InnerEvent::Key(Key::Backspace))),
            "Home" => Ok(Event(InnerEvent::Key(Key::Home))),
            "End" => Ok(Event(InnerEvent::Key(Key::End))),
            "PageUp" => Ok(Event(InnerEvent::Key(Key::PageUp))),
            "PageDown" => Ok(Event(InnerEvent::Key(Key::PageDown))),
            "Insert" => Ok(Event(InnerEvent::Key(Key::Insert))),
            "Esc" => Ok(Event(InnerEvent::Key(Key::Esc))),
            s => {
                const CTRL_PREFIX: &str = "Ctrl+";
                const ALT_PREFIX: &str = "Alt+";
                const FN_PREFIX: &str = "Fn+";

                fn parse_prefixed(
                    haystack: &str,
                    prefix: &str,
                ) -> Option<Result<Event, UnknownEvent>> {
                    if !haystack.starts_with(prefix) {
                        return None;
                    }

                    let suffix = haystack.split_at(prefix.as_bytes().len()).1;

                    if suffix.len() != 1 {
                        Some(Err(UnknownEvent))
                    } else {
                        let c = suffix.chars().next().unwrap();
                        Some(Ok(Event(InnerEvent::Key(Key::Char(c)))))
                    }
                }

                if let Some(ev) = parse_prefixed(s, CTRL_PREFIX) {
                    ev
                } else if let Some(ev) = parse_prefixed(s, ALT_PREFIX) {
                    ev
                } else if let Some(ev) = parse_prefixed(s, FN_PREFIX) {
                    ev
                } else if s.chars().count() == 1 {
                    let c = s.chars().next().unwrap();
                    Ok(Event(InnerEvent::Key(Key::Char(c))))
                } else {
                    Err(UnknownEvent)
                }
            }
        }
    }
}

#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("incorrect toml config: {}", source))]
    IncorrectToml { source: toml::de::Error },
    #[snafu(display("incorrect action value: {}", value))]
    IncorrectAction {
        value: String,
        source: toml::de::Error,
    },
    #[snafu(display("incorrect event value: {}", value))]
    IncorrectEvent { value: String, source: UnknownEvent },
    #[snafu(display("unsupported config key {}", key))]
    UnsupportedKey { key: String },
    #[snafu(display("unsupported toml item"))]
    UnsupportedTomlItem,
}

#[derive(Default, Debug)]
pub struct Config {
    pub binding: BindingConfig,
}

const HOTKEY_TABLE: &str = "hotkey";

macro_rules! try_toml {
    ($val:expr; $t:ident) => {{
        if let toml::Value::$t(value) = $val {
            value
        } else {
            return Err(Error::UnsupportedTomlItem);
        }
    }};
}

impl FromStr for Config {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut config = Config {
            binding: BindingConfig::default(),
        };

        for (key, value) in try_toml!(s.parse().context(IncorrectToml {})?; Table).into_iter() {
            match key.as_str() {
                HOTKEY_TABLE => {
                    let value = try_toml!(value; Table);
                    config.binding = parse_binding_config(value)?;
                }
                _ => return Err(Error::UnsupportedKey { key }),
            }
        }

        Ok(config)
    }
}

fn parse_binding_config(table: toml::value::Table) -> Result<BindingConfig, Error> {
    const SEARCH_TABLE: &str = "search";
    const TRACKLIST_TABLE: &str = "tracklist";
    const PLAYLIST_TABLE: &str = "playlist";

    let mut event_actions: HashMap<_, Vec<_>> = HashMap::new();
    for (key, value) in table.into_iter() {
        let (context, map): (Context, toml::map::Map<_, _>) = match key.as_str() {
            SEARCH_TABLE => {
                let map = try_toml!(value; Table);
                (Context::search(), map)
            }
            TRACKLIST_TABLE => {
                let map = try_toml!(value; Table);
                (Context::tracklist(), map)
            }
            PLAYLIST_TABLE => {
                let map = try_toml!(value; Table);
                (Context::playlist(), map)
            }
            _ => {
                let mut map = toml::map::Map::new();
                map.insert(key, value);
                (Context::all(), map)
            }
        };

        for (key, value) in map {
            let action: Action = toml::Value::String(key.clone())
                .try_into()
                .context(IncorrectAction { value: key })?;
            let action = ContextedAction { action, context };

            let value: String = try_toml!(value; String);
            let event = value
                .as_str()
                .parse::<Event>()
                .context(IncorrectEvent { value })?
                .0;

            event_actions.entry(event).or_default().push(action);
        }
    }

    Ok(event_actions.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_toml() {
        let sample_toml = r#"
[hotkey]
"PointerUp" = "ArrowUp"
"PointerDown" = "ArrowDown"
"NextTrack" = "+"
"PrevTrack" = "-"
"Forward5" = "Ctrl++"
"Backward5" = "Ctrl+-"

[hotkey.search]
"PointerUp" = "ArrowDown"
"PointerDown" = "ArrowUp"

[hotkey.tracklist]
"Select" = "Alt+0"
"#
        .to_string();

        let config = sample_toml.parse::<Config>().unwrap();
        println!("{:?}", config);
    }
}
