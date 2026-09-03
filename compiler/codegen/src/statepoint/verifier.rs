use super::*;

mod llvm;
mod policies;
mod roots;

use llvm::{call_constant, llvm_value_name};
use policies::verify_function_policies;
use roots::{gc_live_root_identities, gc_live_roots, verify_statepoint_shape};

/// Verify RS4GC output against the exact LIR manifest. This is a defensive
/// compiler/toolchain check, never a source-language fallback.
pub(crate) fn verify_rewritten(
    module: &LlvmModule<'_>,
    expected: &ExpectedSafepoints,
    profile: TargetProfile,
) -> Result<(), CodegenError> {
    verify_function_policies(module, expected, profile)?;
    let managed_address_space = profile.managed_address_space_contract();

    let cast_metadata = module
        .get_context()
        .get_kind_id(TYPED_MANAGED_POINTER_BOUNDARY_METADATA);
    let root_identity_metadata = module
        .get_context()
        .get_kind_id(STATEPOINT_ROOT_IDENTITY_METADATA);
    let mut observed = BTreeMap::<u64, ObservedStatepoint<'_>>::new();
    let mut token_ids = BTreeMap::<usize, u64>::new();
    let mut relocations = Vec::<(InstructionValue<'_>, usize, u64, u64)>::new();

    for function in module.get_functions() {
        let function_name = llvm_value_name(function.as_value_ref())?;
        for block in function.get_basic_blocks() {
            for instruction in block.get_instructions() {
                provenance::verify_pointer_instruction(
                    instruction,
                    cast_metadata,
                    managed_address_space,
                    &function_name,
                )?;
                if !matches!(
                    instruction.get_opcode(),
                    InstructionOpcode::Call | InstructionOpcode::Invoke | InstructionOpcode::CallBr
                ) {
                    continue;
                }
                let raw = instruction.as_value_ref();
                // SAFETY: the opcode check above proves this is a CallBase.
                let callee = unsafe { LLVMGetCalledValue(raw) };
                if callee.is_null() {
                    continue;
                }
                let name = llvm_value_name(callee)?;
                if name.starts_with("llvm.experimental.gc.statepoint") {
                    let id = call_constant(raw, 0, "statepoint id")?;
                    let expected_site = expected.sites.get(&id).ok_or_else(|| {
                        CodegenError(format!(
                            "post-RS4GC function `{function_name}` contains unexpected SafepointId {id}"
                        ))
                    })?;
                    if expected_site.function != function_name {
                        return Err(CodegenError(format!(
                            "SafepointId {id} belongs to LIR function `{}`, but was emitted in `{function_name}`",
                            expected_site.function
                        )));
                    }
                    if expected.functions.get(&function_name) != Some(&GcEffect::Managed) {
                        return Err(CodegenError(format!(
                            "statepoint {id} is emitted outside a managed LIR function"
                        )));
                    }
                    let call_args =
                        u32::try_from(call_constant(raw, 3, "statepoint call-argument count")?)
                            .map_err(|_| {
                                CodegenError(format!(
                                    "statepoint {id} call arguments exceed u32::MAX"
                                ))
                            })?;
                    let flags = call_constant(raw, 4, "statepoint flags")?;
                    if flags != 0 {
                        return Err(CodegenError(format!(
                            "statepoint {id} carries unsupported flags {flags}"
                        )));
                    }
                    let transition_index = 5u32.checked_add(call_args).ok_or_else(|| {
                        CodegenError(format!("statepoint {id} call arguments overflow"))
                    })?;
                    let transition_count = call_constant(
                        raw,
                        transition_index,
                        "statepoint transition-argument count",
                    )?;
                    let deopt_index = transition_index
                        .checked_add(1)
                        .and_then(|index| {
                            u32::try_from(transition_count)
                                .ok()
                                .and_then(|count| index.checked_add(count))
                        })
                        .ok_or_else(|| {
                            CodegenError(format!("statepoint {id} transition arguments overflow"))
                        })?;
                    let deopt_count =
                        call_constant(raw, deopt_index, "statepoint deopt-argument count")?;
                    if transition_count != 0 || deopt_count != 0 {
                        return Err(CodegenError(format!(
                            "statepoint {id} carries transition/deopt inputs"
                        )));
                    }
                    let expected_argument_count = deopt_index.checked_add(1).ok_or_else(|| {
                        CodegenError(format!("statepoint {id} argument count overflows u32"))
                    })?;
                    let argument_count = unsafe { LLVMGetNumArgOperands(raw) };
                    if argument_count != expected_argument_count {
                        return Err(CodegenError(format!(
                            "statepoint {id} has {argument_count} intrinsic arguments, expected {expected_argument_count}"
                        )));
                    }
                    let roots = gc_live_roots(raw, id, managed_address_space)?;
                    let identities = gc_live_root_identities(&roots, root_identity_metadata, id)?;
                    verify_statepoint_shape(
                        id,
                        &expected_site.statepoint,
                        instruction,
                        &roots,
                        &identities,
                        &function_name,
                    )?;
                    if observed
                        .insert(
                            id,
                            ObservedStatepoint {
                                instruction,
                                function,
                                roots,
                                identities,
                                relocations: BTreeMap::new(),
                            },
                        )
                        .is_some()
                    {
                        return Err(CodegenError(format!(
                            "SafepointId {id} produced more than one statepoint"
                        )));
                    }
                    token_ids.insert(raw as usize, id);
                } else if name.starts_with("llvm.experimental.gc.relocate") {
                    // SAFETY: relocate has three required operands; the helper
                    // validates both index operands before extraction.
                    let token = unsafe { LLVMGetOperand(raw, 0) } as usize;
                    let base = call_constant(raw, 1, "gc.relocate base index")?;
                    let derived = call_constant(raw, 2, "gc.relocate derived index")?;
                    relocations.push((instruction, token, base, derived));
                }
            }
        }
    }

    for (instruction, token, base, derived) in relocations {
        if base != derived {
            return Err(CodegenError(format!(
                "gc.relocate uses distinct base/derived indices {base}/{derived}"
            )));
        }
        let id = token_ids.get(&token).ok_or_else(|| {
            CodegenError("gc.relocate references an unknown statepoint token".to_string())
        })?;
        let site = observed
            .get_mut(id)
            .expect("statepoint token was inserted with its id");
        let root_count = u64::try_from(site.roots.len())
            .map_err(|_| CodegenError(format!("statepoint {id} root count exceeds u64::MAX")))?;
        if base >= root_count {
            return Err(CodegenError(format!(
                "gc.relocate for statepoint {id} references root index {base}, but gc-live has {root_count} roots"
            )));
        }
        if site.relocations.insert(base, instruction).is_some() {
            return Err(CodegenError(format!(
                "statepoint {id} produced more than one gc.relocate for root index {base}"
            )));
        }
    }

    let observed_ids = observed.keys().copied().collect::<BTreeSet<_>>();
    let expected_ids = expected.sites.keys().copied().collect::<BTreeSet<_>>();
    if observed_ids != expected_ids {
        let missing = expected_ids
            .difference(&observed_ids)
            .map(|id| {
                let site = &expected.sites[id];
                format!(
                    "{id}:{}:{}:{:?}",
                    site.function, site.block, site.statepoint
                )
            })
            .collect::<Vec<_>>();
        let unexpected = observed_ids
            .difference(&expected_ids)
            .copied()
            .collect::<Vec<_>>();
        return Err(CodegenError(format!(
            "post-RS4GC statepoint ids disagree with complete LIR: missing {missing:?}, unexpected {unexpected:?}"
        )));
    }
    provenance::verify_no_derived_live_through(&observed, managed_address_space)?;
    for (id, site) in &observed {
        match &expected.sites[id].statepoint {
            ExpectedStatepoint::Relocating(_) => {}
            ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => {
                if !site.relocations.is_empty() {
                    return Err(CodegenError(format!(
                        "zero-live statepoint {id} produced gc.relocate instructions"
                    )));
                }
                continue;
            }
        }
        for (index, identity) in site.identities.iter().enumerate() {
            let index = u64::try_from(index)
                .map_err(|_| CodegenError(format!("statepoint {id} root index overflow")))?;
            if !site.relocations.contains_key(&index) {
                return Err(CodegenError(format!(
                    "statepoint {id} root {index} ({:?}+{}) has no gc.relocate",
                    identity.source, identity.byte_offset
                )));
            }
        }
        provenance::verify_no_stale_root_uses(*id, site)?;
    }
    Ok(())
}

pub(super) struct ObservedStatepoint<'ctx> {
    pub(super) instruction: InstructionValue<'ctx>,
    pub(super) function: FunctionValue<'ctx>,
    pub(super) roots: Vec<BasicValueEnum<'ctx>>,
    pub(super) identities: Vec<ExpectedRoot>,
    pub(super) relocations: BTreeMap<u64, InstructionValue<'ctx>>,
}
