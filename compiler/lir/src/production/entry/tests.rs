use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactCallableSignature, ExactTypeKey,
    ExecutableEntryIdentity, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PackagePath, PersistentCallableBodyId, PersistentExactTypeId, PersistentStaticStorageId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite, StaticStorageKey,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use super::{EntryProductionPlanBuildError, EntryProductionPlanV1, EntryProductionSourceV1};
use crate::{
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1,
};

#[test]
fn library_rejects_every_executable_root_surface() {
    let fixture = executable_fixture();
    assert!(matches!(
        EntryProductionPlanV1::new(
            EntryProductionSourceV1::Library,
            &fixture.foundation,
            &fixture.registrations,
            &fixture.digests,
        ),
        Err(EntryProductionPlanBuildError::LibraryRootArtifacts { .. })
    ));
}

#[test]
fn executable_requires_the_derived_root_entities() {
    let foundation = empty_foundation();
    let digests = image_only_digests(&foundation);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    assert!(matches!(
        EntryProductionPlanV1::new(
            EntryProductionSourceV1::executable(executable_source_entry()),
            &foundation,
            &registrations,
            &digests,
        ),
        Err(EntryProductionPlanBuildError::RootGatewaySet { .. })
    ));
}

mod support;
use support::*;
