//! Strict source dependencies reuse the parse held by their input revision.

use std::rc::Rc;

use scoop_ast as ast;

use super::ProjectAnalysisError as Error;
use crate::{AnalysisSnapshot, DeclarationAnalysisError, SourceConeInput};

pub(super) fn complete(
    snapshot: &AnalysisSnapshot,
    cone: &SourceConeInput,
    check: &Rc<dyn Fn() -> bool>,
) -> Result<
    (
        ast::CurrentConeParsedSources,
        Vec<ast::IdentifiedSourceDocumentation>,
    ),
    Error,
> {
    let mut sources = Vec::with_capacity(cone.sources.len());
    let mut texts = Vec::with_capacity(cone.sources.len());
    let mut contexts = Vec::with_capacity(cone.sources.len());
    let mut diagnostics = Vec::new();
    let mut documentation = Vec::with_capacity(cone.sources.len());
    for file in &cone.sources {
        if check() {
            return Err(Error::Cancelled);
        }
        let source = snapshot
            .source(file.source)?
            .ok_or(Error::MissingSource(file.source))?;
        let parsed = source
            .parse()
            .map_err(|error| Error::Declarations(DeclarationAnalysisError::SourceLength(*error)))?;
        let Some(syntax) = parsed.ast() else {
            diagnostics.extend(parsed.diagnostics().iter().cloned().map(|mut diagnostic| {
                diagnostic.resolve_sources(std::slice::from_ref(&file.identity));
                diagnostic
            }));
            continue;
        };
        sources.push(
            ast::IdentifiedParsedSource::new(file.identity.clone(), syntax.clone())
                .with_role(file.role),
        );
        texts.push(ast::CurrentSourceText::new(
            file.identity.clone(),
            source.text.as_str().to_owned(),
        ));
        contexts.push(ast::CurrentSourceDiagnosticContext::new(
            file.identity.clone(),
            file.path.clone(),
        ));
        documentation.push(ast::IdentifiedSourceDocumentation {
            source: file.identity.clone(),
            documentation: source
                .documentation()
                .map_err(|error| {
                    Error::Declarations(DeclarationAnalysisError::SourceLength(*error))
                })?
                .clone(),
        });
    }
    if cone.sources.is_empty() {
        diagnostics.push(ast::Diagnostic::without_span(
            ast::DiagnosticSeverity::Error,
            0,
            "no source files to compile",
        ));
    }
    if !diagnostics.is_empty() {
        return Err(Error::SourceDependencyUnavailable {
            cone: cone.manifest.identity(),
            diagnostics,
        });
    }
    let sources = ast::AllParsedSources::try_new(nonempty(sources))
        .expect("selected source identities are unique");
    Ok((
        ast::CurrentConeParsedSources::try_new(sources, nonempty(texts), nonempty(contexts))
            .expect("the host preserves selected source order and sidecar coverage"),
        documentation,
    ))
}

fn nonempty<T>(values: Vec<T>) -> ast::NonEmptyVec<T> {
    let mut values = values.into_iter();
    ast::NonEmptyVec::new(
        values.next().expect("the source set is nonempty"),
        values.collect(),
    )
}
