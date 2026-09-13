use super::*;

#[test]
fn odr_callable_subject_becomes_the_exact_odr_body_identity() {
    let mut module = hello_world();
    let function = mir::FunctionId::from_raw(99_u32.into());
    let exact = exact_callback_signature().result();
    let generated =
        CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineStart { result: exact })
            .unwrap();
    let group =
        scoop_identity::OdrGroupId::from_key(&scoop_identity::SpecializationKey::StructuralType {
            exact_type: exact,
        })
        .unwrap();
    let member_key = scoop_identity::OdrMemberKey::new(
        group,
        scoop_identity::OdrMemberRole::CallableBody,
        scoop_identity::OdrMemberDiscriminator::GeneratedCallable(generated.id()),
    )
    .unwrap();
    let member = scoop_identity::CallableOdrMemberId::from_key(&member_key).unwrap();
    let subject = mir::CallableSignatureSubject::odr(member);
    module.meta.generated_callables =
        mir::MirGeneratedCallableIdentities::checked(vec![mir::MirGeneratedCallableIdentity::new(
            function, &generated, subject,
        )])
        .unwrap();

    let body = crate::callable_body_identity(&module, function);
    assert_eq!(body, expected_callable_body(subject));
    assert_eq!(body.symbol_request().linkage(), lir::LinkageClass::OdrWeak);
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
    let string_runtime_type = lir::RuntimeTypeMappingRecord::new(string_exact_type).unwrap();
    let string_layout_identity = lir::LayoutIdentity::managed_object(
        string_exact_type,
        lir::LirTargetProfile::DARWIN_AARCH64,
        lir::MaterializationRoot::cone_owned(),
    )
    .unwrap();
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
    let module = lower(&source);

    let expected_global_symbols = expected_immortal_objects
        .iter()
        .map(|record| {
            scoop_identity::MangledSymbol::from_key(
                &scoop_identity::PersistentSymbolKey::ImmortalObject(record.id()),
            )
        })
        .collect::<Vec<_>>();
    // Globals: one per MIR string constant, with a persistent symbol and the
    // unchanged source value.
    let globals: Vec<(&str, &str)> = module
        .globals
        .iter()
        .map(|(_, g)| match &g.init {
            lir::GlobalInit::StringConst { value, .. } => (g.symbol(), value.as_str()),
            lir::GlobalInit::CString { value, .. } => (g.symbol(), value.as_str()),
            lir::GlobalInit::Storage { .. } => unreachable!("hello has no storage globals"),
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
            lir::GlobalInit::CString { .. } | lir::GlobalInit::Storage { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        immortal_objects,
        expected_immortal_objects.iter().collect::<Vec<_>>()
    );

    let symbols: Vec<&str> = module.functions.iter().map(lir::Function::symbol).collect();
    assert_eq!(
        symbols,
        expected_callable_bodies
            .iter()
            .map(lir::CallableBodyIdentity::symbol)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        module
            .functions
            .iter()
            .map(|function| &function.callable_body)
            .collect::<Vec<_>>(),
        expected_callable_bodies.iter().collect::<Vec<_>>()
    );
    assert_eq!(module.entry.declaration().into_u32(), 1);

    // The source declaration's typed intrinsic identity survives through
    // MIR and LIR. String metadata is a required singleton, not a layout
    // or descriptor that codegen has to rediscover by name.
    let string_layout = &module.meta.layouts[module.meta.well_known_layouts.string];
    let string_descriptor = descriptor(&module, module.meta.well_known_type_descriptors.string);
    assert_eq!(string_layout.identity, string_layout_identity);
    assert_eq!(
        string_layout.kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(
        string_descriptor.identity.runtime_abi_symbol(),
        Some(lir::RuntimeAbiSymbolV1::CoreStringTypeDescriptor)
    );
    assert_eq!(string_descriptor.identity.symbol_request(), None);
    assert_eq!(
        string_descriptor.identity.runtime_type(),
        string_runtime_type
    );
    assert!(string_descriptor.vtable.slots().is_empty());
    assert_eq!(
        descriptor_values(&module)
            .filter(
                |descriptor| descriptor.identity.exact_type() == string_runtime_type.exact_type()
            )
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
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "hello, world"
  global @scoop$1$io$6389e5e8389d22f0e2baac5ee54d46413239a4323769000ee665c277f1d369ec = "!"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> void <scoop managed nounwind>
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call native-borrowed-void-target0 sp<native-borrowed:0> roots=[] sig=void0 (ptr<managed>) extern0(global1)
    t0 = aggregate () : {}
    ret
  fun @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee() -> void
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    call native-borrowed-void-target0 sp<native-borrowed:0> roots=[] sig=void0 (ptr<managed>) extern0(global0)
    t0 = aggregate () : {}
    call managed-void-target0 sp<managed-call:0> live=[] sig=void1 () local-fn0()
    t1 = aggregate () : {}
    ret
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
  entry @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee
"###);
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
        storage: mir::GlobalStorage::Local {
            thread_local: false,
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

    let module = lower(&module);
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
