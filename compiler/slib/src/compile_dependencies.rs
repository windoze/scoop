//! Source declarations are loaded before selecting machine dependency providers.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use scoop_hir as hir;
use scoop_identity::{
    ConeCoordinate, ConeIdentity, SemanticIdentityImport, SemanticIdentitySession,
    SemanticOriginFingerprint, ValidatedIdentityGraph,
};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    CrossConeArtifactClosureValidationError as Error, CrossConeClosureArtifactSlotV1 as Slot,
    CrossConeLayoutArtifactValidationError as LayoutError, ValidatedGraphArtifact,
};

mod machine;
mod read;

struct HirDependency<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundation: Rc<hir::CanonicalHirFoundation>,
    core: Rc<hir::CoreBootstrapInterfaceSectionV1>,
    interface: Rc<hir::CrossConeHirInterfaceSectionV1>,
    types: Rc<hir::CrossConeTypeSemanticsSectionV1>,
    aliases: Rc<hir::CanonicalTypeAliasExpansionsV1>,
}

/// HIR declarations for the full source visibility graph. Machine payloads
/// remain in the same files until the current HIR selects their providers.
pub struct CompileDependencyClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    providers: Vec<HirDependency<'input>>,
    imported: Vec<hir::ImportedHirFoundation>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependencies: Vec<Vec<usize>>,
    session: RefCell<SemanticIdentitySession>,
}

impl<'input> CompileDependencyClosure<'input> {
    /// The caller supplies the manifest-checked, dependency-first file graph.
    pub fn read_hir(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<&'input crate::ArtifactFile>,
    ) -> Result<Self, Error> {
        let mut providers = Vec::with_capacity(dependency_first.len());
        let mut positions = BTreeMap::new();
        let mut dependencies: Vec<Vec<usize>> = Vec::with_capacity(dependency_first.len());
        for (position, file) in dependency_first.into_iter().enumerate() {
            let graph = crate::DecodedSlibEnvelope::from_file(file)
                .map_err(|source| {
                    layout(LayoutError::Envelope {
                        slot: Slot::Dependency(position),
                        source: Box::new(source),
                    })
                })?
                .validate_graph()
                .map_err(|source| {
                    layout(LayoutError::Graph {
                        slot: Slot::Dependency(position),
                        source: Box::new(source),
                    })
                })?;
            let direct_positions = graph
                .direct_dependencies()
                .iter()
                .map(|dependency| {
                    positions
                        .get(&dependency.identity())
                        .copied()
                        .ok_or_else(|| {
                            semantic(
                                crate::CrossConeClosureGraphError::MissingDependencyArtifact {
                                    dependent: graph.identity(),
                                    dependency: dependency.identity(),
                                },
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut reachable = std::collections::BTreeSet::new();
            let mut pending = direct_positions.clone();
            while let Some(index) = pending.pop() {
                if reachable.insert(index) {
                    pending.extend(&dependencies[index]);
                }
            }
            let provider = read::read_hir(
                graph,
                position,
                &providers,
                &direct_positions,
                &reachable.into_iter().collect::<Vec<_>>(),
            )?;
            positions.insert(provider.graph.identity(), position);
            dependencies.push(direct_positions);
            providers.push(provider);
        }
        let mut session = SemanticIdentitySession::new();
        let imports = providers
            .iter()
            .map(|provider| {
                let fingerprints = provider.graph.envelope.manifest().semantic_fingerprints();
                SemanticIdentityImport::new(
                    provider.graph.identity(),
                    SemanticOriginFingerprint::new(
                        *fingerprints.hir().as_array(),
                        *fingerprints.mir().as_array(),
                        *fingerprints.lir().as_array(),
                    ),
                    &provider.identities,
                )
            })
            .collect::<Vec<_>>();
        let layers = session.import_batch(&imports).map_err(|source| {
            Error::Commit(Box::new(
                crate::CrossConeSemanticCommitError::SemanticImport(source),
            ))
        })?;
        let imported = providers
            .iter()
            .zip(layers)
            .map(|(provider, layers)| {
                hir::ImportedHirFoundation::from_shared(
                    Rc::clone(&provider.foundation),
                    layers.into_parts().0,
                )
            })
            .collect();
        Ok(Self {
            current,
            target,
            direct,
            providers,
            imported,
            positions,
            dependencies,
            session: RefCell::new(session),
        })
    }

    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn identity_inputs(
        &self,
    ) -> impl Iterator<Item = (&ConeCoordinate, &ValidatedIdentityGraph)> {
        self.providers
            .iter()
            .map(|provider| (provider.graph.coordinate(), &provider.identities))
    }

    pub fn hir_metadata(&self) -> impl Iterator<Item = hir::SharedTypeMetadataV1<'_>> {
        self.providers
            .iter()
            .map(|provider| hir::SharedTypeMetadataV1 {
                provider: provider.graph.identity(),
                identities: &provider.identities,
                foundation: &provider.foundation,
                public: &provider.interface,
            })
    }

    pub fn imported_semantic_world(
        &self,
    ) -> Result<hir::ImportedSemanticWorld<'_>, hir::ImportedSemanticWorldBuildError> {
        let mut direct = Vec::with_capacity(self.direct.len());
        let mut support = Vec::new();
        for (provider, imported) in self.providers.iter().zip(&self.imported) {
            let input = hir::ImportedProviderInput {
                foundation: imported,
                interface: &provider.interface,
                alias_expansions: &provider.aliases,
            };
            if self
                .direct
                .binary_search(&provider.graph.identity())
                .is_ok()
            {
                direct.push(input);
            } else {
                support.push(input);
            }
        }
        hir::ImportedSemanticWorld::from_dependencies(self.current, direct, support)
    }

    pub fn import_compiler_protocols(
        &self,
        provider: ConeIdentity,
    ) -> Result<hir::ImportedCoreInputs, crate::CrossConeProtocolImportError> {
        use crate::CrossConeProtocolImportError as E;
        let position = *self
            .positions
            .get(&provider)
            .ok_or(E::MissingProvider(provider))?;
        let definitions = self.providers[position]
            .core
            .compiler_protocols()
            .ok_or(E::MissingDefinitions(provider))?;
        self.imported[position]
            .import_core_inputs(definitions)
            .map_err(E::Import)
    }
}

fn layout(error: LayoutError) -> Error {
    Error::Layout(Box::new(error))
}

fn semantic(error: impl Into<crate::CrossConeLayoutSemanticClosureError>) -> Error {
    layout(LayoutError::Semantic {
        source: Box::new(error.into()),
    })
}

fn sections(slot: usize, source: crate::CompileSectionDecodeError) -> Error {
    layout(LayoutError::CompileSections {
        slot: Slot::Dependency(slot),
        source: Box::new(source),
    })
}
