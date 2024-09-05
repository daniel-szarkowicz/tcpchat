use std::io::{stdout, Result, StdoutLock, Write};
use std::net::TcpStream;
use std::time::Duration;

use chat::common::commands::{ClientCommand, ServerCommand};
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Color, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{ExecutableCommand, QueueableCommand};
use log::error;

use chat::client::channel_logger;
use chat::client::Server;

const EXIT_KEY: KeyCode = KeyCode::Esc;

fn main() -> Result<()> {
    let log_receiver = channel_logger::init_and_get_receiver();
    let mut ui = UI::new()?;
    let mut run = true;
    let mut server = Server::new(TcpStream::connect("localhost:6969")?)?;

    while run {
        while let Some(msg) = server.poll() {
            ui.add_message(msg);
        }
        while let Ok(log) = log_receiver.try_recv() {
            ui.add_log(log);
        }
        while let Some(event) = ui.poll()? {
            match event {
                UIEvent::Exit => run = false,
                UIEvent::Message(msg) => {
                    server.send(&ClientCommand::Message { message: msg });
                    server.flush();
                }
            }
        }
        ui.render()?;
        if !server.connected() {
            run = false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

struct UI {
    stdout: StdoutLock<'static>,
    messages: Vec<Vec<(Color, String)>>,
    typing_buffer: String,
    width: u16,
    height: u16,
    dirty: bool,
}

impl UI {
    fn new() -> Result<Self> {
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

    fn render(&mut self) -> Result<()> {
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
                    self.mark_dirty();
                    self.typing_buffer.push('\n');
                    Some(UIEvent::Message(std::mem::take(
                        &mut self.typing_buffer,
                    )))
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

    fn add_message(&mut self, message: ServerCommand) {
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

    fn add_log(&mut self, log: channel_logger::LogEntry) {
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

    fn poll(&mut self) -> Result<Option<UIEvent>> {
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

enum UIEvent {
    Exit,
    Message(String),
}
