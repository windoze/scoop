use super::support::*;
use super::*;
use crate::*;

fn fingerprint(module: &Module) -> Digest256 {
    canonical_callable_lir_fingerprint(module, &module.functions[0]).unwrap()
}

#[test]
fn canonical_body_ignores_arena_order_diagnostic_names_and_unreachable_slots() {
    let original = scalar(false);
    let mut shuffled = scalar(true);
    for (_, local) in shuffled.functions[0].locals.iter_mut() {
        local.name = "renamed local".to_string();
    }
    for (_, block) in shuffled.functions[0].blocks.iter_mut() {
        block.name = "renamed block".to_string();
    }
    assert_ne!(original.functions[0].entry, shuffled.functions[0].entry);
    assert_eq!(fingerprint(&original), fingerprint(&shuffled));
}

#[test]
fn canonical_body_tracks_operands_operations_and_control_flow() {
    let expected = fingerprint(&scalar(false));
    let mut operands = scalar(false);
    let function = &mut operands.functions[0];
    let Instruction::IntegerBinary { rhs, .. } =
        &mut function.blocks[function.entry].instructions[3]
    else {
        panic!("product instruction")
    };
    *rhs = Value::IntegerConst(LirIntegerConstant::Signed64(8));
    assert_ne!(expected, fingerprint(&operands));

    let mut operations = scalar(false);
    let function = &mut operations.functions[0];
    let Instruction::IntegerBinary { operation, .. } =
        &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("sum instruction")
    };
    *operation = IntegerBinaryOperation::Subtract;
    assert_ne!(expected, fingerprint(&operations));

    let mut flow = scalar(false);
    let function = &mut flow.functions[0];
    let Terminator::CondBr {
        then_block,
        else_block,
        ..
    } = &mut function.blocks[function.entry].terminator
    else {
        panic!("conditional branch")
    };
    std::mem::swap(then_block, else_block);
    assert_ne!(expected, fingerprint(&flow));
}

#[test]
fn canonical_body_retains_abi_storage_and_gc_effect() {
    let mut module = scalar(false);
    let expected = fingerprint(&module);
    module.functions[0].gc_effect = GcEffect::Managed;
    assert_ne!(expected, fingerprint(&module));
    module.functions[0].gc_effect = GcEffect::NoGc;
    let signature = &module.functions[0].signature;
    module.functions[0].signature = ScoopAbiSignature::new(
        signature.arguments().to_vec(),
        AbiReturn::Indirect(value(LirType::I64, 8, RefScan::None)),
        CallingConvention::Cdecl,
    );
    assert_ne!(expected, fingerprint(&module));
}

#[test]
fn identical_nominal_layouts_keep_distinct_exact_identity_in_the_body() {
    let with_nominal = |name| {
        let mut module = scalar(false);
        let exact =
            crate::tests::test_physical_exact(name, scoop_identity::SourceNominalKind::Struct);
        let id =
            module
                .structs
                .alloc_scoop(exact, "same name".to_string(), 0, 1, false, Vec::new());
        let signature = &module.functions[0].signature;
        let mut arguments = signature.arguments().to_vec();
        arguments.push(AbiArgument::ElidedZst(
            AbiZst::new(LirType::Struct(id), AbiZeroSizedLayout::new(1).unwrap()).unwrap(),
        ));
        module.functions[0].signature = ScoopAbiSignature::new(
            arguments,
            signature.result().clone(),
            CallingConvention::Cdecl,
        );
        module
    };
    assert_ne!(
        fingerprint(&with_nominal("Left")),
        fingerprint(&with_nominal("Right"))
    );
}

#[test]
fn roots_are_ranked_by_ordinary_uses_and_retain_membership() {
    let original = roots(false);
    let mut shuffled = roots(true);
    assert_eq!(fingerprint(&original), fingerprint(&shuffled));
    let function = &mut shuffled.functions[0];
    let Instruction::ManagedPoll { site } = &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("poll instruction")
    };
    site.live = StatepointLiveSet::new(vec![site.live.as_slice()[0].clone()]).unwrap();
    assert_ne!(fingerprint(&original), fingerprint(&shuffled));
}

#[test]
fn canonical_body_set_is_computed_from_actual_functions() {
    let module = scalar(false);
    assert_eq!(
        fingerprint(&module).to_string(),
        "795e15ef011549138beb4ab03928c4ee59838a7edef6ffa57132a499cf2cea55"
    );
    let foundation = foundation(&module);
    let definitions = CanonicalCallableLirDefinitionsV1::from_module(&module, &foundation).unwrap();
    assert_eq!(definitions.definitions().len(), 1);
    assert_eq!(
        definitions
            .get(module.functions[0].callable_body.id())
            .unwrap()
            .fingerprint(),
        fingerprint(&module)
    );
    assert!(matches!(
        CanonicalCallableLirDefinitionsV1::new(Vec::new(), &foundation),
        Err(CanonicalCallableLirError::BodySet { .. })
    ));
}

#[test]
fn callable_leaf_order_is_independent_of_the_foundation_topology() {
    use scoop_identity::{
        CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
        ExecutableSourceEntryIdentity, PackagePath, PersistentExactTypeId, SourceDeclarationKey,
        SourceDeclarationSite,
    };
    let producer = ConeCoordinate::new("dev.example", "stage3.source-extern", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            producer,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("main").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let source = ExecutableSourceEntryIdentity::try_new(
        &declaration,
        ExactOrdinaryNoArgUnitSignature::new(unit),
    )
    .unwrap();
    let mut main = function("main", Vec::new(), AbiReturn::UnitVoid);
    main.callable_body = CallableBodyIdentity::for_function(source.declaration()).unwrap();
    main.blocks[main.entry].terminator = Terminator::Return { value: None };
    let mut gateway = function(
        "gateway",
        Vec::new(),
        AbiReturn::Direct(value(LirType::I32, 4, RefScan::None)),
    );
    gateway.callable_body =
        CallableBodyIdentity::for_root_gateway(producer, source.main()).unwrap();
    gateway.blocks[gateway.entry].terminator = Terminator::Return {
        value: Some(Value::IntegerConst(LirIntegerConstant::Unsigned32(0))),
    };
    assert!(gateway.callable_body.id() < main.callable_body.id());
    let mut module = module(vec![main, gateway]);
    module.cone = producer;
    let foundation = foundation(&module);
    let ordered = function_bodies(&foundation).collect::<Vec<_>>();
    assert!(ordered[0] > ordered[1]);
    let leaves = CanonicalCallableLirDefinitionsV1::from_module(&module, &foundation).unwrap();
    assert!(leaves.definitions()[0].body() < leaves.definitions()[1].body());
    let decoded = scoop_wire::decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(
        &scoop_wire::encode(&leaves).unwrap(),
    )
    .unwrap();
    assert_eq!(decoded.validate(&foundation).unwrap(), leaves);
}
