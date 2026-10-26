use super::*;

impl Reporter {
    pub fn checked(&self, outcome: &scoop::CheckOutcome) -> io::Result<()> {
        self.diagnostics(&outcome.warnings, &outcome.sources)?;
        let purpose = match outcome.purpose {
            scoop_protocol::CompilationPurpose::Production => "production",
            scoop_protocol::CompilationPurpose::Test => "test",
        };
        match self.0 {
            MessageFormat::Human => {
                writeln!(io::stderr().lock(), "checked {} ({purpose})", outcome.root)
            }
            MessageFormat::Json => {
                let mut invocations = outcome
                    .compiled_dependencies
                    .iter()
                    .map(|id| {
                        json!({
                            "identity": id.to_string(), "operation": "artifact",
                        })
                    })
                    .collect::<Vec<_>>();
                if !outcome.cache_hit {
                    invocations.push(json!({"identity": outcome.root.identity().expect("checked coordinate has identity").to_string(), "operation": "check"}));
                }
                write_json(&json!({
                    "schema": 1, "kind": "check", "success": true,
                    "root": { "identity": outcome.root.identity().expect("checked coordinate has identity").to_string(),
                        "coordinate": { "group": outcome.root.group(), "name": outcome.root.name(), "version": outcome.root.version() } },
                    "target": outcome.target, "purpose": purpose, "cache_hit": outcome.cache_hit,
                    "observations": { "child_invocations": invocations },
                }))
            }
        }
    }
}
