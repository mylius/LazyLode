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
    if let Some(home) = dirs::home_dir() {
        return home.join(".config").join("lazylode").join("logs");
    }
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    base.join(".config").join("lazylode").join("logs")
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

    let mut guard = LOG_FILE
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock log file mutex"))?;
    *guard = Some(file);
    Ok(())
}

pub fn log(level: LogLevel, message: &str) {
    let mut guard = match LOG_FILE.lock() {
        Ok(g) => g,
        Err(_) => {
            eprintln!("Failed to lock log file mutex");
            return;
        }
    };
    if let Some(file) = &mut *guard {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] {} - {}\n", timestamp, level.as_str(), message);
        if let Err(e) = file.write_all(log_entry.as_bytes()) {
            eprintln!("Failed to write to log file: {}", e);
            return;
        }
        if let Err(e) = file.flush() {
            eprintln!("Failed to flush log file: {}", e);
        }
    }
}

pub fn debug(message: &str) {
    log(LogLevel::Debug, message)
}

pub fn info(message: &str) {
    log(LogLevel::Info, message)
}

pub fn warn(message: &str) {
    log(LogLevel::Warning, message)
}

pub fn error(message: &str) {
    log(LogLevel::Error, message)
}

pub fn handle_non_critical_error(err: &anyhow::Error) {
    let error_msg = format!("{}", err);
    error(&error_msg);
}
