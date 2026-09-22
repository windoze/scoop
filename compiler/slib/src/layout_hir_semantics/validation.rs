use scoop_hir::{
    CheckedTypeSectionPublicSupportV1, CommittedTypeUseSemanticAuthorityV1,
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, OdrFreeHirFoundation, TypeSectionDeclarationSemanticAuthority,
    TypeSectionDefaultSemanticAuthority, TypeSectionFoundationSemanticAuthority,
};
use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_wire::BudgetMeter;
use typed_arena::Arena;

use super::*;
use crate::{
    HirProductionValidatedCrossConeLayoutClosure, HirProductionValidatedCrossConeLayoutSections,
};

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    /// Validates every provider's old public section and new type-semantics
    /// section dependency-first, then lends the complete recursive proof to a
    /// callback. The proof cannot outlive this call.
    pub fn with_checked_hir_semantics<'source, P, F, S, D, C, R>(
        &mut self,
        authorities: &mut [LayoutHirProviderSemanticAuthoritiesV1<'source, P, F, S, D, C>],
        use_checked: impl for<'checked> FnOnce(CheckedCrossConeLayoutHirClosureV1<'checked>) -> R,
    ) -> Result<R, CrossConeLayoutHirSemanticClosureError<P::Error>>
    where
        P: LayoutHirPublicAuthorityFactoryV1,
        F: TypeSectionFoundationSemanticAuthority<P::Error>,
        S: TypeSectionDeclarationSemanticAuthority<P::Error>,
        D: TypeSectionDefaultSemanticAuthority<P::Error>,
        C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
    {
        validate_authority_inventory(self, authorities)?;
        let arena = Arena::new();
        validate_in_scope(self, authorities, &arena, use_checked)
    }
}

fn validate_authority_inventory<P, F, S, D, C>(
    closure: &HirProductionValidatedCrossConeLayoutClosure<'_>,
    authorities: &[LayoutHirProviderSemanticAuthoritiesV1<'_, P, F, S, D, C>],
) -> Result<(), CrossConeLayoutHirSemanticClosureError<P::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
{
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

fn validate_in_scope<'proof, 'input, 'source, P, F, S, D, C, R>(
    closure: &'proof mut HirProductionValidatedCrossConeLayoutClosure<'input>,
    authorities: &'proof mut [LayoutHirProviderSemanticAuthoritiesV1<'source, P, F, S, D, C>],
    arena: &'proof Arena<CheckedCrossConeLayoutHirProviderV1<'proof>>,
    use_checked: impl FnOnce(CheckedCrossConeLayoutHirClosureV1<'proof>) -> R,
) -> Result<R, CrossConeLayoutHirSemanticClosureError<P::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    F: TypeSectionFoundationSemanticAuthority<P::Error>,
    S: TypeSectionDeclarationSemanticAuthority<P::Error>,
    D: TypeSectionDefaultSemanticAuthority<P::Error>,
    C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
    'source: 'proof,
{
    let current = closure.current();
    let target = closure.target_selection();
    let mut direct = Vec::new();
    direct
        .try_reserve_exact(closure.direct_providers().len())
        .map_err(|_| CrossConeLayoutHirSemanticClosureError::Allocation {
            requested_slots: closure.direct_providers().len(),
        })?;
    direct.extend_from_slice(closure.direct_providers());
    let (artifacts, dependency_positions) = closure.hir_semantic_validation_parts();
    let mut checked = Vec::new();
    checked.try_reserve_exact(artifacts.len()).map_err(|_| {
        CrossConeLayoutHirSemanticClosureError::Allocation {
            requested_slots: artifacts.len(),
        }
    })?;
    validate_artifacts(
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
    Ok(use_checked(CheckedCrossConeLayoutHirClosureV1 {
        current,
        target,
        direct,
        providers: checked,
        positions,
    }))
}

#[allow(clippy::too_many_arguments)]
fn validate_artifacts<'proof, 'input, 'source, P, F, S, D, C>(
    artifacts: &'proof mut [HirProductionValidatedCrossConeLayoutSections<'input>],
    authorities: &mut [LayoutHirProviderSemanticAuthoritiesV1<'source, P, F, S, D, C>],
    dependency_positions: &[Vec<usize>],
    position: usize,
    arena: &'proof Arena<CheckedCrossConeLayoutHirProviderV1<'proof>>,
    checked: &mut Vec<&'proof CheckedCrossConeLayoutHirProviderV1<'proof>>,
) -> Result<(), CrossConeLayoutHirSemanticClosureError<P::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    F: TypeSectionFoundationSemanticAuthority<P::Error>,
    S: TypeSectionDeclarationSemanticAuthority<P::Error>,
    D: TypeSectionDefaultSemanticAuthority<P::Error>,
    C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
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
    let (identities, foundation, core, interface, types, meter) = artifact.hir_semantic_parts();
    let checked_provider = validate_hir_provider(
        position,
        provider,
        identities,
        foundation,
        core,
        interface,
        types,
        meter,
        authority,
        dependency_positions,
        checked,
    )?;
    checked.push(arena.alloc(checked_provider));
    validate_artifacts(
        remaining_artifacts,
        remaining_authorities,
        dependency_positions,
        position + 1,
        arena,
        checked,
    )
}

