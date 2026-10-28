use std::{
    fmt,
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestOrigin {
    pub manifest: PathBuf,
    pub span: Range<usize>,
}

#[derive(Debug)]
pub struct PmError {
    pub message: String,
    pub origins: Vec<ManifestOrigin>,
    pub(crate) resolution_conflicts: Vec<scoop_manifest::DependencyCoordinateKey>,
}

pub type PmResult<T> = Result<T, PmError>;

impl PmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            origins: Vec::new(),
            resolution_conflicts: Vec::new(),
        }
    }

    pub fn at(mut self, origin: ManifestOrigin) -> Self {
        self.origins.push(origin);
        self
    }

    pub(crate) fn io(path: &Path, error: std::io::Error) -> Self {
        Self::new(format!("{}: {error}", path.display()))
    }

    pub fn machine_diagnostic(
        &self,
    ) -> Result<scoop_protocol::package::PackageDiagnosticV1, String> {
        use scoop_protocol::{
            DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticSeverityV1, HostPathCarrier,
            ProtocolByteSpan, StructuredDiagnosticV1,
        };
        let mut origins = self.origins.iter().map(|origin| -> Result<_, String> {
            Ok(DiagnosticOriginV1::HostPathSpan {
                path: HostPathCarrier::from_path(&origin.manifest).map_err(|e| e.to_string())?,
                span: ProtocolByteSpan::new(
                    origin
                        .span
                        .start
                        .try_into()
                        .map_err(|e: std::num::TryFromIntError| e.to_string())?,
                    origin
                        .span
                        .end
                        .try_into()
                        .map_err(|e: std::num::TryFromIntError| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            })
        });
        let origin = origins
            .next()
            .transpose()?
            .unwrap_or(DiagnosticOriginV1::None);
        let notes = origins
            .map(|origin| {
                DiagnosticNoteV1::new("related declaration".into(), origin?)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<_, String>>()?;
        Ok(scoop_protocol::package::PackageDiagnosticV1 {
            version: scoop_protocol::package::PACKAGE_PROTOCOL_VERSION,
            diagnostic: StructuredDiagnosticV1::new(
                DiagnosticSeverityV1::Error,
                "SCOOP_PACKAGE_PREPARE".into(),
                self.message.clone(),
                origin,
                notes,
            )
            .map_err(|e| e.to_string())?,
        })
    }
}

impl fmt::Display for PmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        for origin in &self.origins {
            write!(
                f,
                "\n  at {} (bytes {}..{})",
                origin.manifest.display(),
                origin.span.start,
                origin.span.end
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for PmError {}
