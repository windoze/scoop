//! Manifest namespace checks and single-file source warnings.
use scoop_ast::{CurrentConeParsedSources, Diagnostic, PackageSyntax, Span};
use scoop_identity::ConeIdentity;
use scoop_manifest::LoadedConeManifest;

use super::{CurrentConeDiagnosticSet, CurrentConeSourceStageError};

#[cfg(test)]
mod tests;

pub(super) fn validate(
    manifest: &LoadedConeManifest,
    sources: &CurrentConeParsedSources,
) -> Result<(), CurrentConeSourceStageError> {
    let coordinate = manifest.coordinate();
    if coordinate.group() == "scoop" && matches!(coordinate.name(), "scoop.core" | "scoop.std") {
        return Ok(());
    }
    let mut diagnostics = Vec::new();
    for (file, source) in sources.iter().enumerate() {
        let ast = source.source().ast();
        let (span, message) = match &ast.package {
            PackageSyntax::RootPackage => (
                Span::new(ast.span.start, ast.span.start),
                format!(
                    "source file must declare a package equal to Cone group `{}` or one of its subpackages",
                    coordinate.group()
                ),
            ),
            PackageSyntax::QualifiedPackage { path, span, .. } => {
                let mut segments = path.segments();
                if coordinate.group().split('.').all(|expected| {
                    segments
                        .next()
                        .is_some_and(|actual| actual.text == expected)
                }) {
                    continue;
                }
                (
                    *span,
                    format!(
                        "package `{path}` must equal Cone group `{}` or one of its subpackages",
                        coordinate.group()
                    ),
                )
            }
        };
        diagnostics.push(Diagnostic::at_file(file, span, message));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(CurrentConeSourceStageError::Package(Box::new(
            CurrentConeDiagnosticSet::try_new(diagnostics, sources)
                .expect("package diagnostics use the enumerated source indices"),
        )))
    }
}

pub(super) fn single_file_warnings(sources: &CurrentConeParsedSources) -> Vec<Diagnostic> {
    if sources.cone() != ConeIdentity::SINGLE_FILE {
        return Vec::new();
    }
    sources.iter().enumerate().filter_map(|(file, source)| {
        let PackageSyntax::QualifiedPackage { span, .. } = &source.source().ast().package else {
            return None;
        };
        Some(Diagnostic::warning_at_file(file, *span,
            "package declaration is ignored in single-file mode; using the default root package"))
    }).collect()
}
