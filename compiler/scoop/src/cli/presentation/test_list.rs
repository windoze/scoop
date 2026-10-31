use super::*;
use scoop_slib::{TestDirectory, TestSourceLocation};

impl Reporter {
    pub fn test_list(
        &self,
        target: &str,
        directory: &TestDirectory,
        empty_reason: Option<&str>,
    ) -> io::Result<()> {
        let cases = directory.containers.iter().flat_map(|c| &c.cases).count();
        let ignored = directory
            .containers
            .iter()
            .flat_map(|c| &c.cases)
            .filter(|c| c.ignore_reason.is_some())
            .count();
        match self.0 {
            MessageFormat::Json => {
                let containers = containers(directory);
                write_json(&json!({
                    "schema": 1, "kind": "test-list", "target": target,
                    "containers": containers,
                    "counts": {"containers": containers.len(), "cases": cases, "ignored": ignored},
                    "empty_reason": empty_reason,
                }))
            }
            MessageFormat::Human => {
                let mut out = io::stdout().lock();
                writeln!(out, "{target}")?;
                for container in &directory.containers {
                    let loc = &container.location;
                    writeln!(
                        out,
                        "  {} ({}:{}:{})",
                        container.qualified_name, loc.path, loc.line, loc.column
                    )?;
                    for case in &container.cases {
                        let loc = &case.location;
                        write!(
                            out,
                            "    {} ({}:{}:{})",
                            case.name, loc.path, loc.line, loc.column
                        )?;
                        if let Some(reason) = &case.ignore_reason {
                            write!(out, " [ignored: {reason}]")?;
                        }
                        writeln!(out)?;
                    }
                }
                write!(
                    out,
                    "{} containers, {cases} tests, {ignored} ignored",
                    directory.containers.len(),
                )?;
                if let Some(reason) = empty_reason {
                    write!(out, " ({reason})")?;
                }
                writeln!(out)
            }
        }
    }
}

pub(super) fn containers(directory: &TestDirectory) -> Vec<Value> {
    directory
        .containers
        .iter()
        .map(|container| {
            json!({
                "name": container.qualified_name,
                "location": location(&container.location),
                "cases": container.cases.iter().map(|case| json!({
                    "name": case.name, "display_name": case.display_name,
                    "location": location(&case.location), "ignore_reason": case.ignore_reason,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub(super) fn location(location: &TestSourceLocation) -> Value {
    json!({"path": location.path, "line": location.line, "column": location.column})
}
