use std::process::ExitCode;

mod cli;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err((error, machine)) => {
            if machine {
                match error.machine_diagnostic().and_then(|value| {
                    serde_json::to_string(&value).map_err(|error| error.to_string())
                }) {
                    Ok(record) => eprintln!("{record}"),
                    Err(encoding) => {
                        eprintln!("error: {error}\ncannot encode diagnostic: {encoding}")
                    }
                }
            } else {
                eprintln!("error: {error}");
            }
            ExitCode::FAILURE
        }
    }
}
