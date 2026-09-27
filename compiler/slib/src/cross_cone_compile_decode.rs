//! Compile-view HIR-front decoding for the cross-Cone semantics profile.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    DecodedCoreBootstrapInterfaceSectionV1, DecodedCrossConeHirInterfaceSectionV1,
    DecodedHirFoundation,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_lir::{
    DecodedConeProductionSectionV1, DecodedCrossConeLirBridgeSectionV1, DecodedLirFoundation,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedCrossConeMirBridgeSectionV1, DecodedMirFoundation,
};

use crate::{ValidatedGraphArtifact, strong_compile_decode::OdrFreeStrongFoundationSet};

mod decode;
mod front_accessors;
mod front_transitions;
mod surface_validation;

pub use decode::*;
pub use surface_validation::*;

/// Canonically decoded identity, legacy production, and general HIR payloads
/// from one exact `cross-cone-semantics-strong/1` Compile view.
///
/// The MIR dependency bridge is decoded but remains untrusted until the later
/// closure bridge phase. The LIR bridge is decoded but remains untrusted, so
/// this type is neither a complete per-artifact Compile proof nor a semantic
/// closure proof.
#[derive(Debug)]
pub struct DecodedCrossConeHirFrontSections<'input> {
    pub(crate) graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_strong_production: DecodedConeProductionSectionV1,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
}

/// One cross-Cone provider whose HIR/MIR/LIR foundations are structurally
/// valid and ODR-free. General HIR and legacy production payloads remain
/// decoded references and carry no semantic-surface authority yet.
pub struct FoundationValidatedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    lir_strong_production: DecodedConeProductionSectionV1,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
}

/// One cross-Cone provider whose general HIR section contains only typed
/// references resolved by the provider's validated identity authority.
/// Cross-table semantics and dependency routes remain unvalidated.
pub struct ResolvedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    lir_strong_production: DecodedConeProductionSectionV1,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
}

/// One cross-Cone provider whose legacy HIR production surface and general
/// HIR identity references are both validated. General-interface ownership,
/// route, and external-reference semantics remain pending.
pub struct HirProductionValidatedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    lir_strong_production: DecodedConeProductionSectionV1,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
}

#[cfg(test)]
pub(crate) mod tests;
