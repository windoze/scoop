use super::*;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

mod error;
pub use error::DocumentationProductionError;
type Result<T> = std::result::Result<T, DocumentationProductionError>;

pub(super) struct PendingDocumentation {
    destination: PathBuf,
    input: Vec<u8>,
    warnings: CurrentConeDiagnosticSet,
}

impl PendingDocumentation {
    pub(super) fn prepare(
        request: &ParsedSingleConeBuildRequest<'_, '_>,
        hir: &current_hir::CurrentConeHirArtifacts,
        coordinate: &ConeCoordinate,
        kind: scoop_identity::RequestedConeKind,
    ) -> Result<Option<Self>> {
        let Some(destination) = request.request.output().documentation() else {
            return Ok(None);
        };
        let output = hir.hir.output();
        let input = scoop_doc::project_cone(
            &output.export,
            output
                .documentation
                .as_ref()
                .expect("the request selected documentation"),
            coordinate,
            kind,
        )
        .map_err(DocumentationProductionError::Projection)?;
        let sources = request
            .sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                Ok((
                    scoop_doc::SourceKey::new(source.source().identity())
                        .map_err(DocumentationProductionError::Identity)?,
                    (index, source.source().identity().clone()),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        let diagnostics = input
            .warnings
            .iter()
            .map(|warning| {
                let (file, source) = &sources[&warning.location.source];
                let mut diagnostic = scoop_ast::Diagnostic::warning_at_file(
                    *file,
                    span(warning.location.range),
                    &warning.message,
                );
                diagnostic.source = Some(Box::new(source.clone()));
                diagnostic.notes = warning
                    .notes
                    .iter()
                    .map(|note| {
                        let (file, source) = &sources[&note.location.source];
                        scoop_ast::DiagnosticNote {
                            file: *file,
                            source: Some(Box::new(source.clone())),
                            span: span(note.location.range),
                            message: note.message.clone(),
                        }
                    })
                    .collect();
                diagnostic
            })
            .collect();
        Ok(Some(Self {
            destination: destination.as_path().into(),
            input: input
                .to_json()
                .map_err(|error| DocumentationProductionError::Encoding(error.to_string()))?,
            warnings: CurrentConeDiagnosticSet::try_new(diagnostics, &request.sources)
                .map_err(DocumentationProductionError::Diagnostics)?,
        }))
    }

    pub(super) fn publish(self) -> Result<crate::PublishedDocumentation> {
        let publish = || -> std::io::Result<()> {
            let parent = self
                .destination
                .parent()
                .expect("documentation paths are absolute");
            std::fs::create_dir_all(parent)?;
            let mut temporary = tempfile::Builder::new()
                .prefix(".scoop-doc-")
                .tempfile_in(parent)?;
            temporary.write_all(&self.input)?;
            temporary
                .persist(&self.destination)
                .map_err(|error| error.error)?;
            Ok(())
        };
        publish().map_err(|source| DocumentationProductionError::Publication {
            path: self.destination.clone(),
            source,
        })?;
        Ok(crate::PublishedDocumentation::new(
            self.destination,
            self.warnings,
        ))
    }
}

fn span(range: scoop_doc::TextRange) -> scoop_ast::Span {
    scoop_ast::Span::new(range.start, range.end)
}
