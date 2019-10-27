use std::io::{Error, ErrorKind};
use std::marker::Unpin;

use termion::event::{Event, Key, MouseButton, MouseEvent};
use tokio::io::AsyncReadExt;
use tokio::prelude::*;

// This file contains tty event handling rewritten in async
// src: https://github.com/redox-os/termion/blob/master/src/event.rs

async fn fetch_byte(rdr: &mut (impl AsyncRead + Unpin)) -> Result<u8, Error> {
    let mut buf = [0u8];
    rdr.read_exact(&mut buf[..]).await?;
    Ok(buf[0])
}

async fn parse_csi(mut rdr: &mut (impl AsyncRead + Unpin)) -> Option<Event> {
    let ev = match fetch_byte(&mut rdr).await.ok()? {
        b'[' => {
            let val = fetch_byte(&mut rdr).await.ok()?;
            Event::Key(Key::F(1 + val - b'A'))
        }
        b'D' => Event::Key(Key::Left),
        b'C' => Event::Key(Key::Right),
        b'A' => Event::Key(Key::Up),
        b'B' => Event::Key(Key::Down),
        b'H' => Event::Key(Key::Home),
        b'F' => Event::Key(Key::End),
        b'M' => {
            // X10 emulation mouse encoding: ESC [ CB Cx Cy (6 characters only).
            let cb = fetch_byte(&mut rdr).await.ok()? as i8 - 32;
            // (1, 1) are the coords for upper left.
            let cx = fetch_byte(&mut rdr).await.ok()?.saturating_sub(32) as u16;
            let cy = fetch_byte(&mut rdr).await.ok()?.saturating_sub(32) as u16;
            Event::Mouse(match cb & 0b11 {
                0 => {
                    if cb & 0x40 != 0 {
                        MouseEvent::Press(MouseButton::WheelUp, cx, cy)
                    } else {
                        MouseEvent::Press(MouseButton::Left, cx, cy)
                    }
                }
                1 => {
                    if cb & 0x40 != 0 {
                        MouseEvent::Press(MouseButton::WheelDown, cx, cy)
                    } else {
                        MouseEvent::Press(MouseButton::Middle, cx, cy)
                    }
                }
                2 => MouseEvent::Press(MouseButton::Right, cx, cy),
                3 => MouseEvent::Release(cx, cy),
                _ => return None,
            })
        }
        b'<' => {
            // xterm mouse encoding:
            // ESC [ < Cb ; Cx ; Cy (;) (M or m)
            let mut buf = Vec::new();
            let mut c = fetch_byte(&mut rdr).await.unwrap();
            while match c {
                b'm' | b'M' => false,
                _ => true,
            } {
                buf.push(c);
                c = fetch_byte(&mut rdr).await.unwrap();
            }
            let str_buf = String::from_utf8(buf).unwrap();
            let nums = &mut str_buf.split(';');

            let cb = nums.next().unwrap().parse::<u16>().unwrap();
            let cx = nums.next().unwrap().parse::<u16>().unwrap();
            let cy = nums.next().unwrap().parse::<u16>().unwrap();

            let event = match cb {
                0..=2 | 64..=65 => {
                    let button = match cb {
                        0 => MouseButton::Left,
                        1 => MouseButton::Middle,
                        2 => MouseButton::Right,
                        64 => MouseButton::WheelUp,
                        65 => MouseButton::WheelDown,
                        _ => unreachable!(),
                    };
                    match c {
                        b'M' => MouseEvent::Press(button, cx, cy),
                        b'm' => MouseEvent::Release(cx, cy),
                        _ => return None,
                    }
                }
                32 => MouseEvent::Hold(cx, cy),
                3 => MouseEvent::Release(cx, cy),
                _ => return None,
            };

            Event::Mouse(event)
        }
        c @ b'0'..=b'9' => {
            // Numbered escape code.
            let mut buf = Vec::new();
            buf.push(c);
            let mut c = fetch_byte(&mut rdr).await.unwrap();
            // The final byte of a CSI sequence can be in the range 64-126, so
            // let's keep reading anything else.
            while c < 64 || c > 126 {
                buf.push(c);
                c = fetch_byte(&mut rdr).await.unwrap();
            }

            match c {
                // rxvt mouse encoding:
                // ESC [ Cb ; Cx ; Cy ; M
                b'M' => {
                    let str_buf = String::from_utf8(buf).unwrap();

                    let nums: Vec<u16> = str_buf.split(';').map(|n| n.parse().unwrap()).collect();

                    let cb = nums[0];
                    let cx = nums[1];
                    let cy = nums[2];

                    let event = match cb {
                        32 => MouseEvent::Press(MouseButton::Left, cx, cy),
                        33 => MouseEvent::Press(MouseButton::Middle, cx, cy),
                        34 => MouseEvent::Press(MouseButton::Right, cx, cy),
                        35 => MouseEvent::Release(cx, cy),
                        64 => MouseEvent::Hold(cx, cy),
                        96 | 97 => MouseEvent::Press(MouseButton::WheelUp, cx, cy),
                        _ => return None,
                    };

                    Event::Mouse(event)
                }
                // Special key code.
                b'~' => {
                    let str_buf = String::from_utf8(buf).unwrap();

                    // This CSI sequence can be a list of semicolon-separated
                    // numbers.
                    let nums: Vec<u8> = str_buf.split(';').map(|n| n.parse().unwrap()).collect();

                    if nums.is_empty() {
                        return None;
                    }

                    // TODO: handle multiple values for key modififiers (ex: values
                    // [3, 2] means Shift+Delete)
                    if nums.len() > 1 {
                        return None;
                    }

                    match nums[0] {
                        1 | 7 => Event::Key(Key::Home),
                        2 => Event::Key(Key::Insert),
                        3 => Event::Key(Key::Delete),
                        4 | 8 => Event::Key(Key::End),
                        5 => Event::Key(Key::PageUp),
                        6 => Event::Key(Key::PageDown),
                        v @ 11..=15 => Event::Key(Key::F(v - 10)),
                        v @ 17..=21 => Event::Key(Key::F(v - 11)),
                        v @ 23..=24 => Event::Key(Key::F(v - 12)),
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(ev)
}

/// Parse `c` as either a single byte ASCII char or a variable size UTF-8 char.
async fn parse_utf8_char(c: u8, mut rdr: &mut (impl AsyncRead + Unpin)) -> Result<char, Error> {
    if c.is_ascii() {
        return Ok(c as char);
    }
    let mut buf = Vec::with_capacity(5);
    buf.push(c);

    loop {
        buf.push(fetch_byte(&mut rdr).await?);
        match std::str::from_utf8(&buf) {
            Ok(st) => return Ok(st.chars().next().unwrap()),
            Err(err) if buf.len() >= 4 => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Input character is not valid UTF-8: {}", err),
                ));
            }
            _ => {}
        }
    }
}

pub async fn parse_event(mut rdr: &mut (impl AsyncRead + Unpin)) -> Result<Event, Error> {
    let item = match fetch_byte(&mut rdr).await {
        Ok(item) => item,
        Err(err) => return Err(err),
    };
    match item {
        b'\x1B' => {
            // This is an escape character, leading a control sequence.
            let c = match fetch_byte(&mut rdr).await? {
                b'O' => {
                    match fetch_byte(&mut rdr).await? {
                        // F1-F4
                        val @ b'P'..=b'S' => Event::Key(Key::F(1 + val - b'P')),
                        _ => {
                            return Err(Error::new(
                                ErrorKind::Other,
                                "Could not parse a function key event",
                            ))
                        }
                    }
                }
                b'[' => {
                    // This is a CSI sequence.
                    parse_csi(&mut rdr).await.ok_or_else(|| {
                        Error::new(ErrorKind::Other, "Could not parse a csi sequence key event")
                    })?
                }
                c => Event::Key(Key::Alt(parse_utf8_char(c, rdr).await?)),
            };
            Ok(c)
        }
        b'\n' | b'\r' => Ok(Event::Key(Key::Char('\n'))),
        b'\t' => Ok(Event::Key(Key::Char('\t'))),
        b'\x7F' => Ok(Event::Key(Key::Backspace)),
        c @ b'\x01'..=b'\x19' => Ok(Event::Key(Key::Ctrl((c as u8 - 0x1 + b'a') as char))),
        c @ b'\x1C'..=b'\x1F' => Ok(Event::Key(Key::Ctrl((c as u8 - 0x1C + b'4') as char))),
        b'\0' => Ok(Event::Key(Key::Null)),
        c => Ok({ Event::Key(Key::Char(parse_utf8_char(c, rdr).await?)) }),
    }
}

pub async fn events_stream(
    rdr: impl AsyncRead + Unpin,
) -> impl Stream<Item = Result<Event, Error>> {
    tokio::stream::unfold(rdr, |mut rdr| {
        async move {
            match parse_event(&mut rdr).await {
                Ok(event) => Some((Ok(event), rdr)),
                Err(err) if err.kind() == ErrorKind::UnexpectedEof => None,
                Err(err) => Some((Err(err), rdr)),
            }
        }
    })
}
