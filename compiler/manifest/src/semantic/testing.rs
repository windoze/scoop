use super::*;

/// A dependency declaration keeps its locator and diagnostic spans together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDependency {
    pub(super) coordinate: ConeCoordinate,
    pub(super) locator: DependencyLocator,
    pub(super) spans: DependencyManifestSpans,
}

impl ManifestDependency {
    pub fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub fn locator(&self) -> &DependencyLocator {
        &self.locator
    }

    pub fn spans(&self) -> &DependencyManifestSpans {
        &self.spans
    }
}

/// Test-only inputs are excluded from the production semantic projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestManifestConfig {
    pub(super) sources: SourceSelection,
    pub(super) dependencies: BTreeMap<DependencyCoordinateKey, ManifestDependency>,
}

impl TestManifestConfig {
    pub fn sources(&self) -> &SourceSelection {
        &self.sources
    }

    pub fn dependency_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&DependencyCoordinateKey, &ManifestDependency)> {
        self.dependencies.iter()
    }
}
