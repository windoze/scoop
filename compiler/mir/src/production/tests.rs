use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
    ExecutableEntryIdentity, ExportBindingKey, GeneratedCallableKey, LexicalCallableParent,
    LexicalCallableRole, PackagePath, PendingIdentityValidation, PersistentExactTypeId,
    PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment, ValidatedIdentityGraph,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{CallableSignatureRecord, CanonicalMirFoundation, DecodedMirFoundation};

#[test]
fn bridge_section_has_a_fixed_wire_vector_and_validates_against_mir() {
    let fixture = fixture();
    assert_eq!(
        hex(&encode(&fixture.section).unwrap()),
        "a202a100010382a301a2000101582030e4b1927a81fbe498d6b5e02580f89a3da5912f818139892760a6c6ea037df102a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0301a301a2000101582098e824248002f28e6d3b39a8b68c54c2baaf96b7afae9ba5812f2f22f4b2ddb502a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0302"
    );

    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&fixture.section).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Ok(fixture.section)
    );
}

#[test]
fn bridge_reader_rejects_non_closed_products_and_sums() {
    for bytes in [
        vec![0xa1],
        vec![0xa3],
        vec![0xa2, 0x01, 0xa1, 0x00, 0x01, 0x03, 0x80],
        vec![0xa2, 0x02, 0xa1, 0x00, 0x03, 0x03, 0x80],
        vec![0xa2, 0x02, 0xa2, 0x00, 0x01, 0x01, 0x00, 0x03, 0x80],
    ] {
        assert!(decode_canonical::<DecodedCoreBootstrapBridgeSectionV1>(&bytes,).is_err());
    }
}

#[test]
fn reader_rejects_removed_core_production_field() {
    for old_branch in [vec![0xa1, 0x00, 0x01], vec![0xa2, 0x00, 0x02, 0x01, 0xa0]] {
        let mut bytes = encode(&fixture().section).unwrap();
        assert_eq!(bytes[0], 0xa2);
        bytes[0] = 0xa3;
        bytes.splice(1..1, std::iter::once(0x01).chain(old_branch));
        let error = decode_canonical::<DecodedCoreBootstrapBridgeSectionV1>(&bytes).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 2,
                actual: 3
            }
        );
    }
}

#[test]
fn validation_rejects_noncanonical_strong_callable_order() {
    let fixture = fixture();
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact_unit);
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.function.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.other_function.id())),
            signature,
        ),
    ])
    .unwrap();
    let strong_callable_bridges = StrongCallableBridgeSurfaceV1::from_foundation(&mir);
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges
            .with_initialization_cycle(fixture.other_function.id())
            .unwrap(),
    )
    .unwrap();
    let mut decoded = decode(&section);
    decoded.strong_callable_bridges.bridges.swap(0, 1);
    let (mut identities, foundation) = validate_mir(&fixture.hir, &mir);

    assert_eq!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::NonCanonicalStrongCallableOrder { index: 1 })
    );
}

#[test]
fn validation_joins_mixed_callable_owner_variants_by_subject() {
    let fixture = fixture();
    let generated = generated_callable_sorting_before(fixture.function.id());
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact_unit);
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_generated_callables(vec![generated.clone()])
        .unwrap();
    mir.set_callable_signatures(vec![
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.function.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Generated(generated.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(fixture.other_function.id())),
            signature,
        ),
    ])
    .unwrap();
    assert!(matches!(
        mir.callable_signatures()[0].subject(),
        CallableSignatureSubject::Strong(CallableOwner::Generated(_))
    ));

    let strong_callable_bridges = StrongCallableBridgeSurfaceV1::from_foundation(&mir);
    assert!(matches!(
        strong_callable_bridges.bridges()[0].implementation(),
        CallableOwner::Function(_)
    ));
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges
            .with_initialization_cycle(fixture.other_function.id())
            .unwrap(),
    )
    .unwrap();
    let (mut identities, foundation) = validate_mir(&fixture.hir, &mir);

    assert_eq!(
        decode(&section).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Ok(section)
    );
}

#[test]
fn builder_closes_callable_roles_and_entry_implementations() {
    let fixture = fixture();
    let entry = CborIdentityRecord::from_key(source_function("main")).unwrap();
    let unmarked = StrongCallableBridgeSurfaceV1::from_foundation(&fixture.mir);
    assert_eq!(
        unmarked
            .clone()
            .with_initialization_cycle(other_function_id()),
        Err(MirProductionBuildError::MissingStrongInitializationCycle(
            CallableOwner::Function(other_function_id())
        ))
    );
    assert_eq!(
        fixture
            .section
            .strong_callable_bridges
            .clone()
            .with_initialization_cycle(fixture.function.id()),
        Err(MirProductionBuildError::DuplicateInitializationCycle)
    );
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            EntryMirBridgeBranchV1::Executable(Box::new(
                EntryMirBridgeV1::new(
                    entry_source(&entry, fixture.exact_unit),
                    CallableOwner::Function(entry.id())
                )
                .unwrap()
            )),
            fixture.section.strong_callable_bridges.clone()
        ),
        Err(MirProductionBuildError::MissingStrongEntryImplementation(actual))
            if actual == CallableOwner::Function(entry.id())
    ));
    assert!(matches!(
        CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::SINGLE_FILE,
            EntryMirBridgeBranchV1::Executable(Box::new(
                EntryMirBridgeV1::new(
                    entry_source(&entry, fixture.exact_unit),
                    CallableOwner::Function(entry.id())
                )
                .unwrap()
            )),
            unmarked
        ),
        Err(MirProductionBuildError::ForeignEntrySource {
            artifact: ConeIdentity::SINGLE_FILE,
            entry: ConeIdentity::CORE
        })
    ));
}

mod entry;

mod support;
use support::*;
mod roles;
