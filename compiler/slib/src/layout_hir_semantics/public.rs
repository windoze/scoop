//! Complete old-public validation in a scoped typed arena.

use scoop_hir::CheckedTypeSectionPublicSupportV1;
use typed_arena::Arena;

use super::*;
use crate::{
    HirProductionValidatedCrossConeLayoutClosure, HirProductionValidatedCrossConeLayoutSections,
};

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    /// Validates the complete old ten-table surface dependency-first. This
    /// phase is useful independently of the new source projections needed by
    /// the eight-field type section.
    pub fn with_checked_hir_public_support<'source, P, R>(
        &mut self,
        authorities: &mut [LayoutHirProviderPublicAuthorityV1<'source, P>],
        use_checked: impl for<'checked> FnOnce(CheckedCrossConeLayoutHirPublicClosureV1<'checked>) -> R,
    ) -> Result<R, CrossConeLayoutHirSemanticClosureError<P::Error>>
    where
        P: LayoutHirPublicAuthorityFactoryV1,
    {
        validate_public_inventory(self, authorities)?;
        let arena = Arena::new();
        validate_public_in_scope(self, authorities, &arena, use_checked)
    }
}

fn validate_public_inventory<E, P>(
    closure: &HirProductionValidatedCrossConeLayoutClosure<'_>,
    authorities: &[LayoutHirProviderPublicAuthorityV1<'_, P>],
) -> Result<(), CrossConeLayoutHirSemanticClosureError<E>> {
    let expected = closure.dependency_first().count();
    if authorities.len() != expected {
        return Err(CrossConeLayoutHirSemanticClosureError::AuthorityCount {
            expected,
            actual: authorities.len(),
        });
    }
    for (position, (artifact, authority)) in closure
        .dependency_first()
        .zip(authorities.iter())
        .enumerate()
    {
        let expected = artifact.identity();
        if authority.provider != expected {
            return Err(CrossConeLayoutHirSemanticClosureError::AuthorityProvider {
                position,
                expected,
                actual: authority.provider,
            });
        }
    }
    Ok(())
}

fn validate_public_in_scope<'proof, 'input, 'source, P, R>(
    closure: &'proof mut HirProductionValidatedCrossConeLayoutClosure<'input>,
    authorities: &'proof mut [LayoutHirProviderPublicAuthorityV1<'source, P>],
    arena: &'proof Arena<CheckedCrossConeLayoutHirPublicProviderV1<'proof>>,
    use_checked: impl FnOnce(CheckedCrossConeLayoutHirPublicClosureV1<'proof>) -> R,
) -> Result<R, CrossConeLayoutHirSemanticClosureError<P::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    'source: 'proof,
{
    let current = closure.current();
    let target = closure.target_selection();
    let direct = copy_direct(closure)?;
    let (artifacts, dependency_positions) = closure.hir_semantic_validation_parts();
    let mut checked = Vec::new();
    checked.try_reserve_exact(artifacts.len()).map_err(|_| {
        CrossConeLayoutHirSemanticClosureError::Allocation {
            requested_slots: artifacts.len(),
        }
    })?;
    validate_public_artifacts(
        artifacts,
        authorities,
        dependency_positions,
        0,
        arena,
        &mut checked,
    )?;
    let positions = checked
        .iter()
        .enumerate()
        .map(|(position, provider)| (provider.provider, position))
        .collect();
    Ok(use_checked(CheckedCrossConeLayoutHirPublicClosureV1 {
        current,
        target,
        direct,
        providers: checked,
        positions,
    }))
}

