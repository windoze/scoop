use super::*;
use scoop_identity::StrongCallableDefinitionOwner;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DispatchTableKey, ExactTypeKey,
    InitializationUnitKey, LayoutKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, PackagePath, PendingIdentityValidation, PersistentFunctionId,
    PersistentPropertyId, PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId,
    PropertyOwner, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{decode_canonical, encode, encode_runtime};

mod type_references;

fn subjects() -> [ExternalStrongShapeSubjectV1; 10] {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site.clone(),
        CanonicalIdentifier::new("Value").unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site.clone(),
        CanonicalIdentifier::new("call").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site,
        CanonicalIdentifier::new("value").unwrap(),
    ))
    .unwrap();
    let layout = PersistentLayoutId::from_key(&LayoutKey::new(
        exact,
        crate::LirTargetProfile::DARWIN_AARCH64.wire_id(),
        scoop_identity::RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let storage = crate::StaticStorageIdentity::property_backing(
        PropertyOwner::Property(property),
        crate::MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let unit = PersistentInitializationUnitId::from_key(&InitializationUnitKey::TopLevelProperty(
        property,
    ))
    .unwrap();
    let scan = scoop_identity::PersistentScanId::from_key(&scoop_identity::ScanKey::new(
        layout,
        scoop_identity::ScanRole::InlineValue,
    ))
    .unwrap();
    use ExternalStrongShapeSubjectV1 as Subject;
    [
        Subject::Callable(scoop_identity::CallableDefinitionOwner::Strong(
            StrongCallableDefinitionOwner::Function(function),
        )),
        Subject::Layout(layout),
        Subject::Scan(scan),
        Subject::TypeDescriptor(exact),
        Subject::DispatchTable(
            PersistentDispatchTableId::from_key(&DispatchTableKey::vtable(exact)).unwrap(),
        ),
        Subject::TypeRegistration(exact),
        Subject::StaticStorage(storage.identity_record().id()),
        Subject::StaticStorageRegistration(storage.identity_record().id()),
        Subject::InitializationCell(unit),
        Subject::InitializationRegistration(unit),
    ]
}

#[test]
fn all_ten_subjects_have_closed_cbor_and_distinct_runtime_tags() {
    for (index, subject) in subjects().iter().enumerate() {
        let tag = if index == 9 { 11 } else { (index + 1) as u32 };
        let bytes = encode(subject).unwrap();
        assert_eq!(&bytes[..4], &[0xa2, 0, tag as u8, 1]);
        let decoded: DecodedExternalStrongShapeSubjectV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let runtime = encode_runtime(subject).unwrap();
        assert_eq!(&runtime[..4], &tag.to_le_bytes());
        match subject {
            ExternalStrongShapeSubjectV1::Callable(owner) => {
                assert_eq!(&runtime[4..], encode_runtime(owner).unwrap())
            }
            _ => {
                assert_eq!(runtime.len(), 36);
                assert_eq!(&bytes[4..6], &[0x58, 0x20]);
                assert_eq!(&runtime[4..], &bytes[6..]);
            }
        }
        for field_count in [0xa1, 0xa3] {
            let mut bad = bytes.clone();
            bad[0] = field_count;
            assert!(decode_canonical::<DecodedExternalStrongShapeSubjectV1>(&bad).is_err());
        }
    }
    for tag in [0, 10, 12, 255] {
        let mut bytes = vec![0xa2, 0];
        if tag < 24 {
            bytes.push(tag);
        } else {
            bytes.extend([0x18, tag]);
        }
        bytes.extend([1, 0]);
        assert!(decode_canonical::<DecodedExternalStrongShapeSubjectV1>(&bytes).is_err());
    }
}

#[test]
fn role_specific_definitions_do_not_alias_subjects_with_the_same_payload_id() {
    let subjects = subjects();
    for (a, b) in [(3, 5), (6, 7), (8, 9)] {
        let first = subjects[a]
            .expected_definition(ConeIdentity::SINGLE_FILE)
            .unwrap();
        let second = subjects[b]
            .expected_definition(ConeIdentity::SINGLE_FILE)
            .unwrap();
        assert_ne!(first.0, second.0);
        assert_ne!(first.1, second.1);
        assert_eq!(first.0.primary_symbol_key(), Some(first.1));
        assert_eq!(second.0.primary_symbol_key(), Some(second.1));
        let changed_provider = subjects[a].expected_definition(ConeIdentity::CORE).unwrap();
        assert_ne!(first.0, changed_provider.0);
        assert_eq!(first.1, changed_provider.1);
    }
}

#[test]
fn binding_requires_the_exact_symbol_definition_and_unique_primary_atom() {
    let empty = crate::ConeLirFoundation::try_new(
        ConeIdentity::SINGLE_FILE,
        crate::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    for subject in subjects() {
        assert!(matches!(
            StrongShapeDefinitionRefV1::from_foundation(subject, &empty),
            Err(StrongShapeDefinitionError::MissingDefinition(_))
        ));
        let physical = foundation(subject, true, 1);
        let bound = StrongShapeDefinitionRefV1::from_foundation(subject, &physical).unwrap();
        let (key, symbol) = subject
            .expected_definition(ConeIdentity::SINGLE_FILE)
            .unwrap();
        assert_eq!(bound.provider(), ConeIdentity::SINGLE_FILE);
        assert_eq!(bound.subject(), subject);
        assert_eq!(bound.symbol().key(), symbol);
        assert_eq!(
            bound.definition(),
            ObjectDefinitionPlanId::from_key(&key).unwrap()
        );
        assert_eq!(bound.primary(), physical.definition_atoms()[0].id());
        assert!(matches!(
            StrongShapeDefinitionRefV1::from_foundation(subject, &foundation(subject, false, 1)),
            Err(StrongShapeDefinitionError::MissingSymbol(_))
        ));
        for count in [0, 2] {
            assert!(matches!(
                StrongShapeDefinitionRefV1::from_foundation(
                    subject,
                    &foundation(subject, true, count)
                ),
                Err(StrongShapeDefinitionError::PrimaryAtomSet(_))
            ));
        }
    }
}

fn foundation(
    subject: ExternalStrongShapeSubjectV1,
    include_symbol: bool,
    primary_count: usize,
) -> crate::ConeLirFoundation {
    let (key, symbol) = subject
        .expected_definition(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let definition = CborIdentityRecord::from_key(key).unwrap();
    let mut canonical = crate::CanonicalLirFoundation::empty();
    let ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = subjects()[3] else {
        panic!("fixture exact subject");
    };
    let subkeys = [
        DefinitionAtomSubkey::Singleton,
        DefinitionAtomSubkey::ExactType(exact),
    ];
    canonical
        .set_definition_atoms(
            subkeys
                .into_iter()
                .take(primary_count)
                .map(|subkey| {
                    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                        definition.id(),
                        DefinitionAtomRole::Primary,
                        subkey,
                    ))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            include_symbol
                .then(|| PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap())
                .into_iter()
                .collect(),
        )
        .unwrap(),
    );
    crate::ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap()
}

#[test]
fn decoded_ids_must_resolve_in_their_kind_specific_authority() {
    let subject = subjects()[1];
    let ExternalStrongShapeSubjectV1::Layout(layout) = subject else {
        panic!("fixture layout");
    };
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(layout).unwrap();
    let mut identities = pending.finish().unwrap();
    let bytes = encode(&subject).unwrap();
    let decoded: DecodedExternalStrongShapeSubjectV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut identities).unwrap(), subject);
    let mut wrong_kind = bytes;
    wrong_kind[2] = 3;
    let decoded: DecodedExternalStrongShapeSubjectV1 = decode_canonical(&wrong_kind).unwrap();
    assert!(matches!(
        decoded.resolve(&mut identities),
        Err(ExternalShapeSubjectResolutionError::Identity(_))
    ));
}
