//! Requested-kind validation and deterministic executable-entry selection.

use scoop_ast::{Diagnostic, DiagnosticNote, DiagnosticSeverity, Span};
use scoop_hir as hir;
use scoop_identity::{DefinitionOriginSubject, RequestedConeKind, SourceIdentity, SourceSpan};

/// Turn an unvalidated requested kind into the closed HIR output contract.
///
/// The library branch intentionally returns before inspecting declarations.
pub fn select_cone_output_kind(
    module: &hir::ExportHir,
    requested: RequestedConeKind,
) -> Result<hir::ConeOutputKind, Vec<Diagnostic>> {
    if requested == RequestedConeKind::Library {
        return Ok(hir::ConeOutputKind::Library);
    }

    let mut declarations = match current_main_declarations(module) {
        Ok(declarations) => declarations,
        Err(message) => return Err(vec![invalid_hir_diagnostic(message)]),
    };
    declarations.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));

    let valid = declarations
        .iter()
        .filter(|declaration| declaration.rejections.is_empty())
        .collect::<Vec<_>>();
    match valid.as_slice() {
        [declaration] => hir::LocalExecutableEntry::try_new(module, declaration.function)
            .map(|local_entry| hir::ConeOutputKind::Executable { local_entry })
            .map_err(|error| {
                vec![invalid_hir_diagnostic(format!(
                    "failed to seal executable entry: {error}"
                ))]
            }),
        [] => Err(vec![missing_entry_diagnostic(module, &declarations)]),
        [first, rest @ ..] => {
            let diagnostic = Diagnostic::at_file(
                first.file,
                first.span,
                "multiple executable entries: declare exactly one ordinary `fun main(): Unit`",
            );
            let diagnostic = rest.iter().fold(diagnostic, |diagnostic, declaration| {
                diagnostic.with_note(DiagnosticNote::at(
                    declaration.file,
                    declaration.span,
                    "another valid executable entry is declared here",
                ))
            });
            Err(vec![diagnostic])
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PersistentSourceLocation {
    source: SourceIdentity,
    span: SourceSpan,
    identity_kind: SourceFunctionIdentityKind,
    identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SourceFunctionIdentityKind {
    Plain,
    Generic,
}

struct MainDeclaration {
    function: hir::FunctionId,
    file: usize,
    span: Span,
    sort_key: PersistentSourceLocation,
    rejections: Vec<&'static str>,
}

fn current_main_declarations(module: &hir::ExportHir) -> Result<Vec<MainDeclaration>, String> {
    let mut declarations = Vec::new();
    for function_id in module.top_level.iter().copied() {
        let function = &module.functions[function_id];
        if function.name != "main" || function.method.is_some() {
            continue;
        }
        let Some(identity) = module.function_identities[function_id].source_identity() else {
            continue;
        };
        if identity.declaration().origin() != module.cone
            || !identity.declaration().owners().owners().is_empty()
        {
            continue;
        }

        let (subject, identity_kind, identity_bytes, generic) = match identity {
            hir::HirSourceFunctionIdentity::Plain(record) => (
                DefinitionOriginSubject::Function(record.id()),
                SourceFunctionIdentityKind::Plain,
                *record.id().as_array(),
                false,
            ),
            hir::HirSourceFunctionIdentity::Generic(record) => (
                DefinitionOriginSubject::GenericFunction(record.id()),
                SourceFunctionIdentityKind::Generic,
                *record.id().as_array(),
                true,
            ),
        };
        let origin = module
            .export_definition_origins
            .get(subject)
            .ok_or_else(|| "a current `main` declaration has no definition origin".to_string())?
            .origin();
        if origin.source().cone() != module.cone {
            return Err(
                "a current `main` declaration has a cross-Cone definition origin".to_string(),
            );
        }
        let file = module
            .source_files
            .iter()
            .position(|source| &source.identity == origin.source())
            .ok_or_else(|| {
                "a current `main` definition origin is absent from the HIR source table".to_string()
            })?;
        let span = diagnostic_span(origin.span())?;
        let mut rejections = Vec::new();
        if generic || !matches!(function.genericity, hir::FunctionGenericity::Plain) {
            rejections.push("it is generic");
        }
        if function.is_suspend {
            rejections.push("it is suspend");
        }
        if !function.params.is_empty() {
            rejections.push("it has parameters");
        }
        if function.return_ty != module.unit {
            rejections.push("it does not return `Unit`");
        }
        if !matches!(function.kind, hir::FunctionKind::User(_)) {
            rejections.push("it has no ordinary Scoop body");
        }
        declarations.push(MainDeclaration {
            function: function_id,
            file,
            span,
            sort_key: PersistentSourceLocation {
                source: origin.source().clone(),
                span: origin.span(),
                identity_kind,
                identity: identity_bytes,
            },
            rejections,
        });
    }
    Ok(declarations)
}

fn missing_entry_diagnostic(
    module: &hir::ExportHir,
    declarations: &[MainDeclaration],
) -> Diagnostic {
    let anchor = module
        .source_files
        .iter()
        .enumerate()
        .filter(|(_, source)| source.identity.cone() == module.cone)
        .min_by(|(_, left), (_, right)| left.identity.cmp(&right.identity));
    let diagnostic = match anchor {
        Some((file, _)) => Diagnostic::at_file(
            file,
            Span::new(0, 0),
            "missing executable entry: declare exactly one ordinary `fun main(): Unit`",
        ),
        None => Diagnostic::without_span(
            DiagnosticSeverity::Error,
            0,
            "missing executable entry: current Cone has no source in HIR",
        ),
    };
    declarations
        .iter()
        .filter(|declaration| !declaration.rejections.is_empty())
        .fold(diagnostic, |diagnostic, declaration| {
            diagnostic.with_note(DiagnosticNote::at(
                declaration.file,
                declaration.span,
                format!(
                    "`main` is not eligible because {}",
                    declaration.rejections.join(", ")
                ),
            ))
        })
}

fn diagnostic_span(span: SourceSpan) -> Result<Span, String> {
    let start = u32::try_from(span.start_byte())
        .map_err(|_| "a current `main` source span start exceeds the HIR range".to_string())?;
    let end = u32::try_from(span.end_byte())
        .map_err(|_| "a current `main` source span end exceeds the HIR range".to_string())?;
    Ok(Span::new(start, end))
}

fn invalid_hir_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::without_span(
        DiagnosticSeverity::Error,
        0,
        format!("invalid HIR executable-entry relation: {}", message.into()),
    )
}
