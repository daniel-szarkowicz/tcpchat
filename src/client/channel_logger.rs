use std::fmt::Display;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{LazyLock, Mutex};

use log::{Level, Log};

#[derive(Debug)]
pub struct LogEntry {
    pub level: Level,
    pub message: String,
}

impl Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.level, self.message)
    }
}

static LOGGER: LazyLock<ChannelLogger> = LazyLock::new(|| ChannelLogger);
static CHANNELS: LazyLock<Mutex<Vec<Sender<LogEntry>>>> =
    LazyLock::new(Mutex::default);

pub fn init_and_get_receiver() -> Receiver<LogEntry> {
    let (sender, receiver) = channel();
    if let Ok(mut channels) = CHANNELS.lock() {
        channels.push(sender);
    };
    if log::set_logger(&*LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    receiver
}

struct ChannelLogger;
impl Log for ChannelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        log::max_level()
            .to_level()
            .is_some_and(|l| metadata.level() <= l)
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            CHANNELS.lock().unwrap().retain(|sender| {
                sender
                    .send(LogEntry {
                        level: record.metadata().level(),
                        message: record.args().to_string(),
                    })
                    .is_ok()
            });
        }
    }

    fn flush(&self) {}
}
