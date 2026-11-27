// Copyright (c) Vitaly Golik (mjojo)

//! Session recording: a timestamped [`UiEvent`] stream in a small line-based text format.
//!
//! ```text
//! hey-session 1
//! 0 resize 400 300
//! 16 move 10 20
//! 20 down 10 20 left
//! 80 text hello\sworld
//! 90 hash 9ae1f0c2d4b7a311
//! ```
//!
//! Each line is `<ms since start> <kind> <args>`. Strings escape `\`, space, newline, tab and
//! carriage return. `hash` lines are checkpoints: the FNV-1a hash of the rendered frame.

use crate::{Modifiers, MouseButton, Point, UiEvent};
use std::fmt::Write as _;

const HEADER: &str = "hey-session 1";

/// One recorded item.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Entry {
    /// An input event.
    Event(UiEvent),
    /// Expected frame hash at this point (see `Harness::frame_hash`).
    Hash(u64),
}

/// A recorded session: entries with timestamps in milliseconds from the start.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub entries: Vec<(u64, Entry)>,
}

/// Why a recording could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

impl Recording {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event at `ms`.
    pub fn push_event(&mut self, ms: u64, ev: UiEvent) {
        self.entries.push((ms, Entry::Event(ev)));
    }

    /// Append a frame hash checkpoint at `ms`.
    pub fn push_hash(&mut self, ms: u64, hash: u64) {
        self.entries.push((ms, Entry::Hash(hash)));
    }

    /// Serialize to the text format. Events this version cannot encode are skipped.
    pub fn to_text(&self) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        for (ms, e) in &self.entries {
            let line = match e {
                Entry::Event(ev) => encode(ev),
                Entry::Hash(h) => Some(format!("hash {h:016x}")),
            };
            if let Some(line) = line {
                let _ = writeln!(out, "{ms} {line}");
            }
        }
        out
    }

    /// Parse the text format. Blank lines and `#` comments are ignored.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut lines = text.lines().enumerate();
        match lines.next() {
            Some((_, l)) if l.trim() == HEADER => {}
            _ => return Err(ParseError { line: 1, message: format!("expected `{HEADER}`") }),
        }
        let mut rec = Self::new();
        for (i, raw) in lines {
            // Only strip the line ending: a trailing space-separated empty argument is meaningful.
            let line = raw.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let err = |m: &str| ParseError { line: i + 1, message: m.to_owned() };
            let mut it = line.split(' ');
            let ms = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| err("bad timestamp"))?;
            let kind = it.next().ok_or_else(|| err("missing kind"))?;
            let args: Vec<&str> = it.collect();
            let entry = decode(kind, &args).ok_or_else(|| err(&format!("bad `{kind}` line")))?;
            rec.entries.push((ms, entry));
        }
        Ok(rec)
    }

    /// Write to a file.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }

    /// Read from a file.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::parse(&std::fs::read_to_string(path)?)?)
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            ' ' => o.push_str("\\s"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

fn unesc(s: &str) -> Option<String> {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        o.push(match it.next()? {
            '\\' => '\\',
            's' => ' ',
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            _ => return None,
        });
    }
    Some(o)
}

fn button(b: MouseButton) -> &'static str {
    match b {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
    }
}

fn parse_button(s: &str) -> Option<MouseButton> {
    Some(match s {
        "left" => MouseButton::Left,
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => return None,
    })
}

/// Modifiers as letters (`s`hift, `c`trl, `a`lt, `l`ogo) or `-` for none.
fn mods_str(m: Modifiers) -> String {
    let mut s = String::new();
    for (on, c) in [(m.shift, 's'), (m.ctrl, 'c'), (m.alt, 'a'), (m.logo, 'l')] {
        if on {
            s.push(c);
        }
    }
    if s.is_empty() {
        s.push('-');
    }
    s
}

fn parse_mods(s: &str) -> Option<Modifiers> {
    let mut m = Modifiers::default();
    if s != "-" {
        for c in s.chars() {
            match c {
                's' => m.shift = true,
                'c' => m.ctrl = true,
                'a' => m.alt = true,
                'l' => m.logo = true,
                _ => return None,
            }
        }
    }
    Some(m)
}

