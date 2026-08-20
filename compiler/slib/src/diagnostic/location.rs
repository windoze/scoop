use super::{SlibDiagnosticRecord, SlibPrimaryOrigin};

impl SlibDiagnosticRecord {
    /// Preserve the typed origin and wire path for an artifact diagnostic.
    pub fn semantic_path(&self) -> String {
        let origin = match self.primary_origin() {
            None => return self.path().to_string(),
            Some(SlibPrimaryOrigin::Container) => "container".to_owned(),
            Some(SlibPrimaryOrigin::Manifest) => "manifest".to_owned(),
            Some(SlibPrimaryOrigin::Metadata(location)) => location.to_string(),
            Some(SlibPrimaryOrigin::Member(member)) => format!("member/{member}"),
            Some(SlibPrimaryOrigin::Cone(cone)) => format!("cone/{cone}"),
            Some(SlibPrimaryOrigin::Capability {
                location,
                capability,
            }) => {
                let location = location.map_or_else(|| "manifest".to_owned(), |it| it.to_string());
                format!(
                    "{location}/{}/{}/{}",
                    capability.namespace(),
                    capability.name(),
                    capability.major_version()
                )
            }
            Some(SlibPrimaryOrigin::Identity { kind, id }) => {
                format!("{kind}/{}", scoop_wire::Digest256::from_array(*id))
            }
        };
        format!("{origin}:{}", self.path())
    }
}
