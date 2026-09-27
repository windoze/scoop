//! Shared foundation identity and structure transactions for the M23-6 view.

use scoop_identity::{IdentityValidationError, ValidatedIdentityGraph};

use super::{
    DecodedCrossConeLayoutCompileSections, FoundationValidatedCrossConeLayoutCompileSections,
    IdentityCheckedCrossConeLayoutCompileSections,
};
use crate::{
    compile_decode::validate_foundation_identity_graph_with_authorities,
    strong_compile_decode::{
        StrongProfileFoundationError, validate_cross_cone_profile_foundations,
    },
};

impl<'input> DecodedCrossConeLayoutCompileSections<'input> {
    /// Validates the complete foundation identity delta against the already
    /// validated identity authorities of the dependency closure.
    pub fn validate_foundation_identities<'authority>(
        self,
        external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
    ) -> Result<IdentityCheckedCrossConeLayoutCompileSections<'input>, IdentityValidationError>
    {
        let Self {
            mut graph,
            view,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        } = self;
        let identities = validate_foundation_identity_graph_with_authorities(
            &mut graph,
            &hir_foundation,
            &mir_foundation,
            &lir_foundation,
            external_authorities,
        )?;
        Ok(IdentityCheckedCrossConeLayoutCompileSections {
            graph,
            view,
            identities,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        })
    }
}

impl<'input> IdentityCheckedCrossConeLayoutCompileSections<'input> {
    /// Validates all three foundation structures and the profile's
    /// `RejectAll` ODR policy as one transition.
    pub fn validate_foundation_structure(
        self,
    ) -> Result<
        FoundationValidatedCrossConeLayoutCompileSections<'input>,
        StrongProfileFoundationError,
    > {
        let Self {
            mut graph,
            view,
            mut identities,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        } = self;
        let foundations = validate_cross_cone_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;
        Ok(FoundationValidatedCrossConeLayoutCompileSections {
            graph,
            view,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        })
    }
}
