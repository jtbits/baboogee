use std::{
    io::{self, stdout, Write},
    process::exit,
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo},
    event::{poll, read, Event, KeyCode, KeyModifiers},
    style::{PrintStyledContent, Stylize},
    terminal::{self, Clear, ClearType},
    QueueableCommand,
};

fn draw_map(
    stdout: &mut io::Stdout,
    (terminal_width, terminal_height): (u16, u16),
) -> Result<(), io::Error> {
    let center_coords = (terminal_width / 2, terminal_height / 2);

    for x in 0..terminal_height - 2 {
        stdout.queue(MoveTo(0, x))?;
        stdout.queue(PrintStyledContent(
            "X".repeat(terminal_width as usize).grey(),
        ))?;
    }

    stdout.queue(MoveTo(center_coords.0, center_coords.1))?;
    stdout.queue(PrintStyledContent('P'.red()))?;

    Ok(())
}

fn draw_line(stdout: &mut io::Stdout, x: u16, w: usize) -> Result<(), io::Error> {
    stdout.queue(MoveTo(0, x))?;
    stdout.queue(PrintStyledContent("=".repeat(w).green()))?;

    Ok(())
}

fn main() -> Result<(), io::Error> {
    terminal::enable_raw_mode().expect("failed to enable raw mode");
    let mut stdout = stdout();
    let mut terminal_dimensions = terminal::size()?;

    stdout.queue(Clear(ClearType::All))?;
    stdout.queue(Hide)?;

    loop {
        while poll(Duration::ZERO)? {
            match read()? {
                Event::Resize(w, h) => terminal_dimensions = (w, h),
                Event::Key(event) => {
                    if let KeyCode::Char(c) = event.code {
                        if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) {
                            terminal::disable_raw_mode()?;
                            exit(0);
                        }
                    }
                }
                _ => {}
            }
        }

        draw_map(&mut stdout, terminal_dimensions).expect("error during drawing map");
        draw_line(
            &mut stdout,
            terminal_dimensions.1 - 2,
            terminal_dimensions.0 as usize,
        )
        .expect("error during drawign line");

        stdout.flush()?;

        thread::sleep(Duration::from_millis(33));
    }
}
