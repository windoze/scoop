use scoop_hir::{
    CommittedTypeUseSemanticAuthorityV1, TypeSectionDeclarationSemanticAuthority,
    TypeSectionDefaultSemanticAuthority, TypeSectionFoundationSemanticAuthority,
};
use scoop_mir::DecodedCrossConeMirTypeBridgeSectionV1;
use typed_arena::Arena;

mod replay;

use super::*;
use crate::{
    CrossConeLayoutHirSemanticClosureError, DecodedCrossConeLayoutLirCandidates,
    HirProductionValidatedCrossConeLayoutClosure, LayoutHirProviderSemanticAuthoritiesV1,
    LayoutHirPublicAuthorityFactoryV1, PreparedCrossConeLayoutLirValidation,
    PreparedCrossConeLayoutMirSections,
};
use replay::validate_in_scope;

struct PreparedLayoutMirValidationInput<'input> {
    artifact: PreparedCrossConeLayoutMirSections<'input>,
    candidate: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_candidates: DecodedCrossConeLayoutLirCandidates,
}

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    /// Replays the complete HIR publication first and then the M23-6 MIR type
    /// bridge for every provider. Both recursive proof layers are lent to one
    /// callback and cannot escape their typed arenas.
    pub fn with_checked_mir_type_bridges<'hir_source, P, F, S, D, C, M, R>(
        self,
        hir_authorities: &mut [LayoutHirProviderSemanticAuthoritiesV1<
            'hir_source,
            P,
            F,
            S,
            D,
            C,
        >],
        mir_authorities: &mut [LayoutMirProviderSourceAuthorityV1<'_, M>],
        use_checked: impl for<'checked> FnOnce(CheckedCrossConeLayoutMirClosureV1<'checked>) -> R,
    ) -> Result<R, CrossConeLayoutMirSemanticClosureError<P::Error, M::Error>>
    where
        P: LayoutHirPublicAuthorityFactoryV1,
        F: TypeSectionFoundationSemanticAuthority<P::Error>,
        S: TypeSectionDeclarationSemanticAuthority<P::Error>,
        D: TypeSectionDefaultSemanticAuthority<P::Error>,
        C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
        M: LayoutMirSourceAuthorityFactoryV1,
    {
        self.with_checked_mir_type_bridge_parts(hir_authorities, mir_authorities, |checked, _| {
            use_checked(checked)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_checked_mir_type_bridge_parts<'hir_source, P, F, S, D, C, M, R>(
        self,
        hir_authorities: &mut [LayoutHirProviderSemanticAuthoritiesV1<
            'hir_source,
            P,
            F,
            S,
            D,
            C,
        >],
        mir_authorities: &mut [LayoutMirProviderSourceAuthorityV1<'_, M>],
        use_checked: impl for<'checked> FnOnce(
            CheckedCrossConeLayoutMirClosureV1<'checked>,
            Vec<PreparedCrossConeLayoutLirValidation<'checked>>,
        ) -> R,
    ) -> Result<R, CrossConeLayoutMirSemanticClosureError<P::Error, M::Error>>
    where
        P: LayoutHirPublicAuthorityFactoryV1,
        F: TypeSectionFoundationSemanticAuthority<P::Error>,
        S: TypeSectionDeclarationSemanticAuthority<P::Error>,
        D: TypeSectionDefaultSemanticAuthority<P::Error>,
        C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
        M: LayoutMirSourceAuthorityFactoryV1,
    {
        validate_authority_inventories(&self, hir_authorities, mir_authorities)?;
        let (current, target, direct, artifacts, dependency_positions) =
            self.into_mir_semantic_validation_parts();
        let mut validations = Vec::new();
        validations
            .try_reserve_exact(artifacts.len())
            .map_err(|_| CrossConeLayoutMirSemanticClosureError::Allocation {
                requested_slots: artifacts.len(),
            })?;
        for artifact in artifacts {
            let provider = artifact.identity();
            let (artifact, candidate, lir_candidates) =
                artifact.prepare_mir_semantics().map_err(|source| {
                    CrossConeLayoutMirSemanticClosureError::Front {
                        provider,
                        source: Box::new(source),
                    }
                })?;
            validations.push(PreparedLayoutMirValidationInput {
                artifact,
                candidate,
                lir_candidates,
            });
        }
        let artifact_arena = Arena::new();
        let hir_arena = Arena::new();
        let mir_arena = Arena::new();
        validate_in_scope(
            current,
            target,
            direct,
            validations,
            &dependency_positions,
            hir_authorities,
            mir_authorities,
            &artifact_arena,
            &hir_arena,
            &mir_arena,
            use_checked,
        )
    }
}

fn validate_authority_inventories<P, F, S, D, C, M>(
    closure: &HirProductionValidatedCrossConeLayoutClosure<'_>,
    hir: &[LayoutHirProviderSemanticAuthoritiesV1<'_, P, F, S, D, C>],
    mir: &[LayoutMirProviderSourceAuthorityV1<'_, M>],
) -> Result<(), CrossConeLayoutMirSemanticClosureError<P::Error, M::Error>>
where
    P: LayoutHirPublicAuthorityFactoryV1,
    M: LayoutMirSourceAuthorityFactoryV1,
{
    let expected = closure.dependency_first().count();
    if hir.len() != expected {
        return Err(CrossConeLayoutMirSemanticClosureError::Hir(Box::new(
            CrossConeLayoutHirSemanticClosureError::AuthorityCount {
                expected,
                actual: hir.len(),
            },
        )));
    }
    if mir.len() != expected {
        return Err(CrossConeLayoutMirSemanticClosureError::AuthorityCount {
            expected,
            actual: mir.len(),
        });
    }
    for (position, ((artifact, hir), mir)) in
        closure.dependency_first().zip(hir).zip(mir).enumerate()
    {
        let expected = artifact.identity();
        if hir.provider() != expected {
            return Err(CrossConeLayoutMirSemanticClosureError::Hir(Box::new(
                CrossConeLayoutHirSemanticClosureError::AuthorityProvider {
                    position,
                    expected,
                    actual: hir.provider(),
                },
            )));
        }
        if mir.provider != expected {
            return Err(CrossConeLayoutMirSemanticClosureError::AuthorityProvider {
                position,
                expected,
                actual: mir.provider,
            });
        }
    }
    Ok(())
}
