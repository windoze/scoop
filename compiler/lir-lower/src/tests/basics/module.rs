use super::*;

#[test]
fn strong_callable_owner_becomes_the_exact_cone_body_identity() {
    let module = hello_world();
    let function = module.top_level[0];
    let subject = module
        .meta
        .callable_signature_subject(function)
        .expect("hello-world function has a callable subject");
    let mir::CallableSignatureSubject::Strong(owner) = subject else {
        panic!("sealed strong input excludes ODR callable subjects")
    };

    let body = crate::lowering::strong_callable_body_identity(owner);
    assert_eq!(body, expected_callable_body(subject));
    assert_eq!(
        body.symbol_request().linkage(),
        lir::LinkageClass::ConeStrong
    );
    assert_eq!(
        body.symbol_request().key(),
        scoop_identity::PersistentSymbolKey::CallableBody(body.id())
    );
}

#[test]
fn lowers_hello_world() {
    let source = hello_world();
    let string_exact_type = source
        .meta
        .source_exact_types
        .get(&mir::Type::String)
        .expect("hello-world String exact identity")
        .identity_record()
        .id();
    let expected_callable_bodies = source
        .top_level
        .iter()
        .map(|function| {
            expected_callable_body(
                source
                    .meta
                    .callable_signature_subject(*function)
                    .expect("hello-world function has a callable subject"),
            )
        })
        .collect::<Vec<_>>();
    let expected_immortal_objects = source
        .strings
        .iter()
        .map(|(_, string)| {
            CborIdentityRecord::from_key(string.identity.clone())
                .expect("MIR string identity has canonical CBOR")
        })
        .collect::<Vec<_>>();
    let module = lower(source);

    let expected_global_symbols = expected_immortal_objects
        .iter()
        .map(|record| {
            scoop_identity::MangledSymbol::from_key(
                &scoop_identity::PersistentSymbolKey::ImmortalObject(record.id()),
            )
        })
        .collect::<Vec<_>>();
    // Immortal globals preserve the two source strings. The executable also
    // owns one distinct managed failure root for its no-throw gateway.
    let globals: Vec<(&str, &str)> = module
        .globals
        .iter()
        .filter_map(|(_, g)| match &g.init {
            lir::GlobalInit::StringConst { value, .. } | lir::GlobalInit::CString { value, .. } => {
                Some((g.symbol(), value.as_str()))
            }
            lir::GlobalInit::Storage { .. }
            | lir::GlobalInit::RawStorage { .. }
            | lir::GlobalInit::ImportedStorage { .. } => None,
        })
        .collect();
    assert_eq!(
        globals,
        [
            (expected_global_symbols[0].as_str(), "hello, world"),
            (expected_global_symbols[1].as_str(), "!"),
        ]
    );
    let immortal_objects = module
        .globals
        .iter()
        .filter_map(|(_, global)| match &global.init {
            lir::GlobalInit::StringConst { identity, .. } => Some(identity.identity_record()),
            lir::GlobalInit::CString { .. }
            | lir::GlobalInit::Storage { .. }
            | lir::GlobalInit::RawStorage { .. }
            | lir::GlobalInit::ImportedStorage { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        immortal_objects,
        expected_immortal_objects.iter().collect::<Vec<_>>()
    );
    let failure_roots = module
        .globals
        .iter()
        .filter_map(|(_, global)| match &global.init {
            lir::GlobalInit::Storage { identity, .. }
                if identity.identity_record().key().role()
                    == scoop_identity::StorageRole::RootEntryFailureRoot =>
            {
                Some(global)
            }
            lir::GlobalInit::Storage { .. }
            | lir::GlobalInit::RawStorage { .. }
            | lir::GlobalInit::StringConst { .. }
            | lir::GlobalInit::CString { .. }
            | lir::GlobalInit::ImportedStorage { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(failure_roots.len(), 1);
    assert_eq!(failure_roots[0].scan, lir::RefScan::References(vec![0]));

    let symbols: Vec<&str> = module.functions.iter().map(lir::Function::symbol).collect();
    assert_eq!(
        &symbols[..expected_callable_bodies.len()],
        expected_callable_bodies
            .iter()
            .map(lir::CallableBodyIdentity::symbol)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        module.functions[..expected_callable_bodies.len()]
            .iter()
            .map(|function| &function.callable_body)
            .collect::<Vec<_>>(),
        expected_callable_bodies.iter().collect::<Vec<_>>()
    );
    assert_eq!(module.functions.len(), expected_callable_bodies.len() + 1);
    assert!(matches!(
        module
            .functions
            .last()
            .expect("executable root gateway")
            .signature
            .result(),
        lir::AbiReturn::Direct(value) if value.storage_type() == &lir::LirType::I32
    ));
    assert_eq!(
        module
            .executable_entry()
            .expect("test module is executable")
            .declaration()
            .into_u32(),
        1
    );

    // This fixture defines its String role in the current provider.
    let lir::TypeDescriptorRef::Local(string_descriptor) =
        module.meta.well_known_type_descriptors.string
    else {
        panic!("the locally defined String retains its descriptor")
    };
    assert_eq!(
        module.meta.type_descriptors[string_descriptor]
            .identity
            .exact_type(),
        string_exact_type
    );
    assert!(module.meta.external_type_descriptors.is_empty());
    assert!(layout_values(&module).any(|layout| matches!(
        layout.kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    )));
    assert_eq!(
        descriptor_values(&module)
            .filter(|descriptor| descriptor.identity.exact_type() == string_exact_type)
            .count(),
        1
    );
    for representation in lir::IntegerKind::ALL
        .map(lir::IntrinsicTypeRepresentation::Integer)
        .into_iter()
        .chain([lir::IntrinsicTypeRepresentation::Boolean])
    {
        assert!(
            layout_values(&module).any(|layout| {
                layout.kind == lir::LayoutKind::Intrinsic(representation.clone())
            })
        );
    }

    // Golden dump locks the output structure.
    insta::assert_snapshot!(lir::dump(&module), @r#"
    Module
      global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "hello, world"
      global @scoop$1$io$6389e5e8389d22f0e2baac5ee54d46413239a4323769000ee665c277f1d369ec = "!"
      global @scoop$1$ss$9b273ab0bbc562dd7f8e8b0487c0e98f4a7d0781b1cb5aa5b6d69c2d8a7f66b1 : ptr<managed> scan=refs[0]
      extern ef0 write @scoop_rt_write(ptr<managed>) -> void <scoop managed nounwind>
      fun @scoop$1$cb$6cb4a66fa9aacac49c232a58b41d5c6876ef37a3aeb59531418a4b8f28cce6e2() -> void
      block entry
        poll managed-void-target0 sp<managed-poll:0> live=[]
        call native-borrowed-void-target0 sp<native-borrowed:0> roots=[] sig=void0 (ptr<managed>) extern0(global1)
        t0 = aggregate () : {}
        ret
      fun @scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca() -> void
      block entry
        poll managed-void-target1 sp<managed-poll:0> live=[]
        call native-borrowed-void-target0 sp<native-borrowed:0> roots=[] sig=void0 (ptr<managed>) extern0(global0)
        t0 = aggregate () : {}
        call managed-void-target0 sp<managed-call:0> live=[] sig=void1 () local-fn0()
        t1 = aggregate () : {}
        ret
      fun @scoop$1$cb$d3bd523ea7c4b775508c06e622f76772db6a21fddb406c6d3fe7d1f20a2a89c1(i32, ptr<raw>, ptr<raw>) -> i32
      block entry
        poll managed-void-target1 sp<managed-poll:0> live=[]
        call managed-direct-target1 sp<managed-call:0> live=[] t4 = sig=direct1 (ptr<metadata>) -> ptr<managed> runtime @scoop_rt_context_ensure_root(td10)
        invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
        br @success
      block success
        raw_store param2 integer<Int>(0x00000000) align 4
        ret integer<UInt>(0x00000000)
      block failure
        (t0, t1) = landingpad : (exception_record, ptr<raw>)
        t2 = begin_catch t1 : ptr<managed>
        call managed-direct-target0 sp<managed-call:1> live=[t2:ptr<managed>@0] t3 = sig=direct0 (ptr<managed>) -> ptr<managed> runtime @scoop_rt_materialize_exception(t2)
        global_store global2, t3
        end_catch
        ret integer<UInt>(0x00000001)
      td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td10 task-context @scoop$1$td$db9fdace23f2040d3622172122120f4e46c6786180d6495caf23e609df021eac type-id=11462109518987149384 shape=FixedObject minimum-size=24 align=8 parent=none vtable=[] itables=[]
      layout String size=24 align=8 refs=[]
      layout Int8 size=1 align=1 refs=[]
      layout Int16 size=2 align=2 refs=[]
      layout Int size=4 align=4 refs=[]
      layout Long size=8 align=8 refs=[]
      layout UInt8 size=1 align=1 refs=[]
      layout UInt16 size=2 align=2 refs=[]
      layout UInt size=4 align=4 refs=[]
      layout ULong size=8 align=8 refs=[]
      layout Boolean size=1 align=1 refs=[]
      layout task-context value size=8 align=8 refs=[0]
      layout task-context size=24 align=8 refs=[16]
      layout String value size=8 align=8 refs=[0]
      output executable @scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca
    "#);
}

#[test]
fn preserves_library_output_without_an_entry() {
    let mut source = hello_world();
    source.output = mir::MirOutput::Library;

    let module = lower(source);

    assert_eq!(module.output, lir::LirOutput::Library);
    assert_eq!(module.executable_entry(), None);
}

#[test]
fn strong_writer_projects_the_complete_executable_production_section() {
    let section = lower_production(hello_world());
    let image = section
        .digest_finalization_plan()
        .nodes()
        .iter()
        .find(|node| node.kind() == scoop_identity::DigestKind::RuntimeImage)
        .expect("the strong writer always emits one runtime-image node");

    let expected = section
        .digest_finalization_plan()
        .nodes()
        .iter()
        .filter(|node| {
            node.patch_intents().iter().any(|patch| {
                !matches!(
                    patch.key().semantic_field_role(),
                    scoop_identity::DigestSemanticFieldRole::RuntimeImage
                        | scoop_identity::DigestSemanticFieldRole::SourceSignature
                )
            })
        })
        .map(lir::DigestInputRefV1::from_node)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        image
            .direct_inputs()
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert!(matches!(
        section.entry_plan(),
        lir::EntryProductionPlanV1::Executable(_)
    ));
}

#[test]
fn strong_writer_projects_a_deterministic_library_section() {
    let mut first = hello_world();
    first.output = mir::MirOutput::Library;
    let mut second = hello_world();
    second.output = mir::MirOutput::Library;

    let first = lower_production(first);
    let second = lower_production(second);

    assert_eq!(first, second);
    assert_eq!(first.entry_plan(), &lir::EntryProductionPlanV1::Library);
}

#[test]
fn globals_carry_complete_scans_from_their_concrete_storage_types() {
    let mut module = hello_world();
    let (initial_string, _) = module.strings.iter().next().expect("hello-world string");
    let backing_owner = property_owner("managedRoot");
    module.globals.alloc(mir::Global {
        name: "managedRoot".to_string(),
        storage_owner: mir::StaticStorageOwner::PropertyBacking(backing_owner),
        ty: mir::Type::String,
        mutable: true,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::EncodedStaticValue {
                payload: mir::MirConstantImage::String(initial_string),
            },
        },
    });
    let delegate_owner = property_owner("managedDelegate");
    module.globals.alloc(mir::Global {
        name: "managedDelegate".to_string(),
        storage_owner: mir::StaticStorageOwner::PropertyDelegate(delegate_owner),
        ty: mir::Type::String,
        mutable: false,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::EncodedStaticValue {
                payload: mir::MirConstantImage::String(initial_string),
            },
        },
    });
    let singleton_owner = persistent_type("TestSingleton");
    module.globals.alloc(mir::Global {
        name: "singletonRoot".to_string(),
        storage_owner: mir::StaticStorageOwner::SingletonPublishedRoot(singleton_owner),
        ty: mir::Type::String,
        mutable: true,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::EncodedStaticValue {
                payload: mir::MirConstantImage::String(initial_string),
            },
        },
    });

    let module = lower(module);
    let string_constant = module
        .globals
        .iter()
        .map(|(_, global)| global)
        .find(|global| matches!(global.init, lir::GlobalInit::StringConst { .. }))
        .expect("string constant");
    assert_eq!(string_constant.scan, lir::RefScan::None);

    let expected_backing = lir::StaticStorageIdentity::property_backing(
        backing_owner,
        lir::MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let expected_delegate = lir::StaticStorageIdentity::property_delegate(
        delegate_owner,
        lir::MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let expected_singleton = lir::StaticStorageIdentity::singleton_published_root(
        singleton_owner,
        lir::MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let storage_global = |expected: &lir::StaticStorageIdentity| {
        let global = module
            .globals
            .iter()
            .map(|(_, global)| global)
            .find(|global| {
                matches!(
                    &global.init,
                    lir::GlobalInit::Storage { identity, .. } if identity == expected
                )
            })
            .expect("missing storage global");
        let lir::GlobalInit::Storage { identity, .. } = &global.init else {
            unreachable!("the search accepted only storage globals")
        };
        (global, identity)
    };
    let (managed, backing_identity) = storage_global(&expected_backing);
    assert_eq!(managed.scan, lir::RefScan::References(vec![0]));
    assert_eq!(backing_identity, &expected_backing);
    assert_eq!(storage_global(&expected_delegate).1, &expected_delegate);
    assert_eq!(storage_global(&expected_singleton).1, &expected_singleton);
    assert!(lir::dump(&module).contains(&format!(
        "global @{} : ptr<managed> scan=refs[0]",
        expected_backing.symbol()
    )));
}
