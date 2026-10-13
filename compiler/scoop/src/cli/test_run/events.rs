use std::io::BufRead;

use serde::Deserialize;

use super::model::{Failure, RunResult, Status};

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "lowercase", deny_unknown_fields)]
enum Event {
    Begin {
        case: String,
    },
    End {
        case: String,
        duration_ns: u64,
        failures: Vec<Failure>,
    },
    Complete,
}

pub(super) fn read(input: impl BufRead, expected: &[&str]) -> RunResult {
    let mut result = RunResult::empty(expected.len());
    let mut next = 0;
    let mut active = false;
    for line in input.lines() {
        let event = line.map_err(|e| e.to_string()).and_then(|line| {
            serde_json::from_str::<Event>(&line).map_err(|e| format!("invalid test event: {e}"))
        });
        let error = match event {
            Err(error) => Some(error),
            Ok(_) if result.complete => Some("test event follows completion".into()),
            Ok(Event::Begin { case }) if !active && expected.get(next) == Some(&case.as_str()) => {
                let record = &mut result.cases[next];
                record.status = Status::Failed;
                record.interruption = Some("test process ended before the case finished".into());
                active = true;
                None
            }
            Ok(Event::End {
                case,
                duration_ns,
                failures,
            }) if active && expected.get(next) == Some(&case.as_str()) && failures.len() <= 2 => {
                let record = &mut result.cases[next];
                record.status = if failures.is_empty() {
                    Status::Passed
                } else {
                    Status::Failed
                };
                record.duration_ns = Some(duration_ns);
                record.failures = failures;
                record.interruption = None;
                next += 1;
                active = false;
                None
            }
            Ok(Event::Complete) if !active && next == expected.len() => {
                result.complete = true;
                None
            }
            Ok(_) => Some("test events do not match the selected case order or lifecycle".into()),
        };
        if let Some(error) = error {
            result.target_failure = Some(error);
            return result;
        }
    }
    if !result.complete {
        result.target_failure = Some("test process ended without a completion record".into());
    }
    result
}

#[cfg(test)]
mod tests;
