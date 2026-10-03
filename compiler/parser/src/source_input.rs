use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentConeParsedSourcesError,
    CurrentSourceDiagnosticContext, CurrentSourceText, Diagnostic, IdentifiedParsedSource,
    NonEmptyVec,
};
use scoop_identity::{ConeIdentity, SourceIdentity};

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

/// One canonical current-Cone parser input with its request-local display
/// locator. The locator decorates diagnostics but has no semantic role.
#[derive(Debug, Clone, Copy)]
pub struct CurrentConeSourceInput<'a> {
    identity: &'a SourceIdentity,
    text: &'a str,
    display_locator: &'a Path,
}

impl<'a> CurrentConeSourceInput<'a> {
    pub const fn new(
        identity: &'a SourceIdentity,
        text: &'a str,
        display_locator: &'a Path,
    ) -> Self {
        Self {
            identity,
            text,
            display_locator,
        }
    }

    pub const fn identity(&self) -> &'a SourceIdentity {
        self.identity
    }

    pub const fn text(&self) -> &'a str {
        self.text
    }

    pub const fn display_locator(&self) -> &'a Path {
        self.display_locator
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

/// Parse one complete, canonically ordered current-Cone source set.
///
/// Input shape is rejected before lexing. The success type owns the exact
/// source text and diagnostic locator sidecars and can only be constructed
/// after every source parses successfully.
pub fn parse_current_cone(
    inputs: NonEmptyVec<CurrentConeSourceInput<'_>>,
) -> Result<CurrentConeParsedSources, ParseCurrentConeError> {
    validate_current_cone_inputs(&inputs).map_err(ParseCurrentConeError::Input)?;

    let first = inputs.first();
    let source_texts = NonEmptyVec::new(
        CurrentSourceText::new(first.identity().clone(), first.text().to_owned()),
        inputs
            .iter()
            .skip(1)
            .map(|input| CurrentSourceText::new(input.identity().clone(), input.text().to_owned()))
            .collect(),
    );
    let diagnostic_context = NonEmptyVec::new(
        CurrentSourceDiagnosticContext::new(
            first.identity().clone(),
            first.display_locator().to_path_buf(),
        ),
        inputs
            .iter()
            .skip(1)
            .map(|input| {
                CurrentSourceDiagnosticContext::new(
                    input.identity().clone(),
                    input.display_locator().to_path_buf(),
                )
            })
            .collect(),
    );
    let parser_inputs = NonEmptyVec::new(
        IdentifiedSourceInput::new(first.identity(), first.text()),
        inputs
            .iter()
            .skip(1)
            .map(|input| IdentifiedSourceInput::new(input.identity(), input.text()))
            .collect(),
    );
    let parsed = parse_all(parser_inputs).map_err(ParseCurrentConeError::Diagnostics)?;
    CurrentConeParsedSources::try_new(parsed, source_texts, diagnostic_context)
        .map_err(ParseCurrentConeError::Output)
}

