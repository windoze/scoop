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

#[test]
fn embedded_files_emit_struct_elements_with_static_child_relocations() {
    let mut module = values_module();
    module.functions.clear();
    module.output = scoop_lir::LirOutput::Library;
    let paths = module.globals.iter().map(|(id, _)| id).collect::<Vec<_>>();
    let byte_array = array_type(
        &mut module.meta,
        "Array<Byte>",
        scoop_lir::ArrayKind::Immutable,
        LirType::I8,
        1,
        1,
        RefScan::None,
    );
    let file_type = module
        .structs
        .alloc_scoop(
            test_physical_exact("EmbeddedFile", scoop_identity::SourceNominalKind::Struct),
            "EmbeddedFile".into(),
            16,
            8,
            false,
            [0, 8]
                .into_iter()
                .map(|offset| scoop_lir::StructField {
                    ty: MANAGED_PTR,
                    layout: scoop_lir::FieldLayout {
                        offset,
                        access_align: 8,
                    },
                })
                .collect(),
        )
        .definition();
    let file_array = array_type(
        &mut module.meta,
        "Array<EmbeddedFile>",
        scoop_lir::ArrayKind::Immutable,
        LirType::Struct(file_type),
        16,
        8,
        RefScan::References(vec![0, 8]),
    );
    let bytes = module.globals.alloc(Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::ByteArrayConst {
            identity: array_identity(0),
            array_type: byte_array,
            bytes: vec![0, 255],
        },
    });
    let bytes_symbol = module.globals[bytes].symbol().to_owned();
    let path_symbols = paths
        .iter()
        .map(|id| module.globals[*id].symbol().to_owned())
        .collect::<Vec<_>>();
    for (ordinal, files) in [
        vec![],
        paths
            .into_iter()
            .map(|path| scoop_lir::EmbeddedFileConst { path, bytes })
            .collect(),
    ]
    .into_iter()
    .enumerate()
    {
        module.globals.alloc(Global {
            address_kind: PointerKind::Managed,
            scan: RefScan::None,
            init: GlobalInit::EmbeddedFileArrayConst {
                identity: array_identity(ordinal as u32 + 1),
                array_type: file_array,
                files,
            },
        });
    }
    let descriptor = type_descriptor_symbol(&module, "Array<EmbeddedFile>");
    let ir = ir_of(&module);
    let objects = ir
        .lines()
        .filter(|line| line.contains("addrspace(1) constant {") && line.contains(&descriptor))
        .collect::<Vec<_>>();
    assert_eq!(objects.len(), 2, "{ir}");
    assert!(
        objects.iter().any(
            |line| line.contains("[0 x { ptr addrspace(1), ptr addrspace(1) }]")
                && line.contains("i64 0")
        ),
        "{ir}"
    );
    let table = objects.iter().find(|line| line.contains("i64 2")).unwrap();
    assert!(
        path_symbols.iter().all(|symbol| table.contains(symbol)),
        "{ir}"
    );
    assert_eq!(table.matches(&bytes_symbol).count(), 2, "{ir}");
    assert!(
        !ir.contains("call ptr addrspace(1) @scoop_runtime_alloc"),
        "{ir}"
    );
}

fn array_identity(ordinal: u32) -> scoop_lir::ImmortalObjectIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site,
        CanonicalIdentifier::new("files").unwrap(),
    ))
    .unwrap();
    let owner = scoop_identity::ImmortalObjectOwner::Property(
        scoop_identity::PropertyOwner::Property(property),
    );
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::ArrayConstant, ordinal),
        [],
    );
    scoop_lir::ImmortalObjectIdentity::from_key(
        scoop_identity::ImmortalObjectKey::array_constant(owner, path),
        scoop_lir::MaterializationRoot::cone_owned(),
    )
    .unwrap()
}
