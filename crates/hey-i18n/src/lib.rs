// Copyright (c) Vitaly Golik (mjojo)

//! Localization on top of Fluent (`.ftl`) plus locale-aware number and date formatting.
#![warn(missing_docs)]
//!
//! Lookup order for a message: active locale, its language-only bundle, the fallback locale,
//! then the key itself (a missing translation is visible but never panics).

use std::collections::HashMap;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Error returned by [`I18n::add_ftl`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum I18nError {
    /// The locale tag is not a valid BCP 47 identifier.
    InvalidLocale {
        /// The offending tag.
        locale: String,
        /// Parser message.
        message: String,
    },
    /// The FTL source has syntax errors.
    Syntax {
        /// Locale the source was added to.
        locale: String,
        /// Parser diagnostics.
        message: String,
    },
    /// A message id is already defined for this locale.
    Duplicate {
        /// Locale the source was added to.
        locale: String,
        /// Diagnostics.
        message: String,
    },
}

impl std::fmt::Display for I18nError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            I18nError::InvalidLocale { locale, message } => write!(f, "invalid locale `{locale}`: {message}"),
            I18nError::Syntax { locale, message } => write!(f, "FTL syntax error in `{locale}`: {message}"),
            I18nError::Duplicate { locale, message } => write!(f, "duplicate message in `{locale}`: {message}"),
        }
    }
}

impl std::error::Error for I18nError {}

/// Message argument value.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// A string argument.
    Str(String),
    /// A numeric argument.
    Num(f64),
}

impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Arg::Str(s.to_owned())
    }
}
impl From<String> for Arg {
    fn from(s: String) -> Self {
        Arg::Str(s)
    }
}
impl From<f64> for Arg {
    fn from(n: f64) -> Self {
        Arg::Num(n)
    }
}
impl From<i64> for Arg {
    fn from(n: i64) -> Self {
        Arg::Num(n as f64)
    }
}

/// Text direction of a locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Left-to-right.
    Ltr,
    /// Right-to-left.
    Rtl,
}

/// Fluent message catalog for one active locale.
pub struct I18n {
    fallback: String,
    active: String,
    bundles: HashMap<String, FluentBundle<FluentResource>>,
}

fn lang(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

impl I18n {
    /// `fallback` is the locale that is initially active and used for missing messages.
    pub fn new(fallback: &str) -> Self {
        I18n { fallback: fallback.to_owned(), active: fallback.to_owned(), bundles: HashMap::new() }
    }

    /// Adds (or extends) the messages of `locale` from FTL source.
    pub fn add_ftl(&mut self, locale: &str, ftl: &str) -> Result<(), I18nError> {
        let id: LanguageIdentifier = locale.parse().map_err(|e: unic_langid::LanguageIdentifierError| I18nError::InvalidLocale {
            locale: locale.to_owned(),
            message: e.to_string(),
        })?;
        let res = FluentResource::try_new(ftl.to_owned())
            .map_err(|(_, errs)| I18nError::Syntax { locale: locale.to_owned(), message: format!("{errs:?}") })?;
        let bundle = self.bundles.entry(locale.to_owned()).or_insert_with(|| {
            let mut b = FluentBundle::new_concurrent(vec![id]);
            b.set_use_isolating(false); // isolation marks confuse plain-text rendering
            b
        });
        bundle.add_resource(res).map_err(|e| I18nError::Duplicate { locale: locale.to_owned(), message: format!("{e:?}") })
    }

    /// Switches the active locale. Unknown locales are accepted: lookups fall back to the default one.
    pub fn set_locale(&mut self, locale: &str) {
        self.active = locale.to_owned();
    }

    /// Active locale tag, e.g. `en-US`.
    pub fn locale(&self) -> &str {
        &self.active
    }

    /// Text direction of the active locale.
    pub fn direction(&self) -> Direction {
        direction_of(&self.active)
    }

    fn lookup(&self, locale: &str, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let b = self.bundles.get(locale)?;
        let pat = b.get_message(key)?.value()?;
        let mut errs = vec![];
        Some(b.format_pattern(pat, args, &mut errs).into_owned())
    }

    fn resolve(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let chain = [self.active.as_str(), lang(&self.active), self.fallback.as_str()];
        chain.iter().find_map(|l| self.lookup(l, key, args)).unwrap_or_else(|| key.to_owned())
    }

    /// Translates `key` without arguments.
    pub fn tr(&self, key: &str) -> String {
        self.resolve(key, None)
    }

    /// Translates `key` with named arguments (`{ $name }` in FTL).
    pub fn tr_args(&self, key: &str, args: &[(&str, Arg)]) -> String {
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            match v {
                Arg::Str(s) => fa.set(*k, FluentValue::from(s.clone())),
                Arg::Num(n) => fa.set(*k, FluentValue::from(*n)),
            }
        }
        self.resolve(key, Some(&fa))
    }

    /// Formats a number with the active locale's separators; `decimals` fixes the fraction digits.
    pub fn format_number(&self, n: f64, decimals: usize) -> String {
        format_number(&self.active, n, decimals)
    }

    /// Formats a calendar date with the active locale's field order and separator.
    pub fn format_date(&self, year: i32, month: u32, day: u32) -> String {
        format_date(&self.active, year, month, day)
    }
}

/// Right-to-left scripts by primary language subtag.
pub fn direction_of(locale: &str) -> Direction {
    match lang(locale) {
        "ar" | "he" | "fa" | "ur" | "ps" | "yi" | "dv" | "sd" | "ug" => Direction::Rtl,
        _ => Direction::Ltr,
    }
}

/// `(group separator, decimal separator)` for a locale.
fn separators(locale: &str) -> (&'static str, &'static str) {
    match lang(locale) {
        "ru" | "uk" | "pl" | "cs" | "sk" | "bg" | "fi" | "sv" | "nb" | "no" | "hu" => ("\u{a0}", ","),
        "de" | "nl" | "es" | "it" | "pt" | "tr" | "id" | "da" | "el" => (".", ","),
        "fr" => ("\u{202f}", ","),
        _ => (",", "."),
    }
}

/// Formats `n` with `decimals` fraction digits using the separators of `locale`.
pub fn format_number(locale: &str, n: f64, decimals: usize) -> String {
    if !n.is_finite() {
        return n.to_string();
    }
    let (group, dec) = separators(locale);
    let s = format!("{:.*}", decimals, n.abs());
    let (int, frac) = s.split_once('.').unwrap_or((s.as_str(), ""));
    let digits: Vec<char> = int.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(group);
        }
        out.push(*c);
    }
    if !frac.is_empty() {
        out.push_str(dec);
        out.push_str(frac);
    }
    // A value that rounds to zero is not shown as negative.
    if n < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.insert(0, '-');
    }
    out
}

/// Formats a calendar date in the conventional order of `locale`.
pub fn format_date(locale: &str, year: i32, month: u32, day: u32) -> String {
    match lang(locale) {
        "en" if locale.eq_ignore_ascii_case("en-GB") => format!("{day:02}/{month:02}/{year}"),
        "en" => format!("{month:02}/{day:02}/{year}"),
        "ru" | "uk" | "de" | "pl" | "cs" | "tr" | "fi" | "nb" | "no" => format!("{day:02}.{month:02}.{year}"),
        "fr" | "es" | "it" | "pt" | "nl" | "id" => format!("{day:02}/{month:02}/{year}"),
        _ => format!("{year}-{month:02}-{day:02}"), // ja, zh, ko, sv and unknown: ISO 8601
    }
}
