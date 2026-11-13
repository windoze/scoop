//! Native training orchestration. All compilation remains in the paired driver.
use std::process::ExitStatus;

mod args;
mod build;
mod data;
mod merge;
mod process;
mod workflow;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() {
    match args::parse().and_then(workflow::execute) {
        Ok(Some(status)) => scoop_process::exit_with_status(status),
        Ok(None) => {}
        Err(error) => {
            eprintln!("scoop-pgo: {error}");
            std::process::exit(1);
        }
    }
}
