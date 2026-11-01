//! Minimal terminal logger used on Windows, where `env_logger` pulls in
//! `windows-sys` which requires a working `dlltool` that the GNU toolchain
//! lacks.

use std::sync::OnceLock;

use log::{LevelFilter, Metadata, Record};
use std::io::Write;

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= LEVEL.get().copied().unwrap_or(LevelFilter::Warn)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = writeln!(
                std::io::stderr(),
                "[{:<5}] {}",
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;
static LEVEL: OnceLock<LevelFilter> = OnceLock::new();

pub fn init(level: &str) {
    let filter = match level {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Warn,
    };
    let _ = LEVEL.set(filter);
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(filter));
}
