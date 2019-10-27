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
- Left/Right - switch to previous/next track;
- Backspace (at track list view) - display back to album search panel;
- Ctrl+a (at track list view) - add all tracks to playlist;
- Ctrl+r - redraw screen (might be helpful after resizing);
- Ctrl+s - stop playback and clear playlist;
- Enter - select item at list view (at track list view append to the end of playlist, rather than replacing it);
- Delete - quit the program.

# Development

For development you need a nightly compiler, since dependency requires it: `rustup default toolchain nightly`. Afterwards you may build sources via `cargo build` and start hacking. Please also use rustfmt & clippy at development process: `rustup component add rustfmt clippy`.
