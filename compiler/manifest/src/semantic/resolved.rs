//! Bind already expanded declarations to the selected exact graph.

use super::*;
use crate::package::{DependencyDeclaration, InheritableValue, PackageDocument};

impl ParsedConeManifest {
    /// A compiler handoff uses bound coordinates; discovery locators are no longer inputs.
    pub fn into_bound(mut self) -> Self {
        for locator in self.locators.0.values_mut() {
            *locator = DependencyLocator::SearchRoots;
        }
        for dependency in self.tests.dependencies.values_mut() {
            dependency.locator = DependencyLocator::SearchRoots;
        }
        self
    }

    /// The caller has expanded the coordinate and selected every dependency.
    /// This operation neither resolves versions nor interprets source locators.
    pub fn bind_package(
        document: &PackageDocument,
        coordinate: ConeCoordinate,
        bindings: &BTreeMap<DependencyCoordinateKey, ConeCoordinate>,
        include_tests: bool,
    ) -> Result<Self, ManifestParseError> {
        let cone = document.cone.as_ref().ok_or_else(|| {
            ManifestParseError::new(
                ManifestParseErrorKind::InvalidPackage("a virtual workspace is not a Cone".into()),
                Some(document.schema_span.clone()),
            )
        })?;
        let declared = bind(&document.dependencies, bindings)?;
        let mut tests = if include_tests {
            TestManifestConfig {
                sources: document.test_sources.clone(),
                dependencies: bind(&document.test_dependencies, bindings)?,
            }
        } else {
            TestManifestConfig {
                sources: SourceSelection::Explicit(Vec::new()),
                dependencies: BTreeMap::new(),
            }
        };
        // Core is ambient in both purposes; the exact graph has one production edge.
        tests
            .dependencies
            .remove(&DependencyCoordinateKey::from_coordinate(
                &ConeCoordinate::reserved_core(),
            ));
        let semantic = ConeManifestSemantic {
            coordinate,
            requested_kind: cone.kind,
            dependencies: declared
                .iter()
                .map(|(key, value)| (key.clone(), value.coordinate.clone()))
                .collect(),
            sources: document.sources.clone(),
            native: document.native.clone(),
            windows: document.windows.clone(),
        };
        let locators = DependencyLocatorTable(
            declared
                .keys()
                .map(|key| (key.clone(), DependencyLocator::SearchRoots))
                .collect(),
        );
        Ok(Self {
            semantic,
            locators,
            diagnostic_spans: ManifestDiagnosticSpans {
                schema: ManifestSpan::new(document.schema_span.clone()),
                cone: ManifestSpan::new(cone.span.clone()),
                cone_fields: ConeManifestSpans {
                    group: field_span(&cone.group),
                    name: ManifestSpan::new(cone.name.span.clone()),
                    version: field_span(&cone.version),
                    kind: ManifestSpan::new(cone.kind_span.clone()),
                },
                dependencies: declared
                    .into_iter()
                    .map(|(key, value)| (key, value.spans))
                    .collect(),
            },
            tests,
        })
    }
}

fn field_span(value: &InheritableValue) -> ManifestSpan {
    ManifestSpan::new(match value {
        InheritableValue::Value(value) => value.span.clone(),
        InheritableValue::Workspace { span } => span.clone(),
    })
}

fn bind(
    declarations: &BTreeMap<DependencyCoordinateKey, DependencyDeclaration>,
    bindings: &BTreeMap<DependencyCoordinateKey, ConeCoordinate>,
) -> Result<BTreeMap<DependencyCoordinateKey, ManifestDependency>, ManifestParseError> {
    declarations
        .iter()
        .map(|(key, declaration)| {
            let span = match declaration {
                DependencyDeclaration::Workspace { span } => span,
                DependencyDeclaration::Request(request) => &request.span,
            };
            let coordinate = bindings
                .get(key)
                .filter(|coordinate| DependencyCoordinateKey::from_coordinate(coordinate) == *key)
                .ok_or_else(|| {
                    ManifestParseError::new(
                        ManifestParseErrorKind::InvalidPackage(format!(
                            "no exact dependency binding for {key}"
                        )),
                        Some(span.clone()),
                    )
                })?;
            Ok((
                key.clone(),
                ManifestDependency {
                    coordinate: coordinate.clone(),
                    locator: DependencyLocator::SearchRoots,
                    spans: DependencyManifestSpans {
                        declaration: ManifestSpan::new(span.clone()),
                        version: ManifestSpan::new(span.clone()),
                        path: None,
                        artifact: None,
                    },
                },
            ))
        })
        .collect()
}
