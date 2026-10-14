use super::*;
use crate::cli::test_run::model::{AssertionValue, Failure, FailureDetail};

pub(super) fn write(
    mut out: &mut impl Write,
    target: &str,
    selected: &SelectedTests,
    result: &RunResult,
    report: &Value,
) -> io::Result<()> {
    let counts = &report["counts"];
    writeln!(out, "{target}")?;
    let mut cases = result.cases.iter();
    for (index, container) in selected.report_directory.containers.iter().enumerate() {
        writeln!(
            out,
            "  {} [{}]",
            container.qualified_name,
            report["containers"][index]["status"].as_str().unwrap()
        )?;
        for case in &container.cases {
            let location = &case.location;
            write!(
                out,
                "    {} ({}:{}:{})",
                case.name, location.path, location.line, location.column
            )?;
            if let Some(reason) = &case.ignore_reason
                && !selected.run_ignored
            {
                writeln!(out, " ignored: {reason}")?;
                continue;
            }
            let record = cases.next().expect("report uses selected order");
            let status = match record.status {
                Status::Passed => "passed",
                Status::Failed => "failed",
                Status::NotRun => "not run",
            };
            write!(out, " {status}")?;
            if let Some(duration) = record.duration_ns {
                write!(out, " ({duration} ns)")?;
            }
            writeln!(out)?;
            for failed in &record.failures {
                failure(&mut out, failed)?;
            }
            if let Some(interruption) = &record.interruption {
                writeln!(out, "      {interruption}")?;
            }
            captured(&mut out, "case", record.output.as_ref())?;
        }
    }
    if let Some(failure) = &result.target_failure {
        writeln!(out, "  target failed: {failure}")?;
        if let Some(signal) = report["process"]["signal"].as_i64() {
            writeln!(out, "  process signal: {signal}")?;
        } else if let Some(code) = report["process"]["code"].as_i64() {
            writeln!(out, "  process exit: {code}")?;
        }
    }
    captured(&mut out, "target", result.output.as_ref())?;
    write!(
        out,
        "{} containers, {} tests: {} passed, {} failed, {} ignored, {} not run",
        counts["containers"],
        counts["cases"],
        counts["passed"],
        counts["failed"],
        counts["ignored"],
        counts["not_run"]
    )?;
    if let Some(reason) = selected.empty_reason {
        write!(out, " ({reason})")?;
    }
    writeln!(out)
}

fn failure(out: &mut impl Write, failure: &Failure) -> io::Result<()> {
    let location = &failure.location;
    let phase = serde_json::to_value(&failure.phase).expect("phase is a string");
    writeln!(
        out,
        "      {} at {}:{}:{}",
        phase.as_str().unwrap(),
        location.file,
        location.line,
        location.column
    )?;
    match &failure.detail {
        FailureDetail::Exception { message } => writeln!(out, "      {message}"),
        FailureDetail::Assertion {
            assertion,
            message,
            expected,
            actual,
            cause,
        } => {
            writeln!(
                out,
                "      {assertion}: {}",
                message.as_deref().unwrap_or("assertion failed")
            )?;
            value(out, "expected", expected)?;
            value(out, "actual", actual)?;
            if let Some(cause) = cause {
                writeln!(out, "      cause: {cause}")?;
            }
            Ok(())
        }
    }
}

fn value(out: &mut impl Write, name: &str, value: &AssertionValue) -> io::Result<()> {
    match value {
        AssertionValue::Unavailable => Ok(()),
        AssertionValue::Text { text } => writeln!(out, "      {name}: {text}"),
        AssertionValue::RenderingFailed { message } => {
            writeln!(out, "      {name}: <rendering failed: {message}>")
        }
    }
}

fn captured(out: &mut impl Write, scope: &str, value: Option<&Output>) -> io::Result<()> {
    if let Some(value) = value {
        for (stream, bytes) in [("stdout", &value.stdout), ("stderr", &value.stderr)] {
            if !bytes.is_empty() {
                writeln!(out, "      {scope} {stream}:")?;
                out.write_all(bytes)?;
                if !bytes.ends_with(b"\n") {
                    writeln!(out)?;
                }
            }
        }
    }
    Ok(())
}
