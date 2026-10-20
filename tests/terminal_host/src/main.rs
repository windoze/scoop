mod output;

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
    let scenario = args.next().context("usage: terminal-host output PROGRAM")?;
    let program = args.next().context("missing Scoop executable")?;
    if args.next().is_some() || scenario != "output" {
        bail!("usage: terminal-host output PROGRAM");
    }
    let mut host = Terminal::spawn(CommandBuilder::new(program), 20, 8)?;
    let interrupt = signal_hook::flag::register(SIGINT, host.interrupted.clone())?;
    let terminate = signal_hook::flag::register(SIGTERM, host.interrupted.clone())?;
    let result = output::check(&mut host).and_then(|()| host.finish());
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
    println!("terminal output: ok");
    Ok(())
}
