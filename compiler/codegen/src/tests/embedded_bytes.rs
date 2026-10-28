use super::*;

#[test]
fn embedded_bytes_emit_readonly_objects_with_exact_array_descriptors() {
    let mut module = values_module();
    module.functions.clear();
    module.globals = Arena::new();
    module.output = scoop_lir::LirOutput::Library;
    let array = array_type(
        &mut module.meta,
        "Array<Byte>",
        scoop_lir::ArrayKind::Immutable,
        LirType::I8,
        1,
        1,
        RefScan::None,
    );
    let descriptor = type_descriptor_symbol(&module, "Array<Byte>");
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site,
        CanonicalIdentifier::new("bytes").unwrap(),
    ))
    .unwrap();
    let owner = scoop_identity::ImmortalObjectOwner::Property(
        scoop_identity::PropertyOwner::Property(property),
    );
    for (ordinal, bytes) in [vec![], vec![0, 127, 128, 255]].into_iter().enumerate() {
        let path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::ArrayConstant, ordinal as u32),
            [],
        );
        let identity = scoop_lir::ImmortalObjectIdentity::from_key(
            scoop_identity::ImmortalObjectKey::array_constant(owner, path),
            scoop_lir::MaterializationRoot::cone_owned(),
        )
        .unwrap();
        module.globals.alloc(Global {
            address_kind: PointerKind::Managed,
            scan: RefScan::None,
            init: GlobalInit::ByteArrayConst {
                identity,
                array_type: array,
                bytes,
            },
        });
    }
    let ir = ir_of(&module);
    let objects = ir
        .lines()
        .filter(|line| line.contains("addrspace(1) constant {"))
        .collect::<Vec<_>>();
    assert_eq!(objects.len(), 2, "{ir}");
    assert!(
        objects.iter().all(|line| line.contains(&descriptor)),
        "{ir}"
    );
    assert!(
        objects
            .iter()
            .any(|line| line.contains("[0 x i8]") && line.contains("i64 0")),
        "{ir}"
    );
    assert!(
        objects
            .iter()
            .any(|line| line.contains("i64 4") && line.contains(r#"c"\00\7F\80\FF""#)),
        "{ir}"
    );
    assert!(
        !ir.contains("call ptr addrspace(1) @scoop_runtime_alloc"),
        "{ir}"
    );
}
