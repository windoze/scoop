use typed_arena::Arena;

use super::ordinary::{self, PreparedLayoutLirFront};
use crate::{
    CheckedCrossConeLayoutLirClosureV1, CheckedCrossConeLayoutLirProviderV1,
    CheckedCrossConeLayoutMirClosureV1, CheckedLayoutLirDependencyV1,
    CrossConeLayoutLirSemanticClosureError, LayoutLirProviderSourceAuthorityV1,
    LayoutLirSourceAuthorityContextV1, LayoutLirSourceAuthorityFactoryV1,
    PreparedCrossConeLayoutLirValidation, transitive_positions,
};

pub(super) fn validate_in_scope<'proof, 'lir_source, HE, ME, L, R>(
    mir: CheckedCrossConeLayoutMirClosureV1<'proof>,
    validations: Vec<PreparedCrossConeLayoutLirValidation<'proof>>,
    authorities: &'proof mut [LayoutLirProviderSourceAuthorityV1<'lir_source, L>],
    arena: &'proof Arena<CheckedCrossConeLayoutLirProviderV1<'proof>>,
    use_checked: impl FnOnce(CheckedCrossConeLayoutLirClosureV1<'proof>) -> R,
) -> Result<R, CrossConeLayoutLirSemanticClosureError<HE, ME, L::Error>>
where
    L: LayoutLirSourceAuthorityFactoryV1,
    'lir_source: 'proof,
{
    let parts = mir.into_lir_parts();
    let fronts = ordinary::validate(
        validations,
        &parts.providers,
        &parts.positions,
        &parts.dependency_positions,
    )?;
    let count = fronts.len();
    let mut checked = Vec::new();
    checked.try_reserve_exact(count).map_err(|_| {
        CrossConeLayoutLirSemanticClosureError::Allocation {
            requested_slots: count,
        }
    })?;
    let mut fronts = fronts.into_iter();
    let mut authorities = authorities.iter_mut();
    validate_artifacts(
        &mut fronts,
        &mut authorities,
        parts.target,
        &parts.dependency_positions,
        arena,
        &mut checked,
    )?;
    Ok(use_checked(CheckedCrossConeLayoutLirClosureV1 {
        current: parts.current,
        target: parts.target,
        direct: parts.direct,
        providers: checked,
        positions: parts.positions,
    }))
}

