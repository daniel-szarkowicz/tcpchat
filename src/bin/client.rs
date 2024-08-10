use std::io::{stdout, Result, StdoutLock, Write};
use std::net::TcpStream;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Color, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{ExecutableCommand, QueueableCommand};
use log::{error, info};

use chat::client::channel_logger;
use chat::server::Client;

fn main() -> Result<()> {
    let log_receiver = channel_logger::init_and_get_receiver();
    let mut ui = UI::new()?;
    let mut run = true;
    let mut client = Client::new(TcpStream::connect("localhost:6969")?)?;

    while run {
        if let Some(msg) = client.poll() {
            ui.add_message(msg);
        }
        match log_receiver.try_recv() {
            Ok(log) => ui.add_message(log.to_string()),
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                ui.add_message("ERROR: logging just died".to_owned());
            }
        }
        match ui.poll()? {
            UIEvent::Nothing => (),
            UIEvent::Exit => run = false,
            UIEvent::Message(msg) => {
                client.send(&msg);
                client.flush();
            }
        }
        ui.render()?;
        if !client.connected() {
            run = false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(log_receiver);
    Ok(())
}

struct UI {
    stdout: StdoutLock<'static>,
    messages: Vec<String>,
    typing_buffer: String,
    width: u16,
    height: u16,
    dirty: bool,
}

impl UI {
    fn new() -> Result<Self> {
        let mut this = Self {
            stdout: stdout().lock(),
            messages: vec!["Press Esc to exit".to_owned()],
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
            write!(self.stdout, "{index}> {message}")?;
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

    fn handle_key(&mut self, key_event: KeyEvent) -> UIEvent {
        match key_event.code {
            KeyCode::Esc => UIEvent::Exit,
            KeyCode::Backspace => {
                self.mark_dirty();
                self.typing_buffer.pop();
                UIEvent::Nothing
            }
            KeyCode::Enter => {
                if self.typing_buffer.is_empty() {
                    UIEvent::Nothing
                } else {
                    self.mark_dirty();
                    self.typing_buffer.push('\n');
                    UIEvent::Message(std::mem::take(&mut self.typing_buffer))
                }
            }
            KeyCode::Char(c) => {
                self.typing_buffer.push(c);
                self.mark_dirty();
                UIEvent::Nothing
            }
            _ => UIEvent::Nothing,
        }
    }

    fn add_message(&mut self, message: String) {
        self.mark_dirty();
        self.messages.push(message);
    }

    fn poll(&mut self) -> Result<UIEvent> {
        Ok(if event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(event) => self.handle_key(event),
                Event::Resize(w, h) => {
                    (self.width, self.height) = (w, h);
                    self.mark_dirty();
                    UIEvent::Nothing
                }
                _ => UIEvent::Nothing,
            }
        } else {
            UIEvent::Nothing
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
    Nothing,
    Exit,
    Message(String),
}
