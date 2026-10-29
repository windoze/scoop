use clap::Parser;
use scoop_example::{Args, COMMAND_NAME, Error, Format};
use std::io::{self, Write};
use std::process::ExitCode;

fn report(error: Error, json: bool) -> ExitCode {
    if json {
        eprintln!(
            "{}",
            serde_json::json!({
                "schema": 1, "kind": "error", "code": error.code, "message": error.message
            })
        );
    } else {
        eprintln!("{COMMAND_NAME}: {}", error.message);
    }
    ExitCode::from(error.code)
}

fn main() -> ExitCode {
    let raw: Vec<_> = std::env::args_os().collect();
    let json = raw.iter().any(|s| s == "--format=json")
        || raw.windows(2).any(|s| s[0] == "--format" && s[1] == "json");
    let args = match Args::try_parse_from(raw) {
        Ok(args) => args,
        Err(error) if !error.use_stderr() => {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        Err(error) => return report(Error::usage(error.to_string()), json),
    };
    let result = std::env::current_exe()
        .map_err(|e| Error::content(format!("无法定位当前命令：{e}")))
        .and_then(|exe| scoop_example::installation_root(&exe))
        .and_then(|root| scoop_example::run(&root, &args));
    match result {
        Ok(text) => match io::stdout().lock().write_all(text.as_bytes()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(e) => report(
                Error::content(format!("无法写入 stdout：{e}")),
                args.format == Format::Json,
            ),
        },
        Err(error) => report(error, args.format == Format::Json),
    }
}
