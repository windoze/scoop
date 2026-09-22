use scoop_hir::{
    CommittedTypeUseSemanticAuthorityV1, TypeSectionDeclarationSemanticAuthority,
    TypeSectionDefaultSemanticAuthority, TypeSectionFoundationSemanticAuthority,
};
use typed_arena::Arena;

use super::*;
use crate::{
    HirProductionValidatedCrossConeLayoutClosure, LayoutHirProviderSemanticAuthoritiesV1,
    LayoutHirPublicAuthorityFactoryV1, LayoutMirProviderSourceAuthorityV1,
    LayoutMirSourceAuthorityFactoryV1,
};

mod abi;
mod ordinary;
mod replay;

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    /// Replays the complete HIR and MIR closure before validating every LIR
    /// bridge, Strong V2 section, and layout/ABI section dependency-first.
    #[allow(clippy::too_many_arguments)]
    pub fn with_checked_lir_layout_abi<'hir_source, P, F, S, D, C, M, L, R>(
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
        lir_authorities: &mut [LayoutLirProviderSourceAuthorityV1<'_, L>],
        use_checked: impl for<'checked> FnOnce(CheckedCrossConeLayoutLirClosureV1<'checked>) -> R,
    ) -> CrossConeLayoutLirSemanticClosureResult<R, P::Error, M::Error, L::Error>
    where
        P: LayoutHirPublicAuthorityFactoryV1,
        F: TypeSectionFoundationSemanticAuthority<P::Error>,
        S: TypeSectionDeclarationSemanticAuthority<P::Error>,
        D: TypeSectionDefaultSemanticAuthority<P::Error>,
        C: CommittedTypeUseSemanticAuthorityV1<P::Error>,
        M: LayoutMirSourceAuthorityFactoryV1,
        L: LayoutLirSourceAuthorityFactoryV1,
    {
        validate_authority_inventory(&self, lir_authorities)?;
        self.with_checked_mir_type_bridge_parts(
            hir_authorities,
            mir_authorities,
            |mir, validations| {
                let lir_arena = Arena::new();
                replay::validate_in_scope(
                    mir,
                    validations,
                    lir_authorities,
                    &lir_arena,
                    use_checked,
                )
            },
        )
        .map_err(|source| CrossConeLayoutLirSemanticClosureError::Mir(Box::new(source)))?
    }
}

fn validate_authority_inventory<HE, ME, L>(
    closure: &HirProductionValidatedCrossConeLayoutClosure<'_>,
    authorities: &[LayoutLirProviderSourceAuthorityV1<'_, L>],
) -> Result<(), CrossConeLayoutLirSemanticClosureError<HE, ME, L::Error>>
where
    L: LayoutLirSourceAuthorityFactoryV1,
{
    let expected = closure.dependency_first().count();
    if authorities.len() != expected {
        return Err(CrossConeLayoutLirSemanticClosureError::AuthorityCount {
            expected,
            actual: authorities.len(),
        });
    }
    for (position, (artifact, authority)) in closure.dependency_first().zip(authorities).enumerate()
    {
        let expected = artifact.identity();
        if authority.provider != expected {
            return Err(CrossConeLayoutLirSemanticClosureError::AuthorityProvider {
                position,
                expected,
                actual: authority.provider,
            });
        }
    }
    Ok(())
}
