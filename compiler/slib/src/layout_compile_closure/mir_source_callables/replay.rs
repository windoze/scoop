//! Shared source agreement after all canonical MIR transports are resolved.

use super::*;

pub(super) fn validate_sources(
    artifacts: &mut [ResolvedMirSourceSections<'_>],
    dependency_positions: &[Vec<usize>],
) -> Result<
    Vec<Vec<scoop_mir::MirTypeBridgeInitializationUnitV1>>,
    CrossConeLayoutMirSourceCallablesError,
> {
    let mut units = Vec::new();
    let mut checked: Vec<CheckedSharedTypeFoundationV1<'_>> = Vec::new();
    let mut checked_callables: Vec<&CanonicalMirCallableBindingsV1> = Vec::new();
    for (position, artifact) in artifacts.iter_mut().enumerate() {
        let provider = artifact.prepared.provider();
        let parts = artifact.prepared.semantic_parts();
        let mut validate = || -> Result<_, Error> {
            let reachable = transitive_positions(position, dependency_positions)?;
            let mut dependencies = Vec::new();
            scoop_wire::allocation::try_reserve(
                &mut dependencies,
                reachable.len(),
                &WirePath::root(),
            )?;
            dependencies.extend(reachable.iter().map(|position| checked[*position]));
            let mut dependency_callables = Vec::new();
            scoop_wire::allocation::try_reserve(
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
            )?;
            source.with_inheritance_graph(&dependencies, |graph| {
                validate_shared_mir_source_callables(
                    source,
                    &dependencies,
                    graph,
                    parts.mir_ordinary,
                    artifact.mir.callables(),
                )
            })??;
            validate_shared_mir_constructors(source, artifact.mir.callables())
                .map_err(|error| Error::Constructors(Box::new(error)))?;
            validate_shared_mir_objects(
                source,
                artifact.mir.callables(),
                artifact.mir.object_values(),
            )
            .map_err(|error| Error::Objects(Box::new(error)))?;
            validate_shared_mir_equality(
                source,
                &dependencies,
                parts.mir_core.strong_callable_bridges(),
                artifact.mir.callables(),
            )
            .map_err(|error| Error::Equality(Box::new(error)))?;
            super::super::mir_dispatch::validate_shared_mir_dispatch(
                source,
                &dependencies,
                artifact.mir.callables(),
                &dependency_callables,
                artifact.mir.dispatch(),
            )
            .map_err(|error| Error::Dispatch(Box::new(error)))?;
            scoop_wire::allocation::try_reserve(&mut checked, 1, &WirePath::root())?;
            scoop_wire::allocation::try_reserve(&mut checked_callables, 1, &WirePath::root())?;
            let result = source.metadata().signature_exact_type(
                &scoop_identity::SignatureTypeKey::Nominal(
                    scoop_identity::CoreBuiltinNominal::Unit
                        .identity_record()
                        .id(),
                ),
            )?;
            let initialization = scoop_mir::replay_source_initialization_units(
                provider,
                source.metadata().source_initialization_units(),
                parts.mir_foundation,
                parts.mir_core.strong_callable_bridges(),
                parts.identities,
                result,
            )?;
            scoop_wire::allocation::try_reserve(&mut units, 1, &WirePath::root())?;
            Ok((source, initialization))
        };
        let (source, initialization) = validate()
            .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(provider, source))?;
        units.push(initialization);
        checked.push(source);
        checked_callables.push(artifact.mir.callables());
    }
    Ok(units)
}
