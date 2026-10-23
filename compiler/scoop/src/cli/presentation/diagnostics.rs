use std::collections::BTreeMap;
use std::path::PathBuf;

use scoop::SelectedSource;
use scoop_protocol::{DiagnosticOriginV1, DiagnosticSeverityV1, StructuredDiagnosticV1};
use scoop_source::SourceText;

use super::*;

type SourceContents = BTreeMap<PathBuf, Option<SourceText>>;

impl Reporter {
    pub(super) fn diagnostics(
        &self,
        diagnostics: &[StructuredDiagnosticV1],
        sources: &[SelectedSource],
    ) -> io::Result<()> {
        let mut contents = SourceContents::new();
        for diagnostic in diagnostics {
            let severity = match diagnostic.severity() {
                DiagnosticSeverityV1::Error => "error",
                DiagnosticSeverityV1::Warning => "warning",
            };
            match self.0 {
                MessageFormat::Json => {
                    let notes = diagnostic.notes().iter().map(|note| json!({
                        "message": note.message(), "origin": origin(note.origin()), "display": display(note.origin(), sources, &mut contents),
                    })).collect::<Vec<_>>();
                    write_json(&json!({
                        "schema": 1, "kind": "diagnostic", "severity": severity, "code": diagnostic.code(),
                        "message": diagnostic.message(), "origin": origin(diagnostic.origin()), "notes": notes,
                        "display": display(diagnostic.origin(), sources, &mut contents),
                    }))?;
                }
                MessageFormat::Human => {
                    let mut stderr = io::stderr().lock();
                    writeln!(
                        stderr,
                        "{}{}[{}]: {}",
                        location(diagnostic.origin(), sources, &mut contents),
                        severity,
                        diagnostic.code(),
                        diagnostic.message()
                    )?;
                    for note in diagnostic.notes() {
                        writeln!(
                            stderr,
                            "{}note: {}",
                            location(note.origin(), sources, &mut contents),
                            note.message()
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn origin(origin: &DiagnosticOriginV1) -> Value {
    match origin {
        DiagnosticOriginV1::None => json!({"kind": "none"}),
        DiagnosticOriginV1::SemanticSourceSpan {
            cone,
            logical_path,
            span,
        }
        | DiagnosticOriginV1::SourceTextSpan {
            cone,
            logical_path,
            span,
            ..
        } => json!({
            "kind": "source", "cone": scoop_wire::Digest256::from_array(*cone.as_array()).to_string(), "path": logical_path.as_str(), "start": span.start(), "end": span.end(),
        }),
        DiagnosticOriginV1::HostPath { path } => json!({
            "kind": "host", "path": path.to_path_buf().ok().map(|path| path_value(&path)), "start": null, "end": null,
        }),
        DiagnosticOriginV1::HostPathSpan { path, span } => json!({
            "kind": "host", "path": path.to_path_buf().ok().map(|path| path_value(&path)), "start": span.start(), "end": span.end(),
        }),
        DiagnosticOriginV1::ArtifactPath {
            path,
            semantic_path,
        } => json!({
            "kind": "artifact", "path": path.to_path_buf().ok().map(|path| path_value(&path)), "member": semantic_path,
        }),
    }
}

fn display(
    origin: &DiagnosticOriginV1,
    sources: &[SelectedSource],
    contents: &mut SourceContents,
) -> Value {
    match origin {
        DiagnosticOriginV1::SourceTextSpan {
            logical_path,
            span,
            text,
            ..
        } => {
            let mut value =
                json!({"path": logical_path.as_str(), "start": span.start(), "end": span.end()});
            add_position(&mut value, &SourceText::new(text.clone()), span.start());
            value
        }
        DiagnosticOriginV1::None => Value::Null,
        DiagnosticOriginV1::SemanticSourceSpan {
            cone,
            logical_path,
            span,
        } => {
            let Some(source) = sources.iter().find(|source| {
                source.identity().cone().as_array() == cone.as_array()
                    && source.identity().logical_path() == logical_path
            }) else {
                return Value::Null;
            };
            let mut value = json!({"path": path_value(source.display_locator().as_path()), "start": span.start(), "end": span.end()});
            let text = contents
                .entry(source.physical_path().to_owned())
                .or_insert_with(|| {
                    std::fs::read_to_string(source.physical_path())
                        .ok()
                        .map(SourceText::new)
                });
            if let Some(text) = text {
                add_position(&mut value, text, span.start());
            }
            value
        }
        DiagnosticOriginV1::HostPath { path } => {
            json!({"path": path.to_path_buf().ok().map(|path| path_value(&path))})
        }
        DiagnosticOriginV1::HostPathSpan { path, span } => {
            json!({"path": path.to_path_buf().ok().map(|path| path_value(&path)), "start": span.start(), "end": span.end()})
        }
        DiagnosticOriginV1::ArtifactPath {
            path,
            semantic_path,
        } => {
            json!({"path": path.to_path_buf().ok().map(|path| path_value(&path)), "member": semantic_path})
        }
    }
}

fn add_position(value: &mut Value, text: &SourceText, offset: u64) {
    if let Ok(offset) = u32::try_from(offset)
        && text.as_str().get(..offset as usize).is_some()
    {
        let (line, column) = text.diagnostic_position(offset);
        value["line"] = json!(line);
        value["column"] = json!(column);
    }
}

fn location(
    origin: &DiagnosticOriginV1,
    sources: &[SelectedSource],
    contents: &mut SourceContents,
) -> String {
    let rendered = display(origin, sources, contents);
    if let Some(path) = rendered.get("path").and_then(Value::as_str) {
        if let (Some(line), Some(column)) = (rendered.get("line"), rendered.get("column")) {
            return format!("{path}:{line}:{column}: ");
        }
        if let (Some(start), Some(end)) = (rendered.get("start"), rendered.get("end")) {
            return format!("{path}:bytes {start}..{end}: ");
        }
        if let Some(member) = rendered.get("member").and_then(Value::as_str) {
            return format!("{path}:{member}: ");
        }
        return format!("{path}: ");
    }
    match origin {
        DiagnosticOriginV1::SemanticSourceSpan {
            cone,
            logical_path,
            span,
        }
        | DiagnosticOriginV1::SourceTextSpan {
            cone,
            logical_path,
            span,
            ..
        } => format!(
            "{}/{logical_path}:bytes {}..{}: ",
            scoop_wire::Digest256::from_array(*cone.as_array()),
            span.start(),
            span.end()
        ),
        DiagnosticOriginV1::HostPath { path } => format!(
            "{}: ",
            path.to_path_buf().map_or_else(
                |_| "<host path>".to_owned(),
                |path| path.display().to_string(),
            )
        ),
        DiagnosticOriginV1::HostPathSpan { path, span } => format!(
            "{}:bytes {}..{}: ",
            path.to_path_buf().map_or_else(
                |_| "<host path>".to_owned(),
                |path| path.display().to_string()
            ),
            span.start(),
            span.end()
        ),
        DiagnosticOriginV1::ArtifactPath {
            path,
            semantic_path,
        } => format!(
            "{}:{semantic_path}: ",
            path.to_path_buf().map_or_else(
                |_| "<artifact>".to_owned(),
                |path| path.display().to_string()
            )
        ),
        DiagnosticOriginV1::None => String::new(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn artifact_location_keeps_the_member_for_native_path_encodings() {
        for bytes in [b"a.slib".to_vec(), b"a-\xff.slib".to_vec()] {
            let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(bytes));
            let diagnostic = DiagnosticOriginV1::artifact_path(
                scoop_protocol::HostPathCarrier::from_path(&path).unwrap(),
                "HIR:$.3[2]".to_owned(),
            )
            .unwrap();
            assert_eq!(
                location(&diagnostic, &[], &mut SourceContents::new()),
                format!("{}:HIR:$.3[2]: ", path.display())
            );
            assert_eq!(origin(&diagnostic)["path"], path_value(&path));
            assert_eq!(
                display(&diagnostic, &[], &mut SourceContents::new())["member"],
                "HIR:$.3[2]"
            );
        }
    }
}
