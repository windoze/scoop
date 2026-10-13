use scoop_identity::ConeCoordinate;
use scoop_manifest::{SourceSelection, TestManifestConfig};
use scoop_wire::{Encoder, WireEncode};

/// Test semantic inputs exclude host locators and diagnostic spans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestCompileConfiguration {
    sources: SourceSelection,
    dependencies: Vec<ConeCoordinate>,
}

impl TestCompileConfiguration {
    pub(crate) fn new(config: &TestManifestConfig) -> Self {
        let mut dependencies: Vec<_> = config
            .dependency_iter()
            .map(|(_, declaration)| declaration.coordinate().clone())
            .collect();
        dependencies.sort_by(crate::discovery::compare_coordinates);
        Self {
            sources: config.sources().clone(),
            dependencies,
        }
    }
}

impl WireEncode for TestCompileConfiguration {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.sources.encode(encoder)?;
        encoder.field(2)?;
        super::encode_array(&self.dependencies, encoder)
    }
}
