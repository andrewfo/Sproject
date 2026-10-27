use std::io::{self, BufRead, Write};

use fleet_sim::Engine;

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut engine = Engine::new();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = engine.handle_line(&line);
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
