use std::io::{self, Write};

use fleet_sim::Engine;

fn main() -> io::Result<()> {
    // Stdout is line-buffered, so each response goes out as soon as it's written.
    let mut stdout = io::stdout().lock();
    let mut engine = Engine::new();

    for line in io::stdin().lines() {
        let line = line?;
        if !line.trim().is_empty() {
            writeln!(stdout, "{}", engine.handle_line(&line))?;
        }
    }
    Ok(())
}
