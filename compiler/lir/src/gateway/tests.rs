use super::*;

mod body;
mod support;
use support::{eager, root};

fn gateway(module: &mut Module) -> &mut Function {
    &mut module.functions[2]
}
fn failure(function: &mut Function) -> &mut BasicBlock {
    let Instruction::Invoke { site } = &function.blocks[function.entry].instructions[2] else {
        panic!("fixture invoke");
    };
    let block = site.unwind();
    &mut function.blocks[block]
}
fn rejects(module: &Module, reason: &str) {
    let error = validate_startup_gateways(module).unwrap_err();
    assert!(error.reason.contains(reason), "{error}");
}

#[test]
fn accepts_managed_nogc_and_eager_gateways() {
    for module in [root(true), root(false), eager()] {
        validate_startup_gateways(&module).unwrap();
    }
}

#[test]
fn rejects_entry_poll_and_cfg_bypasses() {
    let mut module = root(true);
    let function = gateway(&mut module);
    function.blocks[function.entry].instructions.swap(0, 1);
    rejects(&module, "poll first");
    let mut module = root(false);
    let function = gateway(&mut module);
    function.blocks[function.entry].instructions.remove(0);
    rejects(&module, "poll first");
    let mut module = root(true);
    let function = gateway(&mut module);
    function
        .call_targets
        .managed_targets
        .void
        .values_mut()
        .next()
        .unwrap()
        .destination = ManagedCallDestination::Runtime(ManagedRuntimeFunction::GcCollect);
    rejects(&module, "actual safepoint");
    let mut module = eager();
    let function = gateway(&mut module);
    let Instruction::Invoke {
        site: InvokeSite::Managed(site),
    } = &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("fixture invoke");
    };
    site.unwind = site.normal;
    rejects(&module, "distinct success and failure");
    let mut module = root(true);
    let function = gateway(&mut module);
    function.blocks[function.entry].terminator = Terminator::Return {
        value: Some(Value::IntegerConst(LirIntegerConstant::Unsigned32(0))),
    };
    rejects(&module, "distinct success and failure");
}

#[test]
fn rejects_wrong_signature_and_unclosed_statuses() {
    let mut module = root(true);
    gateway(&mut module).gc_effect = GcEffect::NoGc;
    rejects(&module, "C startup ABI");
    let mut module = eager();
    gateway(&mut module).signature =
        ScoopAbiSignature::new(vec![], AbiReturn::UnitVoid, CallingConvention::Cdecl);
    rejects(&module, "C startup ABI");
    for status in [
        None,
        Some(Value::IntegerConst(LirIntegerConstant::Unsigned32(2))),
        Some(Value::IntegerConst(LirIntegerConstant::Signed32(1))),
    ] {
        let mut module = root(true);
        failure(gateway(&mut module)).terminator = Terminator::Return { value: status };
        rejects(&module, "closed statuses");
    }
    let mut module = root(true);
    let function = gateway(&mut module);
    let Instruction::Invoke { site } = &function.blocks[function.entry].instructions[2] else {
        panic!("fixture invoke");
    };
    let normal = site.normal();
    function.blocks[normal].terminator = Terminator::Return {
        value: Some(Value::IntegerConst(LirIntegerConstant::Unsigned32(1))),
    };
    rejects(&module, "closed statuses");
}

#[test]
fn rejects_unbalanced_and_unmaterialized_root_exceptions() {
    let mut module = root(true);
    failure(gateway(&mut module)).instructions.swap(2, 4);
    rejects(&module, "materialize, store and then end");
    let mut module = root(true);
    let instructions = &mut failure(gateway(&mut module)).instructions;
    let Instruction::BeginCatch { out, .. } = instructions[1] else {
        panic!("fixture catch");
    };
    let Instruction::GlobalStore { value, .. } = &mut instructions[3] else {
        panic!("fixture store");
    };
    *value = Value::Temp(out);
    rejects(&module, "materialized exception");
    let mut module = root(true);
    let function = gateway(&mut module);
    function
        .call_targets
        .managed_targets
        .direct
        .values_mut()
        .next()
        .unwrap()
        .destination = ManagedCallDestination::Runtime(ManagedRuntimeFunction::StringConcat);
    rejects(&module, "materialized exception");
    let mut module = root(true);
    let Instruction::BeginCatch { raw, .. } = &mut failure(gateway(&mut module)).instructions[1]
    else {
        panic!("fixture catch");
    };
    *raw = Value::NullPointer(PointerKind::Raw);
    rejects(&module, "landingpad payload");
    let mut module = root(true);
    failure(gateway(&mut module)).instructions.pop();
    rejects(&module, "materialize, store and then end");
    let mut module = eager();
    failure(gateway(&mut module))
        .instructions
        .push(Instruction::EndCatch);
    rejects(&module, "without republishing");
}

#[test]
fn rejects_wrong_gateway_owners_and_targets() {
    let mut module = root(true);
    module.output = LirOutput::Library;
    rejects(&module, "library contains");
    let mut module = root(true);
    module.cone = ConeIdentity::CORE;
    rejects(&module, "owner or main");
    let mut module = root(true);
    module.globals = Arena::new();
    rejects(&module, "dedicated failure storage");
    let mut module = eager();
    module
        .initialization_units
        .values_mut()
        .next()
        .unwrap()
        .schedule = InitializationSchedule::LazyAccess;
    rejects(&module, "lazy initialization");
    let mut module = eager();
    module.initialization_units = Arena::new();
    rejects(&module, "absent initialization unit");
    let mut module = eager();
    let unit = module.initialization_units.values_mut().next().unwrap();
    unit.ensure = unit.initializer;
    rejects(&module, "typed main or ensure");
}