fn encode(ev: &UiEvent) -> Option<String> {
    Some(match ev {
        UiEvent::PointerMove { pos } => format!("move {} {}", pos.x, pos.y),
        UiEvent::PointerDown { pos, button: b } => format!("down {} {} {}", pos.x, pos.y, button(*b)),
        UiEvent::PointerUp { pos, button: b } => format!("up {} {} {}", pos.x, pos.y, button(*b)),
        UiEvent::PointerLeave => "leave".into(),
        UiEvent::Wheel { pos, dx, dy } => format!("wheel {} {} {dx} {dy}", pos.x, pos.y),
        UiEvent::KeyDown { code, mods } => format!("keydown {} {}", esc(code), mods_str(*mods)),
        UiEvent::KeyUp { code, mods } => format!("keyup {} {}", esc(code), mods_str(*mods)),
        UiEvent::Text(s) => format!("text {}", esc(s)),
        UiEvent::Preedit(s) => format!("preedit {}", esc(s)),
        UiEvent::FocusChanged(b) => format!("focus {}", u8::from(*b)),
        UiEvent::Resize { width, height } => format!("resize {width} {height}"),
        UiEvent::ScaleFactor(s) => format!("scale {s}"),
        UiEvent::FileHovered(p) => format!("filehover {}", esc(&p.to_string_lossy())),
        UiEvent::FileDropped(p) => format!("filedrop {}", esc(&p.to_string_lossy())),
        UiEvent::FileHoverCancelled => "filecancel".into(),
    })
}

fn decode(kind: &str, a: &[&str]) -> Option<Entry> {
    let f = |i: usize| -> Option<f32> { a.get(i)?.parse().ok() };
    let pt = || Some(Point::new(f(0)?, f(1)?));
    let s = |i: usize| -> Option<String> { unesc(a.get(i)?) };
    let ev = match (kind, a.len()) {
        ("hash", 1) => return Some(Entry::Hash(u64::from_str_radix(a[0], 16).ok()?)),
        ("move", 2) => UiEvent::PointerMove { pos: pt()? },
        ("down", 3) => UiEvent::PointerDown { pos: pt()?, button: parse_button(a[2])? },
        ("up", 3) => UiEvent::PointerUp { pos: pt()?, button: parse_button(a[2])? },
        ("leave", 0) => UiEvent::PointerLeave,
        ("wheel", 4) => UiEvent::Wheel { pos: pt()?, dx: f(2)?, dy: f(3)? },
        ("keydown", 2) => UiEvent::KeyDown { code: s(0)?, mods: parse_mods(a[1])? },
        ("keyup", 2) => UiEvent::KeyUp { code: s(0)?, mods: parse_mods(a[1])? },
        ("text", 1) => UiEvent::Text(s(0)?),
        ("preedit", 1) => UiEvent::Preedit(s(0)?),
        // A lone empty preedit/text serializes to an empty argument, which `split` keeps.
        ("focus", 1) => UiEvent::FocusChanged(a[0] == "1"),
        ("resize", 2) => UiEvent::Resize { width: a[0].parse().ok()?, height: a[1].parse().ok()? },
        ("scale", 1) => UiEvent::ScaleFactor(a[0].parse().ok()?),
        ("filehover", 1) => UiEvent::FileHovered(s(0)?.into()),
        ("filedrop", 1) => UiEvent::FileDropped(s(0)?.into()),
        ("filecancel", 0) => UiEvent::FileHoverCancelled,
        _ => return None,
    };
    Some(Entry::Event(ev))
}

/// FNV-1a over raw bytes: stable across platforms and Rust versions (unlike `DefaultHasher`).
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        let mut r = Recording::new();
        r.push_event(0, UiEvent::Resize { width: 400, height: 300 });
        r.push_event(16, UiEvent::PointerMove { pos: Point::new(10.5, 20.25) });
        r.push_event(20, UiEvent::PointerDown { pos: Point::new(10.5, 20.25), button: MouseButton::Right });
        r.push_event(30, UiEvent::Text("hi there\n\\ ok".into()));
        r.push_event(31, UiEvent::Text(String::new()));
        r.push_event(40, UiEvent::KeyDown { code: "a".into(), mods: Modifiers { ctrl: true, shift: true, ..Default::default() } });
        r.push_event(50, UiEvent::ScaleFactor(1.25));
        r.push_event(60, UiEvent::FocusChanged(false));
        r.push_hash(70, 0xdead_beef_0123_4567);
        let back = Recording::parse(&r.to_text()).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Recording::parse("nope").is_err());
        let e = Recording::parse("hey-session 1\n0 move x y\n").unwrap_err();
        assert_eq!(e.line, 2);
    }
}
