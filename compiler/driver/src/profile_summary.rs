use clap::{Args, ValueEnum};
use scoop_profile::{Count, Location, MissingCount};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Args)]
pub(crate) struct SummaryArgs {
    /// Scoop index referencing an indexed LLVM profile and logical position maps.
    index: PathBuf,
    #[arg(long, value_enum, default_value = "human")]
    message_format: MessageFormat,
}

#[derive(Clone, Copy, ValueEnum)]
enum MessageFormat {
    Human,
    Json,
}

fn missing(reason: MissingCount) -> &'static str {
    match reason {
        MissingCount::MissingBody => "missing-body",
        MissingCount::BodyChanged => "body-changed",
        MissingCount::NotEmitted => "not-emitted",
        MissingCount::MissingRecord => "missing-record",
        MissingCount::ZeroEntry => "zero-entry",
        MissingCount::InconsistentFlow => "inconsistent-flow",
        MissingCount::Overflow => "overflow",
    }
}

fn count(value: Count) -> Value {
    match value {
        Count::Observed { value, partial } => json!({"count": value, "partial": partial}),
        Count::Missing(reason) => json!({"count": null, "missing": missing(reason)}),
    }
}

fn summary(args: SummaryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let loaded = scoopc::profiling::read_profile(&args.index, None)?;
    let mut bodies = std::collections::BTreeMap::new();
    for body in loaded.maps.iter().flat_map(|map| &map.bodies) {
        bodies.entry(body.key).or_insert(body);
    }
    let mut output = std::io::stdout().lock();
    match args.message_format {
        MessageFormat::Json => {
            let records = bodies.values().map(|body| {
                let positions = body.positions.iter().enumerate().map(|(position, kind)| {
                    let location = Location { body: body.key, position: position as u32 };
                    let mut value = count(loaded.feedback.count(location));
                    value["position"] = json!(position);
                    value["kind"] = json!(kind.kind);
                    if kind.kind == scoop_profile::PositionKind::Call {
                        value["frequency"] = match loaded.feedback.frequency(location) {
                            Ok(f) => json!({"calls": f.calls, "entries": f.entries, "partial": f.partial}),
                            Err(reason) => json!({"missing": missing(reason)}),
                        };
                    }
                    value
                }).collect::<Vec<_>>();
                json!({"key": body.key, "name": body.name, "positions": positions})
            }).collect::<Vec<_>>();
            serde_json::to_writer(
                &mut output,
                &json!({"schema": 1, "kind": "profile-summary", "target": loaded.target, "bodies": records}),
            )?;
            writeln!(output)?;
        }
        MessageFormat::Human => {
            writeln!(
                output,
                "Scoop profile for {} ({} logical bodies)",
                loaded.target,
                bodies.len()
            )?;
            for body in bodies.values() {
                match loaded.feedback.count(Location {
                    body: body.key,
                    position: 0,
                }) {
                    Count::Observed { value, partial } => writeln!(
                        output,
                        "{value:>12} {}{}",
                        body.name,
                        if partial { " (partial)" } else { "" }
                    )?,
                    Count::Missing(reason) => {
                        writeln!(output, "{:>12} {} ({})", "-", body.name, missing(reason))?
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn run(args: SummaryArgs) -> ExitCode {
    match summary(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
