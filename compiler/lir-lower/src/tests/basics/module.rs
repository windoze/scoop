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

    assert_eq!(
        crate::callable_body_identity(&module, function),
        expected_callable_body(subject)
    );
}

#[test]
fn lowers_hello_world() {
    let source = hello_world();
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
    let module = lower(&source);

    // Globals: one per MIR string constant, same symbol and value.
    let globals: Vec<(&str, &str)> = module
        .globals
        .iter()
        .map(|(_, g)| match &g.init {
            lir::GlobalInit::StringConst(value) => (g.symbol.as_str(), value.as_str()),
            lir::GlobalInit::CString(value) => (g.symbol.as_str(), value.as_str()),
            lir::GlobalInit::Storage { .. } => unreachable!("hello has no storage globals"),
        })
        .collect();
    assert_eq!(
        globals,
        [("scoop.str.0", "hello, world"), ("scoop.str.1", "!")]
    );

    // Functions keep their mangled symbols; the entry symbol is the
    // fixed `scoop_main`.
    let symbols: Vec<&str> = module.functions.iter().map(|f| f.symbol.as_str()).collect();
    assert_eq!(symbols, ["scoop.helper", mir::ENTRY_SYMBOL]);
    assert_eq!(
        module
            .functions
            .iter()
            .map(|function| &function.callable_body)
            .collect::<Vec<_>>(),
        expected_callable_bodies.iter().collect::<Vec<_>>()
    );
    assert_eq!(module.entry_symbol, mir::ENTRY_SYMBOL);

    // The source declaration's typed intrinsic identity survives through
    // MIR and LIR. String metadata is a required singleton, not a layout
    // or descriptor that codegen has to rediscover by name.
    let string_layout = &module.meta.layouts[module.meta.well_known_layouts.string];
    let string_descriptor = descriptor(&module, module.meta.well_known_type_descriptors.string);
    assert_eq!(
        string_layout.kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(string_descriptor.symbol, lir::STRING_TD_SYMBOL);
    assert_eq!(string_descriptor.runtime_type_id, 1);
    assert!(string_descriptor.vtable.is_empty());
    assert_eq!(
        descriptor_values(&module)
            .filter(|descriptor| descriptor.symbol == lir::STRING_TD_SYMBOL)
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
  global @scoop.str.0 = "hello, world"
  global @scoop.str.1 = "!"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> void <scoop managed nounwind>
  fun @scoop.helper() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call native-borrowed-void-target0 sp<native-borrowed:0> roots=[] sig=void0 (ptr<managed>) extern0(global1)
    t0 = aggregate () : {}
    ret
  fun @scoop_main() -> void
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
  entry @scoop_main
"###);
}

#[test]
fn globals_carry_complete_scans_from_their_concrete_storage_types() {
    let mut module = hello_world();
    let (initial_string, _) = module.strings.iter().next().expect("hello-world string");
    module.globals.alloc(mir::Global {
        name: "managedRoot".to_string(),
        symbol: "scoop.global.managedRoot".to_string(),
        ty: mir::Type::String,
        mutable: true,
        storage: mir::GlobalStorage::Local {
            thread_local: false,
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
        .find(|global| matches!(global.init, lir::GlobalInit::StringConst(_)))
        .expect("string constant");
    assert_eq!(string_constant.scan, lir::RefScan::None);

    let managed = module
        .globals
        .iter()
        .map(|(_, global)| global)
        .find(|global| global.symbol == "scoop.global.managedRoot")
        .expect("managed storage global");
    assert_eq!(managed.scan, lir::RefScan::References(vec![0]));
    assert!(
        lir::dump(&module).contains("global @scoop.global.managedRoot : ptr<managed> scan=refs[0]")
    );
}
