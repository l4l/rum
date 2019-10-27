# RUM Player

[![Build Status](https://travis-ci.org/l4l/rum.svg?branch=master)](https://travis-ci.org/l4l/rum)

RUM is a terminal music player, that able to play remote media from different sources (currently only Ya.Music).

# Usage

Playing media is performed via _mpv_ player, thus it need to be accessible.

```bash
cargo install rum-player
# by default installed at ~/.cargo/bin, you may add it to path:
export PATH=$PATH:~/.cargo/bin
rum-player
```

Currently, the tool has 2 main windows: album search and list album tracks.
Hotkeys are currently cannot be configured and are the following:

- Arrow Up/Down - scroll up/down displayed list;
- Backspace (at track list view) - display back to album search panel;
- Ctrl+p - continue/pause player;
- Delete - quits the program.

# Development

For development you need a nightly compiler, since dependency requires it: `rustup default toolchain nightly`. Afterwards you may build sources via `cargo build` and start hacking. Please also use rustfmt & clippy at development process: `rustup component add rustfmt clippy`.
