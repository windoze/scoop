//! Form physical root groups from optimized SSA before statepoint rewriting.

use super::*;

pub(super) fn finalize(
    module: &LlvmModule<'_>,
    expected: &ExpectedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<ExpectedSafepoints, CodegenError> {
    let metadata = module
        .get_context()
        .get_kind_id(STATEPOINT_ROOT_IDENTITY_METADATA);
    let mut final_plan = expected.without_body_sites();
    let mut roots = BTreeMap::<u64, BTreeMap<usize, Vec<ExpectedRoot>>>::new();
    let mut constants = BTreeMap::<u64, Vec<ExpectedRoot>>::new();
    let mut identities = BTreeMap::<u64, BTreeSet<(u8, u32, u64)>>::new();
    for function in module.get_functions() {
        let function_name = function
            .get_name()
            .to_str()
            .map_err(|error| CodegenError(format!("LLVM function name: {error}")))?;
        for block in function.get_basic_blocks() {
            for instruction in block.get_instructions() {
                if let Some((id, root)) = root_identity::read(instruction, metadata)? {
                    let site = expected.sites.get(&id).ok_or_else(|| {
                        CodegenError(format!(
                            "root restoration refers to unknown SafepointId {id}"
                        ))
                    })?;
                    if site.function != function_name
                        || !matches!(site.statepoint, ExpectedStatepoint::Relocating(_))
                    {
                        return Err(CodegenError(format!(
                            "root restoration for {id} has the wrong owner or call protocol"
                        )));
                    }
                    if !identities.entry(id).or_default().insert(root.key()) {
                        return Err(CodegenError(format!(
                            "statepoint {id} repeats typed root identity {root:?}"
                        )));
                    }
                    let value = root_identity::restored_value(
                        instruction,
                        profile.managed_address_space_contract(),
                    )?;
                    if value.is_const() {
                        constants.entry(id).or_default().push(root);
                    } else {
                        roots
                            .entry(id)
                            .or_default()
                            .entry(value.as_value_ref() as usize)
                            .or_default()
                            .push(root);
                    }
                }
                let Some(id) = site_id(instruction)? else {
                    continue;
                };
                let site = expected.sites.get(&id).ok_or_else(|| {
                    CodegenError(format!(
                        "optimized LLVM contains unexpected SafepointId {id}"
                    ))
                })?;
                if site.function != function_name {
                    return Err(CodegenError(format!(
                        "optimized SafepointId {id} changed its callable owner"
                    )));
                }
                if final_plan.sites.insert(id, site.clone()).is_some() {
                    return Err(CodegenError(format!(
                        "optimization duplicated SafepointId {id}"
                    )));
                }
            }
        }
    }
    for (id, site) in &mut final_plan.sites {
        let ExpectedStatepoint::Relocating(initial) = &site.statepoint else {
            continue;
        };
        let actual = identities.remove(id).unwrap_or_default();
        if actual != initial.identities() {
            return Err(CodegenError(format!(
                "optimized statepoint {id} in {} has incomplete typed leaf restorations: expected {:?}, observed {actual:?}",
                site.function,
                initial.identities()
            )));
        }
        let mut groups = roots
            .remove(id)
            .unwrap_or_default()
            .into_values()
            .collect::<Vec<_>>();
        for group in &mut groups {
            group.sort_unstable_by_key(|root| root.key());
        }
        groups.sort_unstable_by_key(|group| group[0].key());
        site.statepoint = ExpectedStatepoint::Relocating(ExpectedRoots {
            groups,
            constants: constants.remove(id).unwrap_or_default(),
        });
    }
    if let Some(id) = identities.keys().next() {
        return Err(CodegenError(format!(
            "restoration remains for deleted SafepointId {id}"
        )));
    }
    Ok(final_plan)
}

fn site_id(instruction: InstructionValue<'_>) -> Result<Option<u64>, CodegenError> {
    if !matches!(
        instruction.get_opcode(),
        InstructionOpcode::Call | InstructionOpcode::Invoke
    ) {
        return Ok(None);
    }
    // SAFETY: the opcode above establishes a CallBase.
    let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
    if let Some(attribute) = call.get_string_attribute(AttributeLoc::Function, "statepoint-id") {
        return attribute
            .get_string_value()
            .to_str()
            .ok()
            .and_then(|text| text.parse().ok())
            .map(Some)
            .ok_or_else(|| CodegenError("invalid statepoint-id call attribute".into()));
    }
    let Some(callee) = call.get_called_fn_value() else {
        return Ok(None);
    };
    if !callee
        .get_name()
        .to_bytes()
        .starts_with(b"llvm.experimental.gc.statepoint")
    {
        return Ok(None);
    }
    let raw = instruction.as_value_ref();
    if unsafe { LLVMGetNumArgOperands(raw) } == 0 {
        return Err(CodegenError("statepoint id operand is missing".into()));
    }
    let operand = unsafe { LLVMGetOperand(raw, 0) };
    if unsafe { LLVMIsAConstantInt(operand) }.is_null() {
        return Err(CodegenError("statepoint id operand is not constant".into()));
    }
    Ok(Some(unsafe { LLVMConstIntGetZExtValue(operand) }))
}
