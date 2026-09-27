use super::*;

/// The three Link payloads decoded atomically from one strong-profile graph
/// artifact. No identity, fingerprint, object, or manifest value has yet been
/// promoted to a Link proof.
#[derive(Debug)]
pub struct DecodedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) hir_foundation: DecodedHirFoundation,
    pub(super) hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    pub(super) mir_foundation: DecodedMirFoundation,
    pub(super) mir_production: DecodedCoreBootstrapBridgeSectionV1,
    pub(super) lir_foundation: DecodedLirFoundation,
    pub(super) strong_production: DecodedConeProductionSectionV1,
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose complete HIR-to-LIR foundation identity graph passed
/// one transaction. Foundation structure and the strong profile's ODR policy
/// remain unproven.
pub struct IdentityCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: ValidatedIdentityGraph,
    pub(super) hir_foundation: DecodedHirFoundation,
    pub(super) hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    pub(super) mir_foundation: DecodedMirFoundation,
    pub(super) mir_production: DecodedCoreBootstrapBridgeSectionV1,
    pub(super) lir_foundation: DecodedLirFoundation,
    pub(super) strong_production: DecodedConeProductionSectionV1,
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections backed by structurally valid foundations that satisfy the
/// `SingleConeStrongProfile` `RejectAll` ODR policy.
pub struct OdrCheckedSingleConeLinkFoundations<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: ValidatedIdentityGraph,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: DecodedStrongProfileProductionSet,
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose HIR, MIR, and LIR production surfaces were rebuilt
/// from the same ODR-free foundations. Object, closure, and manifest proofs
/// remain separate Link obligations.
pub struct ProductionValidatedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose decoded materializations and archive directory were
/// proven to describe the same complete built-in object member plan. Object
/// bytes remain unverified candidates.
pub struct MaterializationCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: MaterializationCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: Vec<ScoopLirObjectCandidateV1<'input>>,
    pub(super) generated_bridge_objects: Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose manifest C-bridge branch and every generated-C object
/// envelope were checked against the same strong bridge plan and request
/// toolchain profile. Scoop object semantics remain unverified.
pub struct CBridgeCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: MaterializationCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: Vec<ScoopLirObjectCandidateV1<'input>>,
    pub(super) generated_bridge_objects: Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    pub(super) c_bridge_production: VerifiedCBridgeProductionEnvelopeSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Every built-in object member through exact symbol, atom-range,
/// relocation, and current-Cone strong-target closure validation. Digest,
/// registration, image, entry, and final fingerprint proofs remain pending.
pub struct BuiltinObjectCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: crate::DigestPatchInputCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    pub(super) builtin_objects: VerifiedBuiltinObjectStrongRelocationSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose decoded digest materializations were bound to the
/// validated plan and proven against the exact Scoop object bytes. Later
/// registration, image, entry, and final fingerprint proofs remain pending.
pub struct DigestPatchCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    pub(super) digest_patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose normalized stackmaps and all six strong registration
/// object tables were proven against the complete LIR registration-production
/// surface and the exact Scoop object bytes. Image, entry, and final
/// fingerprint proofs remain pending.
pub struct RegistrationObjectCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    pub(super) safepoint_registrations: VerifiedStrongSafepointRegistrationSetV1,
    pub(super) callable_registrations: VerifiedStrongCallableRegistrationSetV1,
    pub(super) type_registrations: VerifiedStrongTypeRegistrationSetV1,
    pub(super) immortal_object_registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    pub(super) static_storage_registrations: VerifiedStrongStaticStorageRegistrationSetV1,
    pub(super) initialization_registrations: VerifiedStrongInitializationRegistrationSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections with registration leaves from verified provisional objects.
/// Callable records await their body leaves. Safepoints already carry their
/// final fingerprints using the normalized stackmaps; other registration
/// dependencies are computed after resolving object symbols.
pub struct RegistrationLeafFingerprintedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    pub(super) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(super) callable_registrations: VerifiedStrongCallableRegistrationSetV1,
    pub(super) type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    pub(super) immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    pub(super) static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    pub(super) initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose member-aware defined owners and every undefined use
/// were rebuilt from verified object relocations and the authoritative core,
/// source-native, runtime/EH, and generated-C requirement registries. The two
/// decoded closure tables have matched those rebuilt values exactly.
pub struct LinkSymbolCheckedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    pub(super) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    pub(super) undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    pub(super) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(super) callable_registrations: VerifiedStrongCallableRegistrationSetV1,
    pub(super) type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    pub(super) immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    pub(super) static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    pub(super) initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose complete registration dependencies and final strong
/// fingerprints have been computed. The six tables are ready for one atomic
/// patch transaction.
pub struct RegistrationDependencyFingerprintedSingleConeLinkSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    pub(super) scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    pub(super) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    pub(super) undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    pub(super) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(super) callables: VerifiedStrongCallableFingerprintSetV1,
    pub(super) types: VerifiedStrongTypeFingerprintSetV1,
    pub(super) immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    pub(super) static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    pub(super) initializations: VerifiedStrongInitializationFingerprintSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose six strong-registration tables, runtime image, and
/// entry branch have been patched into copied object bytes and revalidated.
/// Code/member fingerprints and the final Link identity closure remain.
pub struct FinalizedStrongLinkObjectSections<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    pub(super) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    pub(super) undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    pub(super) final_objects: VerifiedEntryPatchSetV1,
    pub(super) production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Fully validated Link view for one `SingleConeStrongProfile` artifact.
/// The owned production manifest retains the complete Code and final-object
/// proof; no earlier provisional state can be recovered from this value.
pub struct ValidatedSingleConeStrongLinkArtifact<'input> {
    pub(super) graph: ValidatedGraphArtifact<'input>,
    pub(super) identities: Rc<ValidatedIdentityGraph>,
    pub(super) foundations: CanonicalFoundationSet,
    pub(super) production: ValidatedSingleConeStrongProduction,
    pub(super) link_identity_closure: LinkIdentityClosureSectionV1,
    pub(super) production_manifest: SingleConeProductionManifestV1,
}
