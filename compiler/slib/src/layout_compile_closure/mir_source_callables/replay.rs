//! Shared source agreement after all canonical MIR transports are resolved.

use super::*;

pub(super) fn validate_sources(
    current: ConeIdentity,
    artifacts: &mut [ResolvedMirSourceSections<'_>],
    dependency_positions: &[Vec<usize>],
) -> Result<
    Vec<Vec<scoop_mir::MirTypeBridgeInitializationUnitV1>>,
    CrossConeLayoutMirSourceCallablesError,
> {
    let mut inputs = Vec::new();
    let mut checked: Vec<CheckedSharedTypeFoundationV1<'_>> = Vec::new();
    let mut dependencies = Vec::new();
    let path = WirePath::root();
    for result in [
        scoop_wire::allocation::try_reserve(&mut inputs, artifacts.len(), &path),
        scoop_wire::allocation::try_reserve(&mut checked, artifacts.len(), &path),
        scoop_wire::allocation::try_reserve(&mut dependencies, artifacts.len(), &path),
    ] {
        result.map_err(|source| {
            CrossConeLayoutMirSourceCallablesError::new(current, source.into())
        })?;
    }
    for artifact in artifacts {
        inputs.push((
            artifact.prepared.provider(),
            artifact.prepared.semantic_parts(),
            &artifact.mir,
        ));
    }
    for (position, (provider, parts, _)) in inputs.iter().enumerate() {
        let validate = || -> Result<_, Error> {
            let reachable = transitive_positions(position, dependency_positions)?;
            let mut source_dependencies = Vec::new();
            scoop_wire::allocation::try_reserve(&mut source_dependencies, reachable.len(), &path)?;
            source_dependencies.extend(reachable.iter().map(|position| checked[*position]));
            let source = parts.hir_types.validate_shared_foundation(
                SharedTypeMetadataV1 {
                    provider: *provider,
                    identities: parts.identities,
                    foundation: parts.hir_foundation,
                    public: parts.hir_interface,
                },
                &source_dependencies,
            )?;
            Ok((source, (reachable, source_dependencies)))
        };
        let (source, scope) = validate()
            .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(*provider, source))?;
        checked.push(source);
        dependencies.push(scope);
    }
    let Some((root, graph_dependencies)) = checked.split_last() else {
        return Ok(Vec::new());
    };
    root.with_inheritance_graph(graph_dependencies, |graph| {
        let mut units = Vec::new();
        for ((provider, parts, mir), (source, (reachable, dependencies))) in
            inputs.iter().zip(checked.iter().zip(&dependencies))
        {
            let mut validate = || -> Result<_, Error> {
                super::super::mir_types::validate_shared_mir_type_exports(
                    *source,
                    dependencies,
                    graph,
                    parts.hir_core,
                    mir.types(),
                    mir.shape_support(),
                )?;
                validate_shared_mir_source_callables(
                    *source,
                    dependencies,
                    parts.mir_ordinary,
                    parts.mir_core.strong_callable_bridges(),
                    mir.callables(),
                )?;
                validate_shared_mir_constructors(*source, mir.callables())
                    .map_err(|error| Error::Constructors(Box::new(error)))?;
                validate_shared_mir_objects(*source, mir.callables(), mir.object_values())
                    .map_err(|error| Error::Objects(Box::new(error)))?;
                validate_shared_mir_equality(
                    *source,
                    dependencies,
                    parts.mir_core.strong_callable_bridges(),
                    mir.callables(),
                )
                .map_err(|error| Error::Equality(Box::new(error)))?;
                let mut dependency_callables = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut dependency_callables,
                    reachable.len(),
                    &path,
                )?;
                dependency_callables.extend(
                    reachable
                        .iter()
                        .map(|position| inputs[*position].2.callables()),
                );
                super::super::mir_dispatch::validate_shared_mir_dispatch(
                    *source,
                    mir.callables(),
                    &dependency_callables,
                    &std::iter::once(parts.mir_ordinary)
                        .chain(
                            reachable
                                .iter()
                                .map(|position| inputs[*position].1.mir_ordinary),
                        )
                        .collect::<Vec<_>>(),
                    mir.dispatch(),
                )
                .map_err(|error| Error::Dispatch(Box::new(error)))?;
                let result = source.metadata().signature_exact_type(
                    &scoop_identity::SignatureTypeKey::Nominal(
                        scoop_identity::CoreBuiltinNominal::Unit
                            .identity_record()
                            .id(),
                    ),
                )?;
                let initialization = scoop_mir::replay_source_initialization_units(
                    *provider,
                    source.metadata().source_initialization_units(),
                    parts.mir_foundation,
                    parts.mir_core.strong_callable_bridges(),
                    parts.identities,
                    result,
                )?;
                scoop_wire::allocation::try_reserve(&mut units, 1, &path)?;
                Ok(initialization)
            };
            let initialization = validate()
                .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(*provider, source))?;
            units.push(initialization);
        }
        Ok(units)
    })
    .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(current, source.into()))?
}
