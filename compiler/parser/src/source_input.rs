use std::collections::BTreeMap;

use scoop_ast::{AllParsedSources, Diagnostic, IdentifiedParsedSource, NonEmptyVec};
use scoop_identity::SourceIdentity;

use crate::parser::parse_file;

/// One source selected by the caller with an already validated semantic
/// identity. Display paths are deliberately absent from this stable input.
#[derive(Debug, Clone, Copy)]
pub struct IdentifiedSourceInput<'a> {
    identity: &'a SourceIdentity,
    text: &'a str,
}

impl<'a> IdentifiedSourceInput<'a> {
    pub const fn new(identity: &'a SourceIdentity, text: &'a str) -> Self {
        Self { identity, text }
    }

    pub const fn identity(&self) -> &'a SourceIdentity {
        self.identity
    }

    pub const fn text(&self) -> &'a str {
        self.text
    }
}

/// Request-local diagnostic decoration keyed by semantic source identity.
///
/// This sidecar is never copied into successful AST output or any persistent
/// key. Two identities may intentionally share the same display locator.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParserDiagnosticContext {
    display_locators: BTreeMap<SourceIdentity, String>,
}

impl ParserDiagnosticContext {
    pub fn new(display_locators: impl IntoIterator<Item = (SourceIdentity, String)>) -> Self {
        Self {
            display_locators: display_locators.into_iter().collect(),
        }
    }

    pub fn display_locator(&self, identity: &SourceIdentity) -> Option<&str> {
        self.display_locators.get(identity).map(String::as_str)
    }
}

/// A parser or input-validation diagnostic attributed by semantic source
/// identity. A caller may obtain its transient display path from a
/// [`ParserDiagnosticContext`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseAllDiagnostic {
    identity: SourceIdentity,
    diagnostic: Diagnostic,
}

impl ParseAllDiagnostic {
    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_diagnostic(self) -> Diagnostic {
        self.diagnostic
    }
}

/// Parse every explicitly supplied source. All sources are visited even when
/// one fails; a successful non-empty AST set is returned only when identities
/// are unique and every source parses successfully.
pub fn parse_all(
    inputs: NonEmptyVec<IdentifiedSourceInput<'_>>,
) -> Result<AllParsedSources, Vec<ParseAllDiagnostic>> {
    let mut seen_identities = Vec::with_capacity(inputs.len());
    let mut parsed = Vec::with_capacity(inputs.len());
    let mut diagnostics = Vec::new();

    for (source_index, input) in inputs.iter().enumerate() {
        let identity = input.identity();
        if let Some((_, first_index)) = seen_identities
            .iter()
            .find(|(seen, _): &&(SourceIdentity, usize)| seen == identity)
        {
            diagnostics.push(input_error(
                identity,
                source_index,
                format!(
                    "duplicate source identity {}/{} (first used by source {first_index})",
                    identity.cone(),
                    identity.logical_path()
                ),
            ));
        } else {
            seen_identities.push((identity.clone(), source_index));
        }

        match parse_file(input.text()) {
            Ok(ast) => parsed.push(IdentifiedParsedSource::new(identity.clone(), ast)),
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics.into_iter().map(|mut diagnostic| {
                    diagnostic.reattribute_single_source(source_index);
                    ParseAllDiagnostic {
                        identity: identity.clone(),
                        diagnostic,
                    }
                }));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut parsed = parsed.into_iter();
    let first = parsed
        .next()
        .expect("a non-empty successful input produces a first parsed source");
    let sources = NonEmptyVec::new(first, parsed.collect());
    Ok(AllParsedSources::try_new(sources)
        .expect("parse-all validates unique identities before construction"))
}

fn input_error(
    identity: &SourceIdentity,
    source_index: usize,
    message: String,
) -> ParseAllDiagnostic {
    ParseAllDiagnostic {
        identity: identity.clone(),
        diagnostic: Diagnostic::without_span(
            scoop_ast::DiagnosticSeverity::Error,
            source_index,
            message,
        ),
    }
}
