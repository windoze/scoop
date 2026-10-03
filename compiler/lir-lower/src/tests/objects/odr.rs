//! Nominal application shapes retain their source group and actual content.

use super::*;
use scoop_identity::{ConeCoordinate, LinkageClass, OdrGroupId, OdrMemberRole};
use scoop_wire::{decode_canonical, encode};

fn fixture(producer: ConeIdentity, shuffled: bool) -> (lir::ConeLirOutput, OdrGroupId) {
    let mut builder = Builder::new();
    if shuffled {
        builder.class("Unused", None, &[], vec![], vec![]);
    }
    let class = builder.class(
        "Box",
        None,
        &[("first", mir::Type::Any), ("second", mir::Type::Any)],
        vec![],
        vec![],
    );
    builder.classes[class].type_arguments = vec![mir::Type::Any];
    for (name, value) in [("first", 1), ("second", 2)] {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", mir::Type::Class(class)));
        builder.user_fn_body(
            name,
            vec![param("this", mir::Type::Class(class), this)],
            INT,
            returning_body(locals, int_expr(value)),
        );
    }
    let main = builder.main(Arena::new(), vec![]);
    let mut exact_types = Vec::new();
    let argument = test_exact_type(&builder.function_types, &mir::Type::Any, &mut exact_types);
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Box").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let origin = PersistentGenericTypeId::from_source_declaration(&declaration).unwrap();
    let arguments = NonEmptyVec::from_first(argument, []);
    let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
        origin,
        arguments: arguments.clone(),
    })
    .unwrap();
    let group =
        CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
    let group_id = group.id();
    exact_types.push(
        mir::SourceExactTypeIdentity::checked(
            mir::Type::Class(class),
            exact,
            mir::SourceExactTypeOrigin::NominalApplication(group),
        )
        .unwrap(),
    );
    let mut module = builder.finish_with_types(main, producer, exact_types);
    module.output = mir::MirOutput::Library;
    (try_lower(module).unwrap(), group_id)
}

fn shapes(
    module: &lir::Module,
    foundation: &lir::ConeLirFoundation,
) -> lir::CanonicalShapeLirDefinitionsV1 {
    let immortals = lir::StrongImmortalObjectSemanticPlanSetV1::from_module(module).unwrap();
    let storages = lir::StrongStaticStorageSemanticPlanSetV1::from_module(module).unwrap();
    lir::CanonicalShapeLirDefinitionsV1::from_module(
        module,
        foundation,
        immortals.objects().iter().copied(),
        storages.storages(),
    )
    .unwrap()
}

#[test]
fn nominal_shape_content_ignores_producer_and_arena_order() {
    let mut records = Vec::new();
    for (name, shuffled) in [("first", false), ("second", true)] {
        let producer = ConeCoordinate::new("test", name, "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let (output, group) = fixture(producer, shuffled);
        let canonical = shapes(output.module(), output.foundation());
        assert_eq!(canonical.definitions().len(), 6);
        for definition in canonical.definitions() {
            assert_eq!(group, definition.group());
        }
        let descriptor = output
            .module()
            .meta
            .type_descriptors
            .iter()
            .find(|(_, descriptor)| descriptor.diagnostic_name == "Box")
            .unwrap()
            .1;
        assert_eq!(
            descriptor.identity.symbol_request().linkage(),
            LinkageClass::OdrWeak
        );
        let bytes = encode(&canonical).unwrap();
        let decoded: lir::DecodedCanonicalShapeLirDefinitionsV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.validate(output.foundation()).unwrap(), canonical);
        records.push(canonical);
    }
    assert_eq!(records[0], records[1]);
}

