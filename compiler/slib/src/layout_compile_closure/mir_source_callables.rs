//! Source callable replay from the same owned artifacts and provider scopes.

use std::{collections::BTreeMap, convert::Infallible};

use scoop_hir::{CheckedSharedTypeFoundationV1, SharedTypeMetadataV1};
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_mir::{
    CallablesResolvedCrossConeMirTypeBridgeSectionV1, CanonicalMirCallableBindingsV1,
    CanonicalMirDispatchSchemasV1, CanonicalMirObjectValuesV1, CanonicalMirShapeSupportsV1,
    CanonicalParamFreeMirTypeExportsV1,
};
use scoop_wire::WirePath;

use super::MirTypesValidatedCrossConeLayoutClosure;
use crate::{
    dependency_reachability::transitive_positions,
    layout_compile_decode::{
        DecodedCrossConeLayoutLirCandidates, PreparedCrossConeLayoutMirSections,
    },
};

mod constructors;
mod errors;
mod objects;
mod validation;
use SharedMirSourceCallableValidationError as Error;
pub use constructors::{
    SharedMirConstructorComponent, SharedMirConstructorValidationError,
    validate_shared_mir_constructors,
};
pub use errors::{
    CrossConeLayoutMirSourceCallablesError, SharedMirSourceCallableComponent,
    SharedMirSourceCallablePartition, SharedMirSourceCallableValidationError,
};
pub use objects::{
    SharedMirObjectComponent, SharedMirObjectValidationError, validate_shared_mir_objects,
};
pub use validation::validate_shared_mir_source_callables;

/// Source functions/accessors, constructors, object initialization and dispatch
/// agree with shared HIR. Derived equality, initialization uses, selected uses
/// and LIR remain unvalidated.
pub struct MirSourceCallablesValidatedCrossConeLayoutSections<'input> {
    prepared: PreparedCrossConeLayoutMirSections<'input>,
    mir: CallablesResolvedCrossConeMirTypeBridgeSectionV1,
    lir: DecodedCrossConeLayoutLirCandidates,
}

pub struct MirSourceCallablesValidatedCrossConeLayoutClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<MirSourceCallablesValidatedCrossConeLayoutSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl<'input> MirTypesValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_source_callables(
        self,
    ) -> Result<
        MirSourceCallablesValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutMirSourceCallablesError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut resolved: Vec<MirSourceCallablesValidatedCrossConeLayoutSections<'input>> =
            Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, Error> {
                let super::mir_types::MirTypesValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    lir,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mir = mir.resolve_callables::<Infallible>(
                    parts.mir_foundation,
                    reachable.iter().map(|position| {
                        let mir = &resolved[*position].mir;
                        (mir.types(), mir.callables(), mir.dispatch())
                    }),
                    parts.identities,
                    parts.meter,
                )?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut resolved, 1, &WirePath::root())?;
                Ok(MirSourceCallablesValidatedCrossConeLayoutSections { prepared, mir, lir })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(provider, source))?;
            resolved.push(artifact);
        }
        validate_sources(&mut resolved, &dependency_positions)?;
        Ok(MirSourceCallablesValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: resolved,
            positions,
            dependency_positions,
        })
    }
}

