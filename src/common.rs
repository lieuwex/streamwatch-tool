use std::{fmt::Display, path::Path};

use chrono::{DateTime, Local, TimeZone};

use once_cell::sync::Lazy;

use regex::Regex;

pub static FILE_STEM_REGEX_DATETIME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}").unwrap());
pub static FILE_STEM_REGEX_DATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap());

pub enum DateType {
    Full,
    DateOnly,
}

pub fn parse_filename(path: &Path) -> Option<(DateTime<Local>, DateType)> {
    use chrono::{NaiveDate, NaiveDateTime};

    let stem = path.file_stem().unwrap().to_str().unwrap();

    let (naive_datetime, typ) = FILE_STEM_REGEX_DATETIME
        .find(stem)
        .and_then(|m| NaiveDateTime::parse_from_str(m.as_str(), "%Y-%m-%d %H:%M:%S").ok())
        .map(|d| (d, DateType::Full))
        .or_else(|| {
            FILE_STEM_REGEX_DATE
                .find(stem)
                .and_then(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok())
                .map(|d| (d.and_hms(0, 0, 0), DateType::DateOnly))
        })?;

    Some((Local.from_local_datetime(&naive_datetime).unwrap(), typ))
}

pub struct Settings {
    pub verbose: bool,
    pub dry_run: bool,
}

impl Settings {
    pub fn print<F, S>(&self, f: F)
    where
        F: FnOnce() -> S,
        S: Display,
    {
        if self.dry_run {
            eprintln!("[DRY] {}", f());
        } else if self.verbose {
            eprintln!("{}", f());
        }
    }
}

pub async fn rename(settings: &Settings, old: &Path, new: &Path) -> std::io::Result<()> {
    settings.print(|| format!("renaming {:?} -> {:?}", old, new));

    if settings.dry_run {
        return Ok(());
    }

    tokio::fs::rename(old, new).await
}

pub async fn remove_file(settings: &Settings, path: &Path) -> std::io::Result<()> {
    settings.print(|| format!("removing {:?}", path));

    if settings.dry_run {
        return Ok(());
    }

    tokio::fs::remove_file(path).await
}
