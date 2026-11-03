//! Local log file (section 8): "Logs bleiben lokal, werden rotiert und
//! enthalten keine Dokumentinhalte."
//!
//! * Only Rust writes the log; the frontend has no logging command.
//! * `project-write.log` in the log folder of the system; at 1 MiB it is
//!   renamed to `project-write.1.log` (older ones shift up to `.3.log`, the
//!   oldest is dropped).
//! * One entry per line; line breaks inside a message are escaped, so an
//!   entry cannot fake further entries.
//! * Messages name ids and counts, never document content. Libraries that
//!   log SQL (`sqlx`) or window internals are limited to warnings.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{LevelFilter, Log, Metadata, Record};

use crate::clock::Timestamp;

pub const LOG_FILE: &str = "project-write.log";
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const KEPT_LOG_FILES: u32 = 3;

/// Targets that are only logged from `Warn` upwards.
const QUIET_TARGETS: &[&str] = &["sqlx", "tao", "wry", "tauri", "tracing", "zbus", "mio"];

/// Installs the file logger for the whole process. Calling it again has no
/// effect.
pub fn init(dir: &Path) -> io::Result<()> {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let logger = FileLogger::open(dir, level, MAX_LOG_BYTES, KEPT_LOG_FILES)?;
    // The logger lives as long as the process.
    let logger: &'static FileLogger = Box::leak(Box::new(logger));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
    Ok(())
}

/// A size-rotated log file.
pub struct FileLogger {
    level: LevelFilter,
    echo_to_stderr: bool,
    state: Mutex<LogState>,
}

struct LogState {
    dir: PathBuf,
    file: File,
    size: u64,
    max_bytes: u64,
    kept: u32,
}

impl FileLogger {
    pub fn open(dir: &Path, level: LevelFilter, max_bytes: u64, kept: u32) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let file = open_append(&dir.join(LOG_FILE))?;
        let size = file.metadata()?.len();
        Ok(Self {
            level,
            echo_to_stderr: cfg!(debug_assertions),
            state: Mutex::new(LogState {
                dir: dir.to_path_buf(),
                file,
                size,
                max_bytes,
                kept,
            }),
        })
    }

    fn line(record: &Record<'_>) -> String {
        let message = record.args().to_string();
        let message = message
            .replace('\\', "\\\\")
            .replace('\r', "\\r")
            .replace('\n', "\\n");
        format!(
            "{} {:<5} {}: {message}\n",
            Timestamp::now().to_iso(),
            record.level(),
            record.target()
        )
    }
}

fn open_append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

impl LogState {
    fn rotate(&mut self) -> io::Result<()> {
        let name = |n: u32| self.dir.join(format!("project-write.{n}.log"));
        let _ = fs::remove_file(name(self.kept));
        for n in (1..self.kept).rev() {
            let from = name(n);
            if from.exists() {
                fs::rename(&from, name(n + 1))?;
            }
        }
        self.file.flush()?;
        fs::rename(self.dir.join(LOG_FILE), name(1))?;
        self.file = open_append(&self.dir.join(LOG_FILE))?;
        self.size = 0;
        Ok(())
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let quiet = QUIET_TARGETS.iter().any(|target| {
            metadata.target() == *target || metadata.target().starts_with(&format!("{target}::"))
        });
        let limit = if quiet {
            self.level.min(LevelFilter::Warn)
        } else {
            self.level
        };
        metadata.level() <= limit
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = Self::line(record);
        if self.echo_to_stderr {
            eprint!("{line}");
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.size + line.len() as u64 > state.max_bytes && state.size > 0 {
            // A failed rotation keeps writing to the current file.
            let _ = state.rotate();
        }
        if state.file.write_all(line.as_bytes()).is_ok() {
            state.size += line.len() as u64;
        }
    }

    fn flush(&self) {
        if let Ok(mut state) = self.state.lock() {
            let _ = state.file.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;

    fn dir() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("tmp");
        fs::create_dir_all(&base).unwrap();
        tempfile::tempdir_in(base).unwrap()
    }

    fn log_line(logger: &FileLogger, level: Level, target: &str, message: &str) {
        logger.log(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{message}"))
                .build(),
        );
    }

    #[test]
    fn writes_one_escaped_line_per_entry() {
        let dir = dir();
        let logger = FileLogger::open(dir.path(), LevelFilter::Info, 1024 * 1024, 3).unwrap();
        log_line(
            &logger,
            Level::Info,
            "project_write_lib::x",
            "first\nINFO fake: entry",
        );
        log_line(&logger, Level::Debug, "project_write_lib::x", "too verbose");
        logger.flush();
        let text = fs::read_to_string(dir.path().join(LOG_FILE)).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("INFO  project_write_lib::x: first\\nINFO fake: entry"));
        assert!(!text.contains("too verbose"));
    }

    #[test]
    fn noisy_libraries_log_warnings_only() {
        let dir = dir();
        let logger = FileLogger::open(dir.path(), LevelFilter::Debug, 1024 * 1024, 3).unwrap();
        log_line(&logger, Level::Info, "sqlx::query", "SELECT …");
        log_line(&logger, Level::Warn, "sqlx::query", "slow statement");
        log_line(&logger, Level::Debug, "project_write_lib::y", "own debug");
        logger.flush();
        let text = fs::read_to_string(dir.path().join(LOG_FILE)).unwrap();
        assert!(!text.contains("SELECT"));
        assert!(text.contains("slow statement"));
        assert!(text.contains("own debug"));
    }

    #[test]
    fn rotates_and_keeps_three_old_files() {
        let dir = dir();
        let logger = FileLogger::open(dir.path(), LevelFilter::Info, 200, 3).unwrap();
        for i in 0..40 {
            log_line(
                &logger,
                Level::Info,
                "t",
                &format!("entry {i:03} {}", "x".repeat(40)),
            );
        }
        logger.flush();
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "project-write.1.log",
                "project-write.2.log",
                "project-write.3.log",
                "project-write.log"
            ]
        );
        for name in &names {
            assert!(
                fs::metadata(dir.path().join(name)).unwrap().len() <= 200,
                "{name}"
            );
        }
        let newest = fs::read_to_string(dir.path().join(LOG_FILE)).unwrap();
        assert!(newest.contains("entry 039"));
    }
}
