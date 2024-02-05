use colored::Colorize;
use log::{Level, LevelFilter};

struct MicroLog {}

pub fn init(level: LevelFilter) {
    static LOGGER: MicroLog = MicroLog {};
    log::set_logger(&LOGGER).expect("Failed to set the logger");
    log::set_max_level(level);
}

impl log::Log for MicroLog {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let lvl = match record.level() {
                Level::Error => "Error".red(),
                Level::Warn => "Warn ".yellow(),
                Level::Info => "Info ".green(),
                Level::Debug => "Debug".blue(),
                Level::Trace => "Trace".white(),
            };
            eprintln!("{} {}", lvl, record.args());
        }
    }

    fn flush(&self) {}
}
