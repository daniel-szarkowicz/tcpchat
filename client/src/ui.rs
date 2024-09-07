use std::io::{stdout, Result, StdoutLock, Write};
use std::str::FromStr;
use std::time::Duration;

use common::commands::ServerCommand;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Color, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{ExecutableCommand, QueueableCommand};
use log::error;

use crate::channel_logger;

const EXIT_KEY: KeyCode = KeyCode::Esc;

pub struct UI {
    stdout: StdoutLock<'static>,
    messages: Vec<Vec<(Color, String)>>,
    typing_buffer: String,
    width: u16,
    height: u16,
    dirty: bool,
}

impl UI {
    pub fn new() -> Result<Self> {
        let mut this = Self {
            stdout: stdout().lock(),
            messages: vec![vec![(
                Color::DarkGrey,
                format!("Press {EXIT_KEY} to exit"),
            )]],
            typing_buffer: String::new(),
            width: 0,
            height: 0,
            dirty: true,
        };
        this.stdout.execute(EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;
        (this.width, this.height) = terminal::size()?;
        Ok(this)
    }

    pub fn render(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.stdout.queue(Clear(ClearType::All))?;
        self.dirty = false;
        for (offset, (index, message)) in self
            .messages
            .iter()
            .enumerate()
            .rev()
            .enumerate()
            .take(self.height as usize - 2)
        {
            self.stdout
                .queue(MoveTo(0, self.height - 3 - (offset as u16)))?;
            self.stdout.queue(SetForegroundColor(Color::DarkGrey))?;
            write!(self.stdout, "{index}> ")?;
            for (color, text) in message {
                self.stdout.queue(SetForegroundColor(*color))?;
                write!(self.stdout, "{text}")?;
            }
        }

        self.stdout.queue(MoveTo(0, self.height - 2))?;
        self.stdout.queue(SetForegroundColor(Color::Reset))?;
        write!(self.stdout, "{}", "-".repeat(self.width as usize))?;

        self.stdout.queue(MoveTo(0, self.height - 1))?;

        // TODO: handle wide characters
        let char_count = self.typing_buffer.chars().count();
        if char_count > self.width as usize {
            self.stdout.queue(SetForegroundColor(Color::Grey))?;
            write!(self.stdout, "...")?;
            self.stdout.queue(SetForegroundColor(Color::Reset))?;
            for c in self
                .typing_buffer
                .chars()
                .skip(char_count - self.width as usize + 3)
            {
                write!(self.stdout, "{c}")?;
            }
        } else {
            self.stdout.queue(SetForegroundColor(Color::Reset))?;
            for c in self.typing_buffer.chars() {
                write!(self.stdout, "{c}")?;
            }
        }
        self.stdout.flush()?;
        Ok(())
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn handle_key(&mut self, key_event: KeyEvent) -> Option<UIEvent> {
        match key_event.code {
            EXIT_KEY => Some(UIEvent::Exit),
            KeyCode::Backspace => {
                self.mark_dirty();
                self.typing_buffer.pop();
                None
            }
            KeyCode::Enter => {
                if self.typing_buffer.is_empty() {
                    None
                } else {
                    let event = self.typing_buffer.parse().ok()?;
                    self.mark_dirty();
                    // TODO: add to history
                    self.typing_buffer.clear();
                    Some(event)
                }
            }
            KeyCode::Char(c) => {
                self.typing_buffer.push(c);
                self.mark_dirty();
                None
            }
            _ => None,
        }
    }

    pub fn add_message(&mut self, message: ServerCommand) {
        self.mark_dirty();
        match message {
            ServerCommand::Padding => (),
            ServerCommand::AddUser { user_id, name } => {
                self.messages.push(vec![
                    (Color::Blue, format!("User Connected {user_id}")),
                    (Color::White, name),
                ]);
            }
            ServerCommand::RemoveUser { user_id } => {
                self.messages.push(vec![(
                    Color::Blue,
                    format!("User Disconnected {user_id}"),
                )]);
            }
            ServerCommand::Message {
                msg_id: _,
                user_id: _,
                message,
            } => self.messages.push(vec![(Color::Reset, message)]),
        }
    }

    pub fn add_log(&mut self, log: channel_logger::LogEntry) {
        self.mark_dirty();
        self.messages.push(vec![
            (
                match log.level {
                    log::Level::Error => Color::Red,
                    log::Level::Warn => Color::DarkYellow,
                    log::Level::Info => Color::Green,
                    log::Level::Debug => Color::Blue,
                    log::Level::Trace => Color::Yellow,
                },
                format!("{}: ", log.level),
            ),
            (Color::Reset, log.message),
        ]);
    }

    pub fn poll(&mut self) -> Result<Option<UIEvent>> {
        Ok(if event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(event) => self.handle_key(event),
                Event::Resize(w, h) => {
                    (self.width, self.height) = (w, h);
                    self.mark_dirty();
                    None
                }
                _ => None,
            }
        } else {
            None
        })
    }
}

impl Drop for UI {
    fn drop(&mut self) {
        match terminal::disable_raw_mode() {
            Ok(()) => (),
            Err(e) => error!("Error while disabling raw mode: {e}"),
        }
        match stdout().execute(LeaveAlternateScreen) {
            Ok(_) => (),
            Err(e) => error!("Error while leaving alternate screen: {e}"),
        }
    }
}

pub enum UIEvent {
    Exit,
    Message(String),
    Connect {
        server_addr: String,
        user_name: String,
    },
    Disconnect,
}

impl FromStr for UIEvent {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some(rawcommand) = s.strip_prefix("/") {
            let mut args = rawcommand.split_whitespace();
            let cmd = args.next().ok_or(())?;
            match cmd {
                "connect" => Ok(Self::Connect {
                    server_addr: args.next().ok_or(())?.to_owned(),
                    user_name: args.next().ok_or(())?.to_owned(),
                }),
                "disconnect" => Ok(Self::Disconnect),
                _ => Err(()),
            }
        } else {
            Ok(Self::Message(s.to_string()))
        }
    }
}
