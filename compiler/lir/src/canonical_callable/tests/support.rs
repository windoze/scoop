use crate::*;
use la_arena::Arena;
use scoop_identity::{CborIdentityRecord, SourceNominalKind};

pub(super) fn value(ty: LirType, size: u64, scan: RefScan) -> AbiValue {
    AbiValue::new(ty, AbiNonZeroLayout::new(size, size.min(8)).unwrap(), scan).unwrap()
}

pub(super) use crate::test_support::module;

pub(super) fn scalar(shuffled: bool) -> Module {
    let mut function = function(
        "canonicalArithmetic",
        vec![
            AbiArgument::Direct(value(LirType::I64, 8, RefScan::None).into()),
            AbiArgument::Direct(value(LirType::I64, 8, RefScan::None).into()),
            AbiArgument::Direct(value(LirType::I1, 1, RefScan::None).into()),
        ],
        AbiReturn::Direct(value(LirType::I64, 8, RefScan::None).into()),
    );
    let slots = locals(
        &mut function,
        shuffled,
        value(LirType::I64, 8, RefScan::None),
    );
    if shuffled {
        function.temps.alloc(Temp { ty: RAW_PTR });
    }
    let first = function.temps.alloc(Temp { ty: LirType::I64 });
    let second = function.temps.alloc(Temp { ty: LirType::I64 });
    let [sum, product] = if shuffled {
        [second, first]
    } else {
        [first, second]
    };
    let first = function.blocks.alloc(block("first"));
    let second = function.blocks.alloc(block("second"));
    let [yes, no] = if shuffled {
        [second, first]
    } else {
        [first, second]
    };
    function.blocks[yes].terminator = Terminator::Return {
        value: Some(Value::Temp(sum)),
    };
    function.blocks[no].terminator = Terminator::Return {
        value: Some(Value::Temp(product)),
    };
    let entry = &mut function.blocks[function.entry];
    entry.instructions = vec![
        Instruction::Store {
            local: slots[0],
            value: Value::Param(0),
        },
        Instruction::Store {
            local: slots[1],
            value: Value::Param(1),
        },
        Instruction::IntegerBinary {
            out: sum,
            kind: IntegerKind::SIGNED_64,
            operation: IntegerBinaryOperation::Add,
            lhs: Value::Local(slots[0]),
            rhs: Value::Local(slots[1]),
        },
        Instruction::IntegerBinary {
            out: product,
            kind: IntegerKind::SIGNED_64,
            operation: IntegerBinaryOperation::Multiply,
            lhs: Value::Temp(sum),
            rhs: Value::IntegerConst(LirIntegerConstant::Signed64(7)),
        },
    ];
    entry.terminator = Terminator::CondBr {
        cond: Value::Param(2),
        then_block: yes,
        else_block: no,
    };
    if shuffled {
        let old_entry = function.entry;
        let entry = std::mem::replace(&mut function.blocks[old_entry], block("unreachable"));
        function.entry = function.blocks.alloc(entry);
    }
    module(vec![function])
}

pub(super) fn function(name: &str, arguments: Vec<AbiArgument>, result: AbiReturn) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(block("entry"));
    Function {
        debug_source: None,
        callable_body: crate::tests::callable_body(name),
        gc_effect: GcEffect::NoGc,
        signature: ScoopAbiSignature::new(arguments, result, CallingConvention::Cdecl),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

pub(super) fn block(name: &str) -> BasicBlock {
    BasicBlock {
        name: name.to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    }
}

pub(super) fn locals(function: &mut Function, shuffled: bool, abi: AbiValue) -> [LocalId; 2] {
    if shuffled {
        function.locals.alloc(Local::new(
            "unused",
            LocalStorage::NonZero(value(RAW_PTR, 8, RefScan::None)),
        ));
    }
    let first = function
        .locals
        .alloc(Local::new("a", LocalStorage::NonZero(abi.clone())));
    let second = function
        .locals
        .alloc(Local::new("b", LocalStorage::NonZero(abi)));
    if shuffled {
        [second, first]
    } else {
        [first, second]
    }
}

pub(super) fn foundation(module: &Module) -> ConeLirFoundation {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(
            module
                .functions
                .iter()
                .map(|function| function.callable_body.identity_record().clone())
                .collect(),
        )
        .unwrap();
    ConeLirFoundation::try_new(module.cone, canonical).unwrap()
}

pub(super) fn odr_foundation(
    module: &mut Module,
    role: scoop_identity::OdrMemberRole,
) -> ConeLirFoundation {
    use scoop_identity::{
        GeneratedCallableKey, OdrMemberDiscriminator, OdrMemberKey, SpecializationKey,
    };
    let exact = crate::tests::test_physical_exact("CanonicalResult", SourceNominalKind::Struct);
    let generated =
        CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineStart { result: exact })
            .unwrap();
    let group =
        CborIdentityRecord::from_key(SpecializationKey::StructuralType { exact_type: exact })
            .unwrap();
    let member = CborIdentityRecord::from_key(
        OdrMemberKey::new(
            group.id(),
            role,
            OdrMemberDiscriminator::GeneratedCallable(generated.id()),
        )
        .unwrap(),
    )
    .unwrap();
    module.functions[0].callable_body = CallableBodyIdentity::for_odr_member(member.key()).unwrap();
    foundation(module)
}