#[allow(clippy::too_many_arguments)]
fn validate_artifacts<'proof, 'lir_source, HE, ME, L>(
    fronts: &mut std::vec::IntoIter<PreparedLayoutLirFront<'proof>>,
    authorities: &mut std::slice::IterMut<
        'proof,
        LayoutLirProviderSourceAuthorityV1<'lir_source, L>,
    >,
    target: scoop_lir::ValidatedLirTargetSelection,
    dependency_positions: &[Vec<usize>],
    arena: &'proof Arena<CheckedCrossConeLayoutLirProviderV1<'proof>>,
    checked: &mut Vec<&'proof CheckedCrossConeLayoutLirProviderV1<'proof>>,
) -> Result<(), CrossConeLayoutLirSemanticClosureError<HE, ME, L::Error>>
where
    L: LayoutLirSourceAuthorityFactoryV1,
    'lir_source: 'proof,
{
    let Some(front) = fronts.next() else {
        return Ok(());
    };
    let Some(authority) = authorities.next() else {
        return Err(CrossConeLayoutLirSemanticClosureError::AuthorityCount {
            expected: dependency_positions.len(),
            actual: front.position,
        });
    };
    let direct = dependency_views(
        front.provider,
        &dependency_positions[front.position],
        checked,
    )?;
    let reachable = transitive_positions(front.position, dependency_positions);
    let transitive = dependency_views(front.provider, &reachable, checked)?;
    let context = LayoutLirSourceAuthorityContextV1 {
        provider: front.provider,
        coordinate: &front.coordinate,
        target,
        identities: front.identities,
        mir: front.mir,
        foundation: front.foundation,
        ordinary: &front.ordinary,
        direct: &direct,
        transitive: &transitive,
    };
    let source = authority.factory.build(context).map_err(|source| {
        CrossConeLayoutLirSemanticClosureError::SourceAuthority {
            provider: front.provider,
            source: Box::new(source),
        }
    })?;
    let (source_provider, strong, exports, physical_imports, source) = source.into_parts();
    if source_provider != front.provider {
        return Err(CrossConeLayoutLirSemanticClosureError::SourceProvider {
            provider: front.provider,
            actual: source_provider,
        });
    }
    if exports.provider() != front.provider {
        return Err(CrossConeLayoutLirSemanticClosureError::SourceProvider {
            provider: front.provider,
            actual: exports.provider(),
        });
    }
    let (
        external_bridges,
        digests,
        entry,
        shape_sources,
        initialization_abi,
        type_definitions,
        initialization_definitions,
    ) = strong.into_parts();
    let mut direct_dependencies = Vec::new();
    front
        .meter
        .try_reserve_collection_slots(
            &mut direct_dependencies,
            direct.len(),
            &scoop_wire::WirePath::root(),
        )
        .map_err(
            |source| CrossConeLayoutLirSemanticClosureError::StrongReplay {
                provider: front.provider,
                source: Box::new(source.into()),
            },
        )?;
    direct_dependencies.extend(direct.iter().map(|dependency| dependency.provider()));
    let replayed = front
        .strong
        .replay(
            front.coordinate,
            &direct_dependencies,
            target.target(),
            front.foundation,
            external_bridges,
            digests,
            entry,
            &shape_sources,
            initialization_abi,
            &type_definitions,
            &initialization_definitions,
            front.meter,
        )
        .map_err(
            |source| CrossConeLayoutLirSemanticClosureError::StrongReplay {
                provider: front.provider,
                source: Box::new(source),
            },
        )?;
    let mut direct_sections = Vec::new();
    direct_sections
        .try_reserve_exact(direct.len())
        .map_err(
            |_| CrossConeLayoutLirSemanticClosureError::DependencyAllocation {
                provider: front.provider,
                requested_slots: direct.len(),
            },
        )?;
    direct_sections.extend(direct.iter().map(|dependency| dependency.layout_abi()));
    let layout = front
        .layout
        .validate(
            &exports,
            &direct_sections,
            physical_imports,
            &source,
            front.identities,
            front.meter,
        )
        .map_err(|source| CrossConeLayoutLirSemanticClosureError::LayoutAbi {
            provider: front.provider,
            source: Box::new(source),
        })?;
    super::abi::validate(&front.abi_expectations, &layout, &transitive, front.meter)?;
    let strong = replayed
        .validate_layout_abi(&layout, front.meter)
        .map_err(
            |source| CrossConeLayoutLirSemanticClosureError::StrongLayoutJoin {
                provider: front.provider,
                source: Box::new(source),
            },
        )?;
    let checked_provider = arena.alloc(CheckedCrossConeLayoutLirProviderV1 {
        position: front.position,
        provider: front.provider,
        mir: front.mir,
        ordinary: front.ordinary,
        strong,
        layout,
    });
    checked.push(checked_provider);
    validate_artifacts(
        fronts,
        authorities,
        target,
        dependency_positions,
        arena,
        checked,
    )
}

fn dependency_views<'proof, HE, ME, LE>(
    provider: scoop_identity::ConeIdentity,
    positions: &[usize],
    checked: &[&'proof CheckedCrossConeLayoutLirProviderV1<'proof>],
) -> Result<
    Vec<CheckedLayoutLirDependencyV1<'proof>>,
    CrossConeLayoutLirSemanticClosureError<HE, ME, LE>,
> {
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(positions.len())
        .map_err(
            |_| CrossConeLayoutLirSemanticClosureError::DependencyAllocation {
                provider,
                requested_slots: positions.len(),
            },
        )?;
    dependencies.extend(
        positions
            .iter()
            .map(|position| CheckedLayoutLirDependencyV1 {
                provider: checked[*position],
            }),
    );
    Ok(dependencies)
}