fn validate_sources(
    artifacts: &mut [MirSourceCallablesValidatedCrossConeLayoutSections<'_>],
    dependency_positions: &[Vec<usize>],
) -> Result<(), CrossConeLayoutMirSourceCallablesError> {
    let mut checked: Vec<CheckedSharedTypeFoundationV1<'_>> = Vec::new();
    let mut checked_callables: Vec<&CanonicalMirCallableBindingsV1> = Vec::new();
    for (position, artifact) in artifacts.iter_mut().enumerate() {
        let provider = artifact.identity();
        let parts = artifact.prepared.semantic_parts();
        let mut validate = || -> Result<_, Error> {
            let reachable = transitive_positions(position, dependency_positions, parts.meter)?;
            let mut dependencies = Vec::new();
            parts.meter.try_reserve_collection_slots(
                &mut dependencies,
                reachable.len(),
                &WirePath::root(),
            )?;
            dependencies.extend(reachable.iter().map(|position| checked[*position]));
            let mut dependency_callables = Vec::new();
            parts.meter.try_reserve_collection_slots(
                &mut dependency_callables,
                reachable.len(),
                &WirePath::root(),
            )?;
            dependency_callables.extend(
                reachable
                    .iter()
                    .map(|position| checked_callables[*position]),
            );
            let source = parts.hir_types.validate_shared_foundation(
                SharedTypeMetadataV1 {
                    provider,
                    identities: parts.identities,
                    foundation: parts.hir_foundation,
                    public: parts.hir_interface,
                },
                &dependencies,
                parts.meter,
            )?;
            source.with_inheritance_graph(&dependencies, parts.meter, |graph, meter| {
                validate_shared_mir_source_callables(
                    source,
                    &dependencies,
                    graph,
                    parts.mir_ordinary,
                    artifact.mir.callables(),
                    meter,
                )
            })??;
            validate_shared_mir_constructors(source, artifact.mir.callables(), parts.meter)
                .map_err(|error| Error::Constructors(Box::new(error)))?;
            validate_shared_mir_objects(
                source,
                artifact.mir.callables(),
                artifact.mir.object_values(),
                parts.meter,
            )
            .map_err(|error| Error::Objects(Box::new(error)))?;
            super::mir_dispatch::validate_shared_mir_dispatch(
                source,
                &dependencies,
                artifact.mir.callables(),
                &dependency_callables,
                artifact.mir.dispatch(),
                parts.meter,
            )
            .map_err(|error| Error::Dispatch(Box::new(error)))?;
            parts
                .meter
                .try_reserve_collection_slots(&mut checked, 1, &WirePath::root())?;
            parts.meter.try_reserve_collection_slots(
                &mut checked_callables,
                1,
                &WirePath::root(),
            )?;
            Ok(source)
        };
        let source = validate()
            .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(provider, source))?;
        checked.push(source);
        checked_callables.push(artifact.mir.callables());
    }
    Ok(())
}

impl MirSourceCallablesValidatedCrossConeLayoutSections<'_> {
    pub fn identity(&self) -> ConeIdentity {
        self.prepared.provider()
    }
    pub fn coordinate(&self) -> &ConeCoordinate {
        self.prepared.coordinate()
    }
    pub fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        self.mir.types()
    }
    pub fn shape_support(&self) -> &CanonicalMirShapeSupportsV1 {
        self.mir.shape_support()
    }
    pub fn callables(&self) -> &CanonicalMirCallableBindingsV1 {
        self.mir.callables()
    }
    pub fn object_values(&self) -> &CanonicalMirObjectValuesV1 {
        self.mir.object_values()
    }
    pub fn dispatch(&self) -> &CanonicalMirDispatchSchemasV1 {
        self.mir.dispatch()
    }
    pub fn lir_strong_production_wire(&self) -> &scoop_lir::DecodedStrongProductionSectionV2 {
        &self.lir.strong
    }
    pub fn lir_cross_cone_bridge_wire(&self) -> &scoop_lir::DecodedCrossConeLirBridgeSectionV1 {
        &self.lir.ordinary
    }
    pub fn lir_layout_abi_wire(&self) -> &scoop_lir::DecodedCrossConeLayoutAbiSectionV1 {
        &self.lir.layout
    }
}

impl MirSourceCallablesValidatedCrossConeLayoutClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }
    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }
    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }
    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &MirSourceCallablesValidatedCrossConeLayoutSections<'_>>
    {
        self.dependency_first.iter()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&MirSourceCallablesValidatedCrossConeLayoutSections<'_>> {
        self.positions
            .get(&provider)
            .map(|&position| &self.dependency_first[position])
    }
    pub fn dependency_count(&self, provider: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&provider)
            .map(|&position| self.dependency_positions[position].len())
    }
}
