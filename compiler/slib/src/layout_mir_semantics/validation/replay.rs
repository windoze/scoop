use std::collections::BTreeMap;

use scoop_hir::{
    CommittedTypeUseSemanticAuthorityV1, TypeSectionDeclarationSemanticAuthority,
    TypeSectionDefaultSemanticAuthority, TypeSectionFoundationSemanticAuthority,
};
use scoop_mir::MirTypeBridgeLocalAuthorityV1;
use typed_arena::Arena;

use super::PreparedLayoutMirValidationInput;
use crate::{
    CheckedCrossConeLayoutHirProviderV1, CheckedCrossConeLayoutMirClosureV1,
    CheckedCrossConeLayoutMirProviderV1, CheckedLayoutMirDependencyV1,
    CrossConeLayoutHirSemanticClosureError, CrossConeLayoutMirSemanticClosureError,
    LayoutHirProviderSemanticAuthoritiesV1, LayoutHirPublicAuthorityFactoryV1,
    LayoutMirProviderSourceAuthorityV1, LayoutMirSourceAuthorityContextV1,
    LayoutMirSourceAuthorityFactoryV1, PreparedCrossConeLayoutLirValidation,
    PreparedCrossConeLayoutMirSections, PreparedLayoutMirSemanticParts, transitive_positions,
    validate_hir_provider,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_in_scope<'proof, 'input, 'hir_source, 'mir_source, P, F, S, D, C, M, R>(
    current: scoop_identity::ConeIdentity,
    target: scoop_lir::ValidatedLirTargetSelection,
    direct: Vec<scoop_identity::ConeIdentity>,
    validations: Vec<PreparedLayoutMirValidationInput<'input>>,
    dependency_positions: &[Vec<usize>],
    hir_authorities: &'proof mut [LayoutHirProviderSemanticAuthoritiesV1<
        'hir_source,
        P,
        F,
        S,
        D,
        C,
    >],
    mir_authorities: &'proof mut [LayoutMirProviderSourceAuthorityV1<'mir_source, M>],
    artifact_arena: &'proof Arena<PreparedCrossConeLayoutMirSections<'input>>,
    hir_arena: &'proof Arena<CheckedCrossConeLayoutHirProviderV1<'proof>>,
    mir_arena: &'proof Arena<CheckedCrossConeLayoutMirProviderV1<'proof>>,
    use_checked: impl FnOnce(
        CheckedCrossConeLayoutMirClosureV1<'proof>,
        Vec<PreparedCrossConeLayoutLirValidation<'proof>>,
    ) -> R,
) -> Result<R, CrossConeLayoutMirSemanticClosureError<P::Error, M::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    F: TypeSectionFoundationSemanticAuthority<P::Error>,
    S: TypeSectionDeclarationSemanticAuthority<P::Error>,
    D: TypeSectionDefaultSemanticAuthority<P::Error>,
    C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
    M: LayoutMirSourceAuthorityFactoryV1,
    'hir_source: 'proof,
    'mir_source: 'proof,
{
    let count = validations.len();
    let mut checked_hir = Vec::new();
    let mut checked_mir = Vec::new();
    let mut lir_validations = Vec::new();
    checked_hir.try_reserve_exact(count).map_err(|_| {
        CrossConeLayoutMirSemanticClosureError::Allocation {
            requested_slots: count,
        }
    })?;
    checked_mir.try_reserve_exact(count).map_err(|_| {
        CrossConeLayoutMirSemanticClosureError::Allocation {
            requested_slots: count,
        }
    })?;
    lir_validations.try_reserve_exact(count).map_err(|_| {
        CrossConeLayoutMirSemanticClosureError::Allocation {
            requested_slots: count,
        }
    })?;
    let mut validations = validations.into_iter();
    let mut hir_authorities = hir_authorities.iter_mut();
    let mut mir_authorities = mir_authorities.iter_mut();
    validate_artifacts(
        &mut validations,
        &mut hir_authorities,
        &mut mir_authorities,
        dependency_positions,
        0,
        artifact_arena,
        hir_arena,
        mir_arena,
        &mut checked_hir,
        &mut checked_mir,
        &mut lir_validations,
    )?;
    let positions = checked_mir
        .iter()
        .enumerate()
        .map(|(position, provider)| (provider.provider, position))
        .collect::<BTreeMap<_, _>>();
    Ok(use_checked(
        CheckedCrossConeLayoutMirClosureV1 {
            current,
            target,
            direct,
            providers: checked_mir,
            positions,
            dependency_positions: dependency_positions.to_vec(),
        },
        lir_validations,
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_artifacts<'proof, 'input, 'hir_source, 'mir_source, P, F, S, D, C, M>(
    validations: &mut std::vec::IntoIter<PreparedLayoutMirValidationInput<'input>>,
    hir_authorities: &mut std::slice::IterMut<
        '_,
        LayoutHirProviderSemanticAuthoritiesV1<'hir_source, P, F, S, D, C>,
    >,
    mir_authorities: &mut std::slice::IterMut<
        '_,
        LayoutMirProviderSourceAuthorityV1<'mir_source, M>,
    >,
    dependency_positions: &[Vec<usize>],
    position: usize,
    artifact_arena: &'proof Arena<PreparedCrossConeLayoutMirSections<'input>>,
    hir_arena: &'proof Arena<CheckedCrossConeLayoutHirProviderV1<'proof>>,
    mir_arena: &'proof Arena<CheckedCrossConeLayoutMirProviderV1<'proof>>,
    checked_hir: &mut Vec<&'proof CheckedCrossConeLayoutHirProviderV1<'proof>>,
    checked_mir: &mut Vec<&'proof CheckedCrossConeLayoutMirProviderV1<'proof>>,
    lir_validations: &mut Vec<PreparedCrossConeLayoutLirValidation<'proof>>,
) -> Result<(), CrossConeLayoutMirSemanticClosureError<P::Error, M::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    F: TypeSectionFoundationSemanticAuthority<P::Error>,
    S: TypeSectionDeclarationSemanticAuthority<P::Error>,
    D: TypeSectionDefaultSemanticAuthority<P::Error>,
    C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
    M: LayoutMirSourceAuthorityFactoryV1,
    'hir_source: 'proof,
    'mir_source: 'proof,
{
    let Some(validation) = validations.next() else {
        return Ok(());
    };
    let Some(hir_authority) = hir_authorities.next() else {
        return Err(CrossConeLayoutMirSemanticClosureError::Hir(Box::new(
            CrossConeLayoutHirSemanticClosureError::AuthorityCount {
                expected: dependency_positions.len(),
                actual: position,
            },
        )));
    };
    let Some(mir_authority) = mir_authorities.next() else {
        return Err(CrossConeLayoutMirSemanticClosureError::AuthorityCount {
            expected: dependency_positions.len(),
            actual: position,
        });
    };
    let PreparedLayoutMirValidationInput {
        artifact,
        candidate,
        lir_candidates,
    } = validation;
    let artifact: &'proof mut PreparedCrossConeLayoutMirSections<'input> =
        artifact_arena.alloc(artifact);
    let provider = artifact.provider();
    let coordinate = artifact.coordinate().clone();
    let PreparedLayoutMirSemanticParts {
        identities,
        hir_foundation,
        hir_core,
        hir_interface,
        hir_types,
        mir_foundation,
        mir_core,
        mir_ordinary,
        lir_foundation,
        meter,
        link_sections,
    } = artifact.semantic_parts();
    let hir = validate_hir_provider(
        position,
        provider,
        identities,
        hir_foundation,
        hir_core,
        hir_interface,
        hir_types,
        meter,
        hir_authority,
        dependency_positions,
        checked_hir,
    )
    .map_err(|source| CrossConeLayoutMirSemanticClosureError::Hir(Box::new(source)))?;
    let hir: &'proof CheckedCrossConeLayoutHirProviderV1<'proof> = hir_arena.alloc(hir);
    checked_hir.push(hir);

    let direct = dependency_views(
        provider,
        &dependency_positions[position],
        checked_hir,
        checked_mir,
    )?;
    let reachable = transitive_positions(position, dependency_positions);
    let transitive = dependency_views(provider, &reachable, checked_hir, checked_mir)?;
    let context = LayoutMirSourceAuthorityContextV1 {
        provider,
        identities,
        hir,
        foundation: mir_foundation,
        core: mir_core,
        ordinary: mir_ordinary,
        direct: &direct,
        transitive: &transitive,
    };
    let source = mir_authority.factory.build(context).map_err(|source| {
        CrossConeLayoutMirSemanticClosureError::SourceAuthority {
            provider,
            source: Box::new(source),
        }
    })?;
    let mut direct_sections = Vec::new();
    direct_sections
        .try_reserve_exact(direct.len())
        .map_err(
            |_| CrossConeLayoutMirSemanticClosureError::DependencyAllocation {
                provider,
                requested_slots: direct.len(),
            },
        )?;
    direct_sections.extend(direct.iter().map(|dependency| dependency.type_bridge()));
    let bridge = candidate
        .validate(
            MirTypeBridgeLocalAuthorityV1::Reader {
                provider,
                foundation: mir_foundation,
                production: mir_core,
                ordinary: mir_ordinary,
            },
            &direct_sections,
            &source,
            identities,
            meter,
        )
        .map_err(
            |source| CrossConeLayoutMirSemanticClosureError::TypeBridge {
                provider,
                source: Box::new(source),
            },
        )?;
    let mir: &'proof CheckedCrossConeLayoutMirProviderV1<'proof> =
        mir_arena.alloc(CheckedCrossConeLayoutMirProviderV1 {
            position,
            provider,
            hir,
            core: mir_core,
            ordinary: mir_ordinary,
            bridge,
            lir_foundation,
        });
    checked_mir.push(mir);
    lir_validations.push(PreparedCrossConeLayoutLirValidation {
        link_sections,
        coordinate,
        identities,
        foundation: lir_foundation,
        meter,
        candidates: lir_candidates,
    });
    validate_artifacts(
        validations,
        hir_authorities,
        mir_authorities,
        dependency_positions,
        position + 1,
        artifact_arena,
        hir_arena,
        mir_arena,
        checked_hir,
        checked_mir,
        lir_validations,
    )
}

fn dependency_views<'proof, HE, ME>(
    provider: scoop_identity::ConeIdentity,
    positions: &[usize],
    hir: &[&'proof CheckedCrossConeLayoutHirProviderV1<'proof>],
    mir: &[&'proof CheckedCrossConeLayoutMirProviderV1<'proof>],
) -> Result<Vec<CheckedLayoutMirDependencyV1<'proof>>, CrossConeLayoutMirSemanticClosureError<HE, ME>>
{
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(positions.len())
        .map_err(
            |_| CrossConeLayoutMirSemanticClosureError::DependencyAllocation {
                provider,
                requested_slots: positions.len(),
            },
        )?;
    dependencies.extend(
        positions
            .iter()
            .map(|position| CheckedLayoutMirDependencyV1 {
                hir: hir[*position],
                mir: mir[*position],
            }),
    );
    Ok(dependencies)
}
