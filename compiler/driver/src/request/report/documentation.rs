use super::*;

#[derive(Debug)]
pub struct PublishedDocumentation {
    path: std::path::PathBuf,
    warnings: CurrentConeDiagnosticSet,
}

impl PublishedDocumentation {
    pub(in crate::request) fn new(
        path: std::path::PathBuf,
        warnings: CurrentConeDiagnosticSet,
    ) -> Self {
        Self { path, warnings }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn warnings(&self) -> &CurrentConeDiagnosticSet {
        &self.warnings
    }
}

#[derive(Debug)]
pub struct SingleConeDocumentationSuccess {
    pub(in crate::request) documentation: PublishedDocumentation,
    pub(in crate::request) warnings: CurrentConeDiagnosticSet,
    pub(in crate::request) dumps: Vec<EmittedStageDump>,
    pub(in crate::request) used_generators: Vec<scoop_protocol::GeneratorToolInputV1>,
}

impl SingleConeDocumentationSuccess {
    pub fn used_generators(&self) -> &[scoop_protocol::GeneratorToolInputV1] {
        &self.used_generators
    }
    pub fn documentation(&self) -> &PublishedDocumentation {
        &self.documentation
    }
}