fn validate_current_cone_inputs(
    inputs: &NonEmptyVec<CurrentConeSourceInput<'_>>,
) -> Result<(), ParseCurrentConeInputError> {
    let expected = inputs.first().identity().cone();
    for (index, input) in inputs.iter().enumerate() {
        if input.identity().cone() != expected {
            return Err(ParseCurrentConeInputError::MixedCone {
                index,
                expected,
                actual: input.identity().cone(),
            });
        }
    }
    for (index, pair) in inputs.as_slice().windows(2).enumerate() {
        if pair[0].identity() >= pair[1].identity() {
            return Err(ParseCurrentConeInputError::NonIncreasingSourceIdentity {
                first_index: index,
                second_index: index + 1,
                first: Box::new(pair[0].identity().clone()),
                second: Box::new(pair[1].identity().clone()),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCurrentConeInputError {
    MixedCone {
        index: usize,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    NonIncreasingSourceIdentity {
        first_index: usize,
        second_index: usize,
        first: Box<SourceIdentity>,
        second: Box<SourceIdentity>,
    },
}

impl fmt::Display for ParseCurrentConeInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MixedCone {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "source {index} belongs to Cone {actual}, expected current Cone {expected}"
            ),
            Self::NonIncreasingSourceIdentity {
                first_index,
                second_index,
                first,
                second,
            } => write!(
                formatter,
                "source identities are not strictly increasing at indexes {first_index} and {second_index}: {}/{} then {}/{}",
                first.cone(),
                first.logical_path(),
                second.cone(),
                second.logical_path(),
            ),
        }
    }
}

impl std::error::Error for ParseCurrentConeInputError {}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseCurrentConeError {
    Input(ParseCurrentConeInputError),
    Diagnostics(Vec<ParseAllDiagnostic>),
    Output(CurrentConeParsedSourcesError),
}

impl fmt::Display for ParseCurrentConeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Diagnostics(diagnostics) => {
                write!(formatter, "{} parser diagnostic(s)", diagnostics.len())
            }
            Self::Output(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParseCurrentConeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Diagnostics(_) => None,
        }
    }
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

#[cfg(test)]
mod current_cone_tests {
    use std::path::PathBuf;

    use scoop_identity::{ConeCoordinate, NormalizedSourcePath};

    use super::*;

    fn identity(cone_name: &str, path: &str) -> SourceIdentity {
        let cone = ConeCoordinate::new("test", cone_name, "0.0.0")
            .unwrap()
            .identity()
            .unwrap();
        SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
    }

    #[test]
    fn parse_current_cone_returns_complete_owned_sidecars() {
        let first = identity("current", "src/first.scoop");
        let second = identity("current", "src/second.scoop");
        let first_path = PathBuf::from("checkout/src/first.scoop");
        let second_path = PathBuf::from("checkout/src/second.scoop");
        let parsed = parse_current_cone(NonEmptyVec::new(
            CurrentConeSourceInput::new(&first, "package first", &first_path),
            vec![CurrentConeSourceInput::new(
                &second,
                "package second\nfun main() {}",
                &second_path,
            )],
        ))
        .unwrap();

        assert_eq!(parsed.cone(), first.cone());
        assert_eq!(parsed.sources().sources().len(), 2);
        assert_eq!(
            parsed.source_texts().get(&second).unwrap().text(),
            "package second\nfun main() {}"
        );
        assert_eq!(
            parsed.diagnostic_context().display_locator(&first),
            Some(first_path.as_path())
        );
    }

    #[test]
    fn parse_current_cone_rejects_shape_before_parsing() {
        let later = identity("current", "src/z.scoop");
        let earlier = identity("current", "src/a.scoop");
        let path = PathBuf::from("source.scoop");
        let error = parse_current_cone(NonEmptyVec::new(
            CurrentConeSourceInput::new(&later, "$", &path),
            vec![CurrentConeSourceInput::new(&earlier, "$", &path)],
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ParseCurrentConeError::Input(
                ParseCurrentConeInputError::NonIncreasingSourceIdentity { .. }
            )
        ));

        let foreign = identity("foreign", "src/foreign.scoop");
        let error = parse_current_cone(NonEmptyVec::new(
            CurrentConeSourceInput::new(&earlier, "$", &path),
            vec![CurrentConeSourceInput::new(&foreign, "$", &path)],
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ParseCurrentConeError::Input(ParseCurrentConeInputError::MixedCone { index: 1, .. })
        ));
    }

    #[test]
    fn parse_current_cone_discards_all_asts_when_one_source_fails() {
        let first = identity("current", "src/first.scoop");
        let second = identity("current", "src/second.scoop");
        let path = PathBuf::from("source.scoop");
        let error = parse_current_cone(NonEmptyVec::new(
            CurrentConeSourceInput::new(&first, "fun valid() {}", &path),
            vec![CurrentConeSourceInput::new(&second, "import .bad", &path)],
        ))
        .unwrap_err();
        let ParseCurrentConeError::Diagnostics(diagnostics) = error else {
            panic!("expected parser diagnostics")
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].identity(), &second);
        assert_eq!(diagnostics[0].diagnostic().file, 1);
    }
}
