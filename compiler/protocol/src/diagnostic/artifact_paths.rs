use super::{DiagnosticOriginV1, HostPathCarrier, StructuredDiagnosticV1};

impl StructuredDiagnosticV1 {
    /// Replaces transport locators with retained artifact locators, preserving
    /// every semantic location and independently sourced note.
    pub fn remap_artifact_paths(
        &mut self,
        mut remap: impl FnMut(&HostPathCarrier) -> Option<HostPathCarrier>,
    ) {
        for origin in std::iter::once(&mut self.origin)
            .chain(self.notes.iter_mut().map(|note| &mut note.origin))
        {
            if let DiagnosticOriginV1::ArtifactPath { path, .. } = origin
                && let Some(original) = remap(path)
            {
                *path = original;
            }
        }
    }
}

#[cfg(test)]
mod tests;
