//! Physical sections consume the semantic data retained by the common reader.

use super::*;
use crate::compile_sections::DecodedCompileMetadataEnvelopes;

pub(crate) struct DecodedCrossConeLinkOnlySections<'input> {
    pub(crate) graph: ValidatedGraphArtifact<'input>,
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    pub(super) cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
}

pub(crate) fn decode_cross_cone_link_only<'input>(
    mut graph: ValidatedGraphArtifact<'input>,
    metadata: &DecodedCompileMetadataEnvelopes<'input>,
) -> Result<DecodedCrossConeLinkOnlySections<'input>, SingleConeLinkSectionDecodeError> {
    let profile = require_strong_profile(
        &graph,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
    )?;
    let production_manifest = decode_production_manifest(&mut graph, profile)?;
    for location in [
        MetadataLocation::Hir,
        MetadataLocation::Mir,
        MetadataLocation::Lir,
    ] {
        profile
            .validate_link_metadata_inventory(location, metadata.envelope(location).sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
    }
    let lir = metadata.envelope(MetadataLocation::Lir);
    let capability = lir_link_identity_closure_capability();
    let payload = required_metadata_section(lir, &capability)?;
    let link_identity_closure = decode_inner(MetadataLocation::Lir, capability, payload)?;
    let capability = lir_cross_cone_link_closure_capability();
    let payload = required_metadata_section(lir, &capability)?;
    let cross_cone_link_closure = decode_inner(MetadataLocation::Lir, capability, payload)?;
    Ok(DecodedCrossConeLinkOnlySections {
        graph,
        link_identity_closure,
        cross_cone_link_closure,
        production_manifest,
    })
}

pub(crate) fn validate_cross_cone_link_from_compile(
    sections: DecodedCrossConeLinkOnlySections<'_>,
    compile: &crate::ValidatedCompileArtifact<crate::CrossConeSemanticsStrongProfile>,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact, StrongLinkArtifactValidationError> {
    let DecodedCrossConeLinkOnlySections {
        graph,
        link_identity_closure,
        cross_cone_link_closure,
        production_manifest,
    } = sections;
    ProductionValidatedSingleConeLinkSections {
        graph,
        identities: compile.shared_identity_graph(),
        foundations: compile.production().shared_foundations(),
        production: compile.production().shared_strong_production(),
        link_identity_closure,
        production_manifest,
    }
    .validate_materializations()
    .map_err(|error| StrongLinkArtifactValidationError::Materializations(Box::new(error)))?
    .validate_c_bridge_envelopes(c_bridge_profile)
    .map_err(|error| StrongLinkArtifactValidationError::CBridge(Box::new(error)))?
    .validate_builtin_objects()
    .map_err(|error| StrongLinkArtifactValidationError::BuiltinObjects(Box::new(error)))?
    .validate_digest_patch_sites()
    .map_err(|error| StrongLinkArtifactValidationError::DigestPatches(Box::new(error)))?
    .validate_registration_objects()
    .map_err(|error| StrongLinkArtifactValidationError::RegistrationObjects(Box::new(error)))?
    .fingerprint_registration_leaves()
    .map_err(|error| StrongLinkArtifactValidationError::RegistrationLeaves(Box::new(error)))?
    .validate_cross_cone_link_symbol_requirements(
        compile.production().lir_cross_cone(),
        cross_cone_link_closure,
        dependency_owners,
        c_bridge_profile,
    )
    .map_err(|error| StrongLinkArtifactValidationError::Symbols(Box::new(error)))?
    .fingerprint_registration_dependencies()
    .map_err(|error| StrongLinkArtifactValidationError::RegistrationDependencies(Box::new(error)))?
    .finalize_strong_objects()
    .map_err(|error| StrongLinkArtifactValidationError::ObjectFinalization(Box::new(error)))?
    .validate_code_and_closures()
    .map_err(|error| StrongLinkArtifactValidationError::FinalProof(Box::new(error)))
}
