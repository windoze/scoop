use scoop_ast::{
    AllParsedSources, Diagnostic, DiagnosticSeverity, NonEmptyVec, ParsedSource, Stage1SourceHandle,
};

use crate::parser::parse_file;

/// One source selected by the caller for a non-empty M23-1 parse request.
/// The display locator is diagnostic-only and is not copied into successful
/// AST output.
#[derive(Debug, Clone, Copy)]
pub struct Stage1SourceInput<'a> {
    source_handle: Stage1SourceHandle,
    text: &'a str,
    display_locator: &'a str,
}

impl<'a> Stage1SourceInput<'a> {
    pub const fn new(
        source_handle: Stage1SourceHandle,
        text: &'a str,
        display_locator: &'a str,
    ) -> Self {
        Self {
            source_handle,
            text,
            display_locator,
        }
    }

    pub const fn source_handle(&self) -> Stage1SourceHandle {
        self.source_handle
    }

    pub const fn text(&self) -> &'a str {
        self.text
    }

    pub const fn display_locator(&self) -> &'a str {
        self.display_locator
    }
}

/// A parser or input-validation diagnostic decorated with transient display
/// information. Neither the locator nor this wrapper enters successful AST
/// data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseAllDiagnostic {
    source_handle: Stage1SourceHandle,
    display_locator: String,
    diagnostic: Diagnostic,
}

impl ParseAllDiagnostic {
    pub const fn source_handle(&self) -> Stage1SourceHandle {
        self.source_handle
    }

    pub fn display_locator(&self) -> &str {
        &self.display_locator
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_diagnostic(self) -> Diagnostic {
        self.diagnostic
    }
}

/// Parse every explicitly supplied source. All sources are visited even when
/// one fails; a successful non-empty AST set is returned only when input
/// handles and every source are valid.
pub fn parse_all(
    inputs: NonEmptyVec<Stage1SourceInput<'_>>,
) -> Result<AllParsedSources, Vec<ParseAllDiagnostic>> {
    let request = inputs.first().source_handle().request();
    let mut seen_handles = Vec::with_capacity(inputs.len());
    let mut parsed = Vec::with_capacity(inputs.len());
    let mut diagnostics = Vec::new();

    for (source_index, input) in inputs.iter().enumerate() {
        let handle = input.source_handle();
        if handle.request() != request {
            diagnostics.push(input_error(
                input,
                source_index,
                format!(
                    "source handle belongs to stage-1 request {}, expected request {}",
                    handle.request().into_raw(),
                    request.into_raw()
                ),
            ));
        }
        if let Some(first_index) = seen_handles.iter().position(|seen| *seen == handle) {
            diagnostics.push(input_error(
                input,
                source_index,
                format!(
                    "duplicate stage-1 source handle {} (first used by source {first_index})",
                    handle.local_index()
                ),
            ));
        } else {
            seen_handles.push(handle);
        }

        match parse_file(input.text()) {
            Ok(ast) => parsed.push(ParsedSource::new(handle, ast)),
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics.into_iter().map(|mut diagnostic| {
                    diagnostic.file = source_index;
                    ParseAllDiagnostic {
                        source_handle: handle,
                        display_locator: input.display_locator().to_string(),
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
    Ok(AllParsedSources::try_new(request, sources)
        .expect("parse-all validates request membership and unique handles before construction"))
}

fn input_error(
    input: &Stage1SourceInput<'_>,
    source_index: usize,
    message: String,
) -> ParseAllDiagnostic {
    ParseAllDiagnostic {
        source_handle: input.source_handle(),
        display_locator: input.display_locator().to_string(),
        diagnostic: Diagnostic {
            severity: DiagnosticSeverity::Error,
            file: source_index,
            span: None,
            message,
        },
    }
}