/// Validates one provider against already checked dependency-first HIR proofs.
/// The identity graph is borrowed only while the independent public authority
/// is built; the returned proof borrows the HIR transports themselves.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_hir_provider<'proof, 'source, P, F, S, D, C>(
    position: usize,
    provider: ConeIdentity,
    identities: &ValidatedIdentityGraph,
    foundation: &'proof OdrFreeHirFoundation,
    core: &'proof CoreBootstrapInterfaceSectionV1,
    interface: &'proof CrossConeHirInterfaceSectionV1,
    types: &'proof CrossConeTypeSemanticsSectionV1,
    meter: &mut BudgetMeter,
    authority: &mut LayoutHirProviderSemanticAuthoritiesV1<'source, P, F, S, D, C>,
    dependency_positions: &[Vec<usize>],
    checked: &[&'proof CheckedCrossConeLayoutHirProviderV1<'proof>],
) -> Result<
    CheckedCrossConeLayoutHirProviderV1<'proof>,
    CrossConeLayoutHirSemanticClosureError<P::Error>,
>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    F: TypeSectionFoundationSemanticAuthority<P::Error>,
    S: TypeSectionDeclarationSemanticAuthority<P::Error>,
    D: TypeSectionDefaultSemanticAuthority<P::Error>,
    C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
    'source: 'proof,
{
    let reachable = transitive_positions(position, dependency_positions);
    let mut public_dependencies = Vec::new();
    let mut type_dependencies = Vec::new();
    public_dependencies
        .try_reserve_exact(reachable.len())
        .map_err(
            |_| CrossConeLayoutHirSemanticClosureError::AuthorityAllocation {
                provider,
                requested_slots: reachable.len(),
            },
        )?;
    type_dependencies
        .try_reserve_exact(reachable.len())
        .map_err(
            |_| CrossConeLayoutHirSemanticClosureError::AuthorityAllocation {
                provider,
                requested_slots: reachable.len(),
            },
        )?;
    for dependency in reachable {
        let terminal = checked[dependency];
        public_dependencies.push(CheckedLayoutHirPublicDependencyV1 {
            provider: terminal.provider,
            core: terminal.core,
            interface: terminal.public.section(),
            checked: terminal.public,
        });
        type_dependencies.push(&terminal.types);
    }
    public_dependencies.sort_unstable_by_key(|dependency| dependency.provider);
    type_dependencies.sort_unstable_by_key(|dependency| dependency.provider());
    let direct_positions = &dependency_positions[position];
    let mut direct_dependencies = Vec::new();
    direct_dependencies
        .try_reserve_exact(direct_positions.len())
        .map_err(
            |_| CrossConeLayoutHirSemanticClosureError::AuthorityAllocation {
                provider,
                requested_slots: direct_positions.len(),
            },
        )?;
    direct_dependencies.extend(
        direct_positions
            .iter()
            .map(|dependency| checked[*dependency].provider),
    );
    direct_dependencies.sort_unstable();

    crate::cross_cone_hir_authority::validate_intrinsic_declarations(
        interface,
        identities,
        std::iter::once((provider, core)).chain(
            public_dependencies
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
    let public = {
        let context = LayoutHirPublicAuthorityContextV1 {
            provider,
            identities,
            foundation,
            core,
            interface,
            direct_dependencies: &direct_dependencies,
            dependencies: &public_dependencies,
        };
        let mut public_authority = authority.public.build(context).map_err(|source| {
            CrossConeLayoutHirSemanticClosureError::PublicAuthority { provider, source }
        })?;
        CheckedTypeSectionPublicSupportV1::validate(
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
        })?
    };
    let types = types
        .validate_semantics(
            public,
            &type_dependencies,
            authority.foundation,
            authority.declarations,
            authority.defaults,
            authority.committed,
            meter,
            &scoop_wire::WirePath::root(),
        )
        .map_err(|source| CrossConeLayoutHirSemanticClosureError::Types {
            provider,
            source: Box::new(source),
        })?;
    Ok(CheckedCrossConeLayoutHirProviderV1 {
        position,
        provider,
        core,
        public,
        types,
    })
}

pub(crate) fn transitive_positions(
    position: usize,
    dependency_positions: &[Vec<usize>],
) -> Vec<usize> {
    let mut reachable = vec![false; position];
    let mut pending = dependency_positions[position].clone();
    while let Some(dependency) = pending.pop() {
        if reachable[dependency] {
            continue;
        }
        reachable[dependency] = true;
        pending.extend(dependency_positions[dependency].iter().copied());
    }
    reachable
        .into_iter()
        .enumerate()
        .filter_map(|(position, reachable)| reachable.then_some(position))
        .collect()
}
