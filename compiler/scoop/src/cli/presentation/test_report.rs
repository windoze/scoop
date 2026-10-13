use std::process::ExitStatus;

use super::*;
use crate::cli::test_run::model::{Output, RunResult, Status};
use crate::cli::test_selection::SelectedTests;

mod human;
#[cfg(test)]
mod tests;

impl Reporter {
    pub fn test_report(
        &self,
        target: &str,
        selected: &SelectedTests,
        result: &RunResult,
        process: Option<ExitStatus>,
    ) -> io::Result<()> {
        let mut cases = result.cases.iter();
        let mut ignored = 0;
        let containers = selected
            .report_directory
            .containers
            .iter()
            .map(|container| {
                let cases = container
                    .cases
                    .iter()
                    .map(|case| {
                        let mut record = json!({
                            "name": case.name, "display_name": case.display_name,
                            "location": test_list::location(&case.location),
                            "ignore_reason": case.ignore_reason,
                        });
                        if case.ignore_reason.is_some() && !selected.run_ignored {
                            ignored += 1;
                            record["status"] = json!("ignored");
                        } else {
                            let outcome = cases
                                .next()
                                .expect("selection and report preserve case order");
                            record["status"] = json!(outcome.status);
                            record["duration_ns"] = json!(outcome.duration_ns);
                            record["failures"] = json!(outcome.failures);
                            record["interruption"] = json!(outcome.interruption);
                            record["output"] = output(outcome.output.as_ref());
                        }
                        record
                    })
                    .collect::<Vec<_>>();
                let statuses = cases
                    .iter()
                    .map(|case| case["status"].as_str().unwrap())
                    .collect::<Vec<_>>();
                let status = ["failed", "not-run", "passed", "ignored"]
                    .into_iter()
                    .find(|status| statuses.contains(status))
                    .unwrap_or("ignored");
                json!({"name": container.qualified_name, "status": status,
                "location": test_list::location(&container.location), "cases": cases})
            })
            .collect::<Vec<_>>();
        let count = |status| {
            result
                .cases
                .iter()
                .filter(|case| case.status == status)
                .count()
        };
        let record = json!({
            "schema": 1, "kind": "test-report", "target": target,
            "status": if result.failed() { "failed" } else { "passed" },
            "complete": result.complete, "empty_reason": selected.empty_reason,
            "containers": containers,
            "counts": {"containers": containers.len(), "cases": result.cases.len() + ignored,
                "passed": count(Status::Passed), "failed": count(Status::Failed),
                "not_run": count(Status::NotRun), "ignored": ignored},
            "target_failure": result.target_failure, "output": output(result.output.as_ref()),
            "process": process.map(process_status),
        });
        match self.0 {
            MessageFormat::Json => write_json(&record),
            MessageFormat::Human => {
                human::write(&mut io::stdout().lock(), target, selected, result, &record)
            }
        }
    }
}

fn process_status(status: ExitStatus) -> Value {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let signal: Option<i32> = None;
    json!({"code": status.code(), "signal": signal})
}

fn output(value: Option<&Output>) -> Value {
    value.map_or(Value::Null, |value| {
        json!({
            "stdout": bytes(&value.stdout), "stderr": bytes(&value.stderr),
        })
    })
}

fn bytes(value: &[u8]) -> Value {
    match std::str::from_utf8(value) {
        Ok(text) => json!({"text": text}),
        Err(_) => {
            json!({"hex": value.iter().map(|byte| format!("{byte:02x}")).collect::<String>()})
        }
    }
}
