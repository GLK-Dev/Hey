//! Typed errors of the application shell.

use std::fmt;
use std::path::PathBuf;

/// Error returned by [`App::build`](crate::App::build), [`App::run`](crate::App::run) and [`run`](crate::run).
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A font could not be loaded. `path` is `None` for the bundled default font.
    Font {
        /// File the font was read from, if any.
        path: Option<PathBuf>,
        /// Human-readable reason.
        message: String,
    },
    /// A required builder call (`update` or `view`) was not made.
    MissingCallback(&'static str),
    /// The operating system event loop or window could not be created or failed while running.
    EventLoop(String),
    /// The GPU renderer was requested but could not be created.
    Gpu(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Font { path: Some(p), message } => write!(f, "font {}: {message}", p.display()),
            Error::Font { path: None, message } => write!(f, "default font: {message}"),
            Error::MissingCallback(n) => write!(f, "App: {n} is not set"),
            Error::EventLoop(m) => write!(f, "event loop: {m}"),
            Error::Gpu(m) => write!(f, "GPU renderer: {m}"),
        }
    }
}

impl std::error::Error for Error {}
