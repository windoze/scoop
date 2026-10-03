use super::*;

pub(super) fn validate(function: &Function, gateway: Gateway) -> Result<()> {
    let fail = |reason| error(function, reason);
    let AbiReturn::Direct(result) = function.signature.result() else {
        return Err(fail("gateway signature must be C uint32_t(void)"));
    };
    if function.gc_effect != GcEffect::Managed
        || !function.signature.arguments().is_empty()
        || function.signature.calling_convention() != CallingConvention::Cdecl
        || result.storage_type() != &LirType::I32
        || result.layout().size().get() != 4
        || result.layout().alignment().get() != 4
        || result.scan() != &RefScan::None
    {
        return Err(fail("gateway signature must be C uint32_t(void)"));
    }
    let entry = get(&function.blocks, function.entry)
        .ok_or_else(|| fail("gateway entry block is absent"))?;
    let [
        Instruction::ManagedPoll { site: poll },
        Instruction::Invoke { site: invoke },
    ] = entry.instructions.as_slice()
    else {
        return Err(fail("gateway must poll first and then invoke its target"));
    };
    let poll = get(&function.call_targets.managed_targets.void, poll.target)
        .ok_or_else(|| fail("gateway poll target is absent"))?;
    if poll.destination != ManagedCallDestination::Runtime(ManagedRuntimeFunction::Safepoint)
        || !void_signature(function, poll.signature)
    {
        return Err(fail(
            "gateway entry poll must call the actual safepoint runtime",
        ));
    }
    validate_invoke(function, invoke, gateway.entry())?;
    if function.blocks.len() != 3
        || invoke.normal() == invoke.unwind()
        || invoke.normal() == function.entry
        || invoke.unwind() == function.entry
        || !matches!(entry.terminator, Terminator::Br(target) if target == invoke.normal())
    {
        return Err(fail("gateway must have distinct success and failure exits"));
    }
    let success = get(&function.blocks, invoke.normal())
        .ok_or_else(|| fail("gateway success block is absent"))?;
    let failure = get(&function.blocks, invoke.unwind())
        .ok_or_else(|| fail("gateway failure block is absent"))?;
    if !success.instructions.is_empty() || !returns(success, 0) || !returns(failure, 1) {
        return Err(fail(
            "gateway exits must return the closed statuses 0 and 1",
        ));
    }
    let [
        Instruction::LandingPad { record, raw },
        Instruction::BeginCatch {
            out: payload,
            raw: caught,
        },
        tail @ ..,
    ] = failure.instructions.as_slice()
    else {
        return Err(fail("gateway failure must begin a catch on its landingpad"));
    };
    if !temp_is(function, *record, &LirType::ExceptionRecord)
        || !temp_is(function, *raw, &RAW_PTR)
        || !temp_is(function, *payload, &MANAGED_PTR)
        || *caught != Value::Temp(*raw)
    {
        return Err(fail("gateway catch must consume the landingpad payload"));
    }
    match gateway {
        Gateway::Initialization { .. } if matches!(tail, [Instruction::EndCatch]) => Ok(()),
        Gateway::Initialization { .. } => Err(fail(
            "eager gateway must end its catch without republishing failure",
        )),
        Gateway::Root { failure_root, .. } => {
            validate_root_failure(function, *payload, failure_root, tail)
        }
    }
}

fn validate_root_failure(
    function: &Function,
    payload: TempId,
    failure_root: GlobalId,
    instructions: &[Instruction],
) -> Result<()> {
    let fail = |reason| error(function, reason);
    let [
        Instruction::Call {
            site: CallSite::Managed(call),
        },
        Instruction::GlobalStore { global, value },
        Instruction::EndCatch,
    ] = instructions
    else {
        return Err(fail(
            "root catch must materialize, store and then end the catch",
        ));
    };
    let ManagedTypedCall::Direct { target, out, args } = &call.call else {
        return Err(fail("root catch needs a managed exception materialization"));
    };
    let target = get(&function.call_targets.managed_targets.direct, *target)
        .ok_or_else(|| fail("root materialization target is absent"))?;
    if target.destination
        != ManagedCallDestination::Runtime(ManagedRuntimeFunction::MaterializeException)
        || args.as_slice() != [AbiCallArgument::Direct(Value::Temp(payload))]
        || *out == payload
        || !temp_is(function, *out, &MANAGED_PTR)
        || *global != failure_root
        || *value != Value::Temp(*out)
    {
        return Err(fail(
            "root catch must publish the materialized exception to its own failure root",
        ));
    }
    Ok(())
}

fn validate_invoke(
    function: &Function,
    invoke: &InvokeSite,
    entry: LocalFunctionRef,
) -> Result<()> {
    let target = match invoke {
        InvokeSite::Managed(site) => match &site.call {
            ManagedTypedCall::Void { target, args } if args.is_empty() => {
                get(&function.call_targets.managed_targets.void, *target).and_then(|target| {
                    match target.destination {
                        ManagedCallDestination::Local(local) => {
                            Some((LocalFunctionRef::Managed(local), target.signature))
                        }
                        _ => None,
                    }
                })
            }
            _ => None,
        },
        InvokeSite::NoGc(site) => match &site.call {
            NoGcTypedCall::Void { target, args } if args.is_empty() => {
                get(&function.call_targets.no_gc_targets.void, *target).and_then(|target| {
                    match target.destination {
                        NoGcCallDestination::Local(local) => {
                            Some((LocalFunctionRef::NoGc(local), target.signature))
                        }
                        _ => None,
                    }
                })
            }
            _ => None,
        },
    };
    if target
        .is_some_and(|(target, signature)| target == entry && void_signature(function, signature))
    {
        Ok(())
    } else {
        Err(error(
            function,
            "gateway invoke must call its typed main or ensure target",
        ))
    }
}

fn void_signature(function: &Function, signature: VoidCallSignatureId) -> bool {
    get(&function.call_targets.void_signatures, signature).is_some_and(|signature| {
        signature.arguments().is_empty()
            && signature.calling_convention() == CallingConvention::Cdecl
    })
}
fn returns(block: &BasicBlock, status: u32) -> bool {
    matches!(block.terminator, Terminator::Return { value: Some(Value::IntegerConst(LirIntegerConstant::Unsigned32(value))) } if value == status)
}
fn temp_is(function: &Function, temp: TempId, ty: &LirType) -> bool {
    get(&function.temps, temp).is_some_and(|temp| &temp.ty == ty)
}
fn get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
