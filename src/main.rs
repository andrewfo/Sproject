use std::io::{self, BufRead, Write};

use fleet_sim::Engine;
use fleet_sim::error::SimError;
use fleet_sim::protocol;

fn main() -> io::Result<()> {
    // Stdout is line-buffered, so each response goes out as soon as it's written.
    let mut stdout = io::stdout().lock();
    let mut engine = Engine::new();

    // Read raw bytes so a line of bad UTF-8 gets an error response instead of ending the loop.
    for line in io::stdin().lock().split(b'\n') {
        let mut line = line?;
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let response = match String::from_utf8(line) {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => engine.handle_line(&line),
            Err(_) => protocol::render_response(Err(SimError::InvalidCommand(
                "line is not valid UTF-8".into(),
            ))),
        };
        writeln!(stdout, "{response}")?;
    }
    Ok(())
}
