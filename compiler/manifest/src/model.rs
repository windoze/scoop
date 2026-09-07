//! Parsed manifest model: one `Cone.toml` yields the semantic projection
//! plus the locator projection.

use scoop_identity::ConeCoordinate;

/// Product kind of a Cone. An executable cannot appear as another Cone's
/// dependency; enforcement happens during graph resolution, where both
/// endpoints' manifests are known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConeKind {
    Library,
    Executable,
}

impl ConeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ConeKind::Library => "library",
            ConeKind::Executable => "executable",
        }
    }
}

/// How the build tool locates the artifact or sources of a declared
/// dependency. Locators are resolution hints only: they are never written
/// into `.slib` metadata, stable keys or cache semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyLocator {
    /// No locator: `scoop` searches `--cone-path` roots for
    /// `<group>/<name>/<version>/cone.slib`.
    Search,
    /// `path = "..."` — a source Cone root, relative to this manifest.
    SourcePath { path: String },
    /// `artifact = "..."` — a `.slib` path, relative to this manifest.
    ArtifactPath { path: String },
}

/// One `[dependencies]` entry: an exact coordinate plus its locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredDependency {
    coordinate: ConeCoordinate,
    locator: DependencyLocator,
}

impl DeclaredDependency {
    pub fn new(coordinate: ConeCoordinate, locator: DependencyLocator) -> Self {
        DeclaredDependency {
            coordinate,
            locator,
        }
    }

    pub fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub fn locator(&self) -> &DependencyLocator {
        &self.locator
    }
}

/// A parsed `Cone.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeManifest {
    coordinate: ConeCoordinate,
    kind: ConeKind,
    dependencies: Vec<DeclaredDependency>,
}

impl ConeManifest {
    pub fn new(
        coordinate: ConeCoordinate,
        kind: ConeKind,
        dependencies: Vec<DeclaredDependency>,
    ) -> Self {
        ConeManifest {
            coordinate,
            kind,
            dependencies,
        }
    }

    pub fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub fn kind(&self) -> ConeKind {
        self.kind
    }

    pub fn dependencies(&self) -> &[DeclaredDependency] {
        &self.dependencies
    }

    /// The semantic projection consumed by `scoopc`: dependency
    /// coordinates only, no locators. The returned order is the manifest
    /// declaration order; canonical orderings are established by graph
    /// resolution, not by this projection.
    pub fn semantic_dependency_coordinates(&self) -> impl Iterator<Item = &ConeCoordinate> {
        self.dependencies.iter().map(|d| d.coordinate())
    }
}
