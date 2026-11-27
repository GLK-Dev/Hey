// Copyright (c) Vitaly Golik (mjojo)

//! `hey-style`: check a design-token file and export it to CSS or Figma (DTCG) variables.
//!
//! ```text
//! hey-style check [themes.json] [--against other.json]
//! hey-style export css|figma [themes.json]
//! ```
//!
//! Without a path the built-in `themes.json` is used. Exit code: 0 ok, 1 contract broken, 2 usage.

use hey_style::{export_css, export_figma, validate_tokens};
use std::process::ExitCode;

const BUILTIN: &str = include_str!("../assets/themes.json");

const USAGE: &str = "usage:\n  hey-style check [themes.json] [--against other.json]\n  hey-style export css|figma [themes.json]";

fn load(path: Option<&str>) -> Result<String, String> {
    match path {
        None => Ok(BUILTIN.to_string()),
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}")),
    }
}

fn run(args: &[String]) -> Result<bool, String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["check", rest @ ..] => {
            let mut path = None;
            let mut against = None;
            let mut it = rest.iter();
            while let Some(&x) = it.next() {
                if x == "--against" {
                    against = Some(*it.next().ok_or(USAGE)?);
                } else if path.is_none() {
                    path = Some(x);
                } else {
                    return Err(USAGE.into());
                }
            }
            let json = load(path)?;
            let mut ok = true;
            for i in validate_tokens(&json) {
                eprintln!("error: {i}");
                ok = false;
            }
            if let Some(other) = against {
                if load(Some(other))? != json {
                    eprintln!("error: {} differs from {}", other, path.unwrap_or("the built-in themes.json"));
                    ok = false;
                }
            }
            if ok {
                println!("tokens ok");
            }
            Ok(ok)
        }
        ["export", kind @ ("css" | "figma"), rest @ ..] if rest.len() <= 1 => {
            let json = load(rest.first().copied())?;
            let out = if *kind == "css" { export_css(&json)? } else { export_figma(&json)? };
            println!("{out}");
            Ok(true)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