#[test]
fn nominal_shape_fingerprints_include_fields_scans_and_dispatch_order() {
    let (output, _) = fixture(ConeIdentity::SINGLE_FILE, false);
    let foundation = output.foundation().clone();
    let mut module = output.into_module();
    let original = shapes(&module, &foundation);
    let layout = module
        .meta
        .layouts
        .iter()
        .find(|(_, layout)| layout.name == "Box" && layout.fields.len() == 2)
        .unwrap()
        .0;
    let descriptor = module
        .meta
        .type_descriptors
        .iter()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "Box")
        .unwrap()
        .0;
    module.meta.layouts[layout].name = "diagnostic layout name".into();
    assert_eq!(original, shapes(&module, &foundation));

    module.meta.layouts[layout].fields.swap(0, 1);
    let reordered = shapes(&module, &foundation);
    for definition in original.definitions() {
        let current = reordered.get(definition.member()).unwrap();
        if definition.entity()
            == lir::StrongDefinitionEntity::layout(
                module.meta.layouts[layout].identity.layout_record().id(),
            )
        {
            assert_ne!(definition.fingerprint(), current.fingerprint());
            assert_ne!(definition.abi(), current.abi());
        } else {
            assert_eq!(definition, current);
        }
    }
    module.meta.layouts[layout].fields.swap(0, 1);

    module.meta.type_descriptors[descriptor].diagnostic_name = "renamed diagnostic".into();
    let renamed = shapes(&module, &foundation);
    for definition in original.definitions() {
        let current = renamed.get(definition.member()).unwrap();
        assert_eq!(definition.abi(), current.abi());
        assert_eq!(
            definition.fingerprint() != current.fingerprint(),
            definition.role() == OdrMemberRole::TypeDescriptor,
        );
    }
    module.meta.type_descriptors[descriptor].diagnostic_name = "Box".into();

    let scan = lir::RefScan::References(vec![16]);
    let old_kind = std::mem::replace(
        &mut module.meta.layouts[layout].kind,
        lir::LayoutKind::Plain { scan: scan.clone() },
    );
    let old_shape = module.meta.type_descriptors[descriptor]
        .instance_shape
        .clone();
    module.meta.type_descriptors[descriptor].instance_shape =
        lir::TypeInstanceShapeV1::fixed_object(
            module.meta.target_profile,
            module.meta.layouts[layout].size,
            module.meta.layouts[layout].align,
            scan,
        )
        .unwrap();
    let rescanned = shapes(&module, &foundation);
    assert!(original.definitions().iter().any(|definition| {
        definition.role() == OdrMemberRole::ScanProgram
            && definition.fingerprint() != rescanned.get(definition.member()).unwrap().fingerprint()
    }));
    module.meta.layouts[layout].kind = old_kind;
    module.meta.type_descriptors[descriptor].instance_shape = old_shape;

    let slots = module.meta.type_descriptors[descriptor].vtable.slots_mut();
    slots.extend([0, 1].map(|id| lir::DispatchEntry {
        callable: lir::CallableRef::Local(lir::LocalFunctionId::from_u32(id)),
    }));
    let ordered = shapes(&module, &foundation);
    module.meta.type_descriptors[descriptor]
        .vtable
        .slots_mut()
        .reverse();
    let reversed = shapes(&module, &foundation);
    for definition in ordered.definitions() {
        let current = reversed.get(definition.member()).unwrap();
        assert_eq!(
            definition.fingerprint() != current.fingerprint(),
            definition.role() == OdrMemberRole::DispatchTable,
        );
        assert_eq!(
            definition.abi() != current.abi(),
            definition.role() == OdrMemberRole::DispatchTable,
        );
    }
}

#[test]
fn nominal_shape_reader_rejects_missing_duplicate_unsorted_and_unknown_members() {
    let (output, _) = fixture(ConeIdentity::SINGLE_FILE, false);
    let canonical = shapes(output.module(), output.foundation());
    let bytes = encode(&canonical).unwrap();
    let count = canonical.definitions().len();
    assert_eq!(bytes[0], 0x80 + u8::try_from(count).unwrap());
    let width = (bytes.len() - 1) / count;
    assert_eq!(bytes.len(), 1 + count * width);
    let validate = |bytes: &[u8]| {
        decode_canonical::<lir::DecodedCanonicalShapeLirDefinitionsV1>(bytes)
            .unwrap()
            .validate(output.foundation())
    };
    assert!(matches!(
        validate(&[0x80]),
        Err(lir::CanonicalShapeLirError::MemberSet)
    ));

    let mut missing = bytes[..bytes.len() - width].to_vec();
    missing[0] -= 1;
    assert!(matches!(
        validate(&missing),
        Err(lir::CanonicalShapeLirError::MemberSet)
    ));
    let mut duplicate = bytes.clone();
    duplicate[1 + width..1 + 2 * width].copy_from_slice(&bytes[1..1 + width]);
    assert!(matches!(
        validate(&duplicate),
        Err(lir::CanonicalShapeLirError::MemberSet)
    ));
    let mut unsorted = bytes.clone();
    unsorted[1..1 + width].copy_from_slice(&bytes[1 + width..1 + 2 * width]);
    unsorted[1 + width..1 + 2 * width].copy_from_slice(&bytes[1..1 + width]);
    assert!(matches!(
        validate(&unsorted),
        Err(lir::CanonicalShapeLirError::MemberSet)
    ));
    let mut unknown = bytes;
    let member = canonical.definitions()[0].member();
    let offset = unknown
        .windows(32)
        .position(|bytes| bytes == member.as_array())
        .unwrap();
    unknown[offset] ^= 1;
    assert!(matches!(
        validate(&unknown),
        Err(lir::CanonicalShapeLirError::UnknownMember(_))
    ));
}
