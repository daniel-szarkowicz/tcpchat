use std::io::{stdout, Result, StdoutLock, Write};
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode};
use crossterm::style::{Color, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{ExecutableCommand, QueueableCommand};
use log::error;

fn main() -> Result<()> {
    let mut ui = UI::new()?;
    ui.run()
}

struct UI {
    stdout: StdoutLock<'static>,
    run: bool,
    messages: Vec<String>,
    typing_buffer: String,
    width: u16,
    height: u16,
}

impl UI {
    fn new() -> Result<Self> {
        let mut this = Self {
            stdout: stdout().lock(),
            run: true,
            messages: vec!["Press Esc to exit".to_owned()],
            typing_buffer: String::new(),
            width: 0,
            height: 0,
        };
        this.stdout.execute(EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;
        (this.width, this.height) = terminal::size()?;
        Ok(this)
    }

    fn render(&mut self) -> Result<()> {
        for (offset, (index, message)) in
            self.messages.iter().enumerate().rev().enumerate()
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

    fn run(&mut self) -> Result<()> {
        self.render()?;
        while self.run {
            if event::poll(Duration::ZERO)? {
                match event::read()? {
                    Event::Key(event) => match event.code {
                        KeyCode::Esc => self.run = false,
                        KeyCode::Backspace => {
                            self.typing_buffer.pop();
                            self.stdout.queue(Clear(ClearType::CurrentLine))?;
                        }
                        KeyCode::Enter => {
                            if !self.typing_buffer.is_empty() {
                                self.messages.push(std::mem::take(
                                    &mut self.typing_buffer,
                                ));
                                self.stdout.queue(Clear(ClearType::All))?;
                            }
                        }
                        KeyCode::Char(c) => self.typing_buffer.push(c),
                        _ => (),
                    },
                    Event::Resize(w, h) => {
                        (self.width, self.height) = (w, h);
                        self.stdout.queue(Clear(ClearType::All))?;
                    }
                    _ => (),
                }
                self.render()?;
            }
        }
        Ok(())
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
