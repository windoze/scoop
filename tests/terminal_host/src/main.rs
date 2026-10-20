mod clipboard;
mod input;
mod output;
mod query;
mod session;

use anyhow::{Context, Result, bail};
use portable_pty::CommandBuilder;
use signal_hook::consts::{SIGINT, SIGTERM};
use terminal_host::Terminal;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let scenario = args
        .next()
        .context("usage: terminal-host SCENARIO PROGRAM")?;
    let program = args.next().context("missing Scoop executable")?;
    let scenario = scenario.to_str().context("scenario must be UTF-8")?;
    if args.next().is_some()
        || !matches!(
            scenario,
            "output" | "session" | "session-raw" | "clipboard" | "input" | "query" | "asyncio-tty"
        )
    {
        bail!(
            "usage: terminal-host output|session|session-raw|clipboard|input|query|asyncio-tty PROGRAM"
        );
    }
    let command = CommandBuilder::new(program);
    let mut host = if scenario == "session-raw" {
        Terminal::spawn_raw(command, 20, 8)?
    } else {
        Terminal::spawn(command, 20, 8)?
    };
    let interrupt = signal_hook::flag::register(SIGINT, host.interrupted.clone())?;
    let terminate = signal_hook::flag::register(SIGTERM, host.interrupted.clone())?;
    let result = match scenario {
        "output" => output::check(&mut host),
        "clipboard" => clipboard::check(&mut host),
        "input" => input::check(&mut host),
        "query" => query::check(&mut host),
        "asyncio-tty" => asyncio_tty::check(&mut host),
        _ => session::check(&mut host),
    }
    .and_then(|()| host.finish());
    if result.is_err() {
        eprintln!("{}", host.diagnostics());
    }
    let cleanup = host.cleanup();
    signal_hook::low_level::unregister(interrupt);
    signal_hook::low_level::unregister(terminate);
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => bail!("{error:#}; cleanup also failed: {cleanup:#}"),
        (Err(error), _) | (_, Err(error)) => return Err(error),
        (Ok(()), Ok(())) => {}
    }
    println!("terminal {scenario}: ok");
    Ok(())
}
mod asyncio_tty;
