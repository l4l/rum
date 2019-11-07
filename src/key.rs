use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::prelude::*;
use itertools::Itertools;
use termion::event::{Event, Key};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Context {
    is_search: bool,
    is_tracklist: bool,
    is_playlist: bool,
}

impl Context {
    fn is_valid(self) -> bool {
        self.is_search | self.is_tracklist | self.is_playlist
    }

    fn is_sub(self, other: Context) -> bool {
        macro_rules! implies {
            ($a:expr => $b:expr) => {{
                !$a || $b
            }};
        }

        implies!(self.is_search => other.is_search)
            && implies!(self.is_tracklist => other.is_tracklist)
            && implies!(self.is_playlist => other.is_playlist)
    }

    pub fn search() -> Self {
        Context {
            is_search: true,
            is_tracklist: false,
            is_playlist: false,
        }
    }

    pub fn tracklist() -> Self {
        Context {
            is_search: false,
            is_tracklist: true,
            is_playlist: false,
        }
    }

    pub fn playlist() -> Self {
        Context {
            is_search: false,
            is_tracklist: false,
            is_playlist: true,
        }
    }

    pub fn all() -> Self {
        Context {
            is_search: true,
            is_tracklist: true,
            is_playlist: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Quit,
    PointerUp,
    PointerDown,
    NextTrack,
    PrevTrack,
    Pause,
    Stop,
    Forward5,
    Backward5,
    Refresh,
    AddAll,
    ShowPlaylist,
    SwitchToAlbums,
    SwitchToTracks,
    SwitchToArtists,
    Enter,
    SwitchView,
    #[serde(skip)]
    Char(char),
    Backspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextedAction {
    pub context: Context,
    pub action: Action,
}

#[derive(Default, Debug)]
pub struct BindingConfig {
    bindings: HashMap<Event, Vec<ContextedAction>>,
}

impl From<HashMap<Event, Vec<ContextedAction>>> for BindingConfig {
    fn from(event_actions: HashMap<Event, Vec<ContextedAction>>) -> Self {
        Self {
            bindings: event_actions
                .into_iter()
                .filter_map(|(key, mut actions)| {
                    actions.sort_by_key(|v| v.context);
                    let actions: Vec<_> = actions
                        .into_iter()
                        .filter(|action| action.context.is_valid())
                        .dedup()
                        .collect();
                    if actions.is_empty() {
                        None
                    } else {
                        Some((key, actions))
                    }
                })
                .collect(),
        }
    }
}

impl BindingConfig {
    fn action(&self, context: Context, event: &Event) -> Option<Action> {
        self.bindings
            .get(event)
            .and_then(|actions| {
                actions
                    .iter()
                    .find(|contexed| context.is_sub(contexed.context))
                    .map(|contexed| contexed.action)
            })
            .or_else(|| BindingConfig::default_action(&event))
    }

    // TODO: use context here
    fn default_action(event: &Event) -> Option<Action> {
        match event {
            Event::Key(Key::Up) => Some(Action::PointerUp),
            Event::Key(Key::Down) => Some(Action::PointerDown),
            Event::Key(Key::Right) => Some(Action::NextTrack),
            Event::Key(Key::Left) => Some(Action::PrevTrack),
            Event::Key(Key::Delete) | Event::Key(Key::Ctrl('c')) => Some(Action::Quit),
            Event::Key(Key::Ctrl('p')) => Some(Action::Pause),
            Event::Key(Key::Char(']')) => Some(Action::Forward5),
            Event::Key(Key::Char('[')) => Some(Action::Backward5),
            Event::Key(Key::Ctrl('r')) => Some(Action::Refresh),
            Event::Key(Key::Ctrl('s')) => Some(Action::Stop),
            Event::Key(Key::Ctrl('a')) => Some(Action::AddAll),
            Event::Key(Key::Alt('p')) => Some(Action::ShowPlaylist),
            Event::Key(Key::Alt('a')) => Some(Action::SwitchToAlbums),
            Event::Key(Key::Alt('t')) => Some(Action::SwitchToTracks),
            Event::Key(Key::Alt('s')) => Some(Action::SwitchToArtists),
            Event::Key(Key::Char('\n')) => Some(Action::Enter),
            Event::Key(Key::Char('\t')) => Some(Action::SwitchView),
            Event::Key(Key::Char(c)) => Some(Action::Char(*c)),
            Event::Key(Key::Backspace) => Some(Action::Backspace),
            _ => None,
        }
    }

    pub fn actions(self) -> (mpsc::UnboundedReceiver<Action>, Arc<Mutex<Context>>) {
        let (mut action_tx, action_rx) = mpsc::unbounded_channel();
        let context = Arc::new(Mutex::new(Context::search()));

        let current_context = context.clone();

        tokio::spawn(async move {
            let mut stdin = tokio::io::stdin();
            let stream = crate::input::events_stream(&mut stdin);
            let mut stream = Box::pin(stream);
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) => {
                        let current_context = *current_context.lock().unwrap();
                        if let Some(action) = self.action(current_context, &event) {
                            if let Err(err) = action_tx.send(action).await {
                                log::warn!("events ended due to closed rx channel {}", err);
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("stdint event stream issue: {}", err);
                    }
                };
            }
        });
        (action_rx, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BinaryHeap;

    use quickcheck::{Arbitrary, Gen, TestResult};
    use quickcheck_macros::quickcheck;

    impl From<u8> for Context {
        fn from(bits: u8) -> Self {
            Self {
                is_search: bits & 0b001 > 0,
                is_tracklist: bits & 0b010 > 0,
                is_playlist: bits & 0b100 > 0,
            }
        }
    }

    #[test]
    fn test_context_order() {
        let contexts = (1u8..=7)
            .map(Context::from)
            .collect::<BinaryHeap<_>>()
            .into_sorted_vec();
        assert_eq!(
            contexts[0],
            Context {
                is_search: false,
                is_tracklist: false,
                is_playlist: true,
            }
        );
        assert_eq!(
            *contexts.last().unwrap(),
            Context {
                is_search: true,
                is_tracklist: true,
                is_playlist: true,
            }
        );
    }

    impl Arbitrary for Context {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            let mut val = [0];
            loop {
                g.fill_bytes(&mut val);
                let this: Context = val[0].into();
                if this.is_valid() {
                    return this;
                }
            }
        }
    }

    #[quickcheck]
    fn test_context_search(search: Context, contexts: Vec<Context>) -> TestResult {
        if !contexts.iter().any(|x| x == &search) {
            return TestResult::discard();
        }

        let event = Event::Key(Key::Up);
        let contexts = contexts
            .into_iter()
            .map(|context| {
                if context == search {
                    ContextedAction {
                        context,
                        action: Action::Enter,
                    }
                } else {
                    ContextedAction {
                        context,
                        action: Action::Quit,
                    }
                }
            })
            .collect::<Vec<_>>();
        let config: BindingConfig = vec![(event.clone(), contexts)]
            .into_iter()
            .collect::<HashMap<_, _>>()
            .into();

        if let Some(found) = config.action(search, &event) {
            TestResult::from_bool(found == Action::Enter)
        } else {
            TestResult::error("item not found")
        }
    }
}
