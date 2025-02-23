// src/logging.rs
use anyhow::{Context, Result};
use chrono::Local;
use lazy_static::lazy_static;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

lazy_static! {
    static ref LOG_FILE: Mutex<Option<File>> = Mutex::new(None);
}

fn get_log_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find HOME directory");
    home.join(".config").join("lazylode").join("logs")
}

pub fn init_logger() -> Result<()> {
    let log_dir = get_log_dir();
    std::fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let log_file_path = log_dir.join(format!("lazylode_{}.log", timestamp));

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_file_path)
        .context("Failed to create log file")?;

    *LOG_FILE.lock().unwrap() = Some(file);
    Ok(())
}

pub fn log(level: LogLevel, message: &str) -> Result<()> {
    if let Some(file) = &mut *LOG_FILE.lock().unwrap() {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] {} - {}\n", timestamp, level.as_str(), message);
        file.write_all(log_entry.as_bytes())?;
        file.flush()?;
    }
    Ok(())
}

pub fn debug(message: &str) -> Result<()> {
    log(LogLevel::Debug, message)
}

pub fn info(message: &str) -> Result<()> {
    log(LogLevel::Info, message)
}

pub fn warn(message: &str) -> Result<()> {
    log(LogLevel::Warning, message)
}

pub fn error(message: &str) -> Result<()> {
    log(LogLevel::Error, message)
}
