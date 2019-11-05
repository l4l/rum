# RUM Player

[![Build Status](https://travis-ci.org/l4l/rum.svg?branch=master)](https://travis-ci.org/l4l/rum)
[![Crates.io](https://img.shields.io/crates/v/rum-player.svg)](https://crates.io/crates/rum-player)

RUM is a terminal music player, that able to play remote media from different sources (currently only Ya.Music).

# Usage

Playing media is performed via _mpv_ player, thus it need to be accessible.

```bash
cargo install rum-player
# by default installed at ~/.cargo/bin, you may add it to path:
export PATH=$PATH:~/.cargo/bin
rum-player
```

Currently, the tool has 2 main views: search panel and tracks listing.
Hotkeys are currently cannot be configured and are the following:

- Arrow Up/Down - scroll up/down displayed list;
- Arrow Left/Right - switch to previous/next track;
- Backspace (at track list view) - display back to search panel;
- Tab - switch between search types (currently track & album search are available);
- Ctrl+a (at track list view) - add all tracks to playlist;
- Ctrl+s - stop playback and clear playlist;
- Ctrl+p - pause/unpause playback;
- Alt+a (at artist search) - switch to artist albums;
- Alt+t (at artist search) - switch to artist tracks;
- Alt+s - switch to related artist(s);
- Alt+p - switch to playlist view;
- ] - skip 5 seconds forward of currently played track;
- [ - skip 5 seconds backward of currently played track;
- Enter - select item at list view (at track list view append to the end of playlist, rather than replacing it);
- Ctrl+c/Delete - quit the program.

# Development

For development you need a nightly compiler, since dependency requires it: `rustup default toolchain nightly`. Afterwards you may build sources via `cargo build` and start hacking. Please also use rustfmt & clippy at development process: `rustup component add rustfmt clippy`.