fn validate_public_artifacts<'proof, 'input, 'source, P>(
    artifacts: &'proof mut [HirProductionValidatedCrossConeLayoutSections<'input>],
    authorities: &mut [LayoutHirProviderPublicAuthorityV1<'source, P>],
    dependency_positions: &[Vec<usize>],
    position: usize,
    arena: &'proof Arena<CheckedCrossConeLayoutHirPublicProviderV1<'proof>>,
    checked: &mut Vec<&'proof CheckedCrossConeLayoutHirPublicProviderV1<'proof>>,
) -> Result<(), CrossConeLayoutHirSemanticClosureError<P::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    'source: 'proof,
{
    let Some((artifact, remaining_artifacts)) = artifacts.split_first_mut() else {
        return Ok(());
    };
    let Some((authority, remaining_authorities)) = authorities.split_first_mut() else {
        return Err(CrossConeLayoutHirSemanticClosureError::AuthorityCount {
            expected: position + 1 + remaining_artifacts.len(),
            actual: position,
        });
    };
    let provider = artifact.identity();
    let reachable = super::validation::transitive_positions(position, dependency_positions);
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(reachable.len())
        .map_err(
            |_| CrossConeLayoutHirSemanticClosureError::AuthorityAllocation {
                provider,
                requested_slots: reachable.len(),
            },
        )?;
    dependencies.extend(reachable.into_iter().map(|dependency| {
        let terminal = checked[dependency];
        CheckedLayoutHirPublicDependencyV1 {
            provider: terminal.provider,
            core: terminal.core,
            interface: terminal.checked.section(),
            checked: terminal.checked,
        }
    }));
    dependencies.sort_unstable_by_key(|dependency| dependency.provider);
    let direct = direct_dependencies(provider, position, dependency_positions, checked)?;
    let (identities, foundation, core, interface, _, meter) = artifact.hir_semantic_parts();
    crate::cross_cone_hir_authority::validate_intrinsic_declarations(
        interface,
        identities,
        std::iter::once((provider, core)).chain(
            dependencies
                .iter()
                .map(|dependency| (dependency.provider, dependency.core)),
        ),
        meter,
    )
    .map_err(
        |source| CrossConeLayoutHirSemanticClosureError::Intrinsics {
            provider,
            source: Box::new(source),
        },
    )?;
    let context = LayoutHirPublicAuthorityContextV1 {
        provider,
        identities,
        foundation,
        core,
        interface,
        direct_dependencies: &direct,
        dependencies: &dependencies,
    };
    let mut public_authority = authority.factory.build(context).map_err(|source| {
        CrossConeLayoutHirSemanticClosureError::PublicAuthority { provider, source }
    })?;
    let public = CheckedTypeSectionPublicSupportV1::validate(
        interface,
        provider,
        core.direct_public_surface(),
        &mut public_authority,
        meter,
        &scoop_wire::WirePath::root(),
    )
    .map_err(|source| CrossConeLayoutHirSemanticClosureError::Public {
        provider,
        source: Box::new(source),
    })?;
    checked.push(arena.alloc(CheckedCrossConeLayoutHirPublicProviderV1 {
        position,
        provider,
        core,
        checked: public,
    }));
    validate_public_artifacts(
        remaining_artifacts,
        remaining_authorities,
        dependency_positions,
        position + 1,
        arena,
        checked,
    )
}

fn direct_dependencies<E>(
    provider: scoop_identity::ConeIdentity,
    position: usize,
    dependency_positions: &[Vec<usize>],
    checked: &[&CheckedCrossConeLayoutHirPublicProviderV1<'_>],
) -> Result<Vec<scoop_identity::ConeIdentity>, CrossConeLayoutHirSemanticClosureError<E>> {
    let positions = &dependency_positions[position];
    let mut direct = Vec::new();
    direct.try_reserve_exact(positions.len()).map_err(|_| {
        CrossConeLayoutHirSemanticClosureError::AuthorityAllocation {
            provider,
            requested_slots: positions.len(),
        }
    })?;
    direct.extend(positions.iter().map(|position| checked[*position].provider));
    direct.sort_unstable();
    Ok(direct)
}

fn copy_direct<E>(
    closure: &HirProductionValidatedCrossConeLayoutClosure<'_>,
) -> Result<Vec<scoop_identity::ConeIdentity>, CrossConeLayoutHirSemanticClosureError<E>> {
    let mut direct = Vec::new();
    direct
        .try_reserve_exact(closure.direct_providers().len())
        .map_err(|_| CrossConeLayoutHirSemanticClosureError::Allocation {
            requested_slots: closure.direct_providers().len(),
        })?;
    direct.extend_from_slice(closure.direct_providers());
    Ok(direct)
}
