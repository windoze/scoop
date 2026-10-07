use super::*;

mod layout;
mod render;

pub use layout::c_layout_assertions;
use layout::c_layout_assertions_for_unit;
use render::{CBridgeTypeSurface, CTypeRenderer};

pub(crate) const GENERATED_BRIDGE_SIGNATURE_SECTION: (&[u8], &[u8]) = (b"__TEXT", b"__scoop_sig");
pub(crate) const GENERATED_BRIDGE_CONTEXT_SECTION: (&[u8], &[u8]) = (b"__TEXT", b"__scoop_ctx");
pub(crate) const ELF_BRIDGE_SIGNATURE_SECTION: &str = ".rodata.scoop_sig";

/// One canonical generated-C translation unit and the bridge unit whose
/// physical object it must produce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCBridgeSourceUnitV1 {
    unit: scoop_lir::GeneratedBridgeUnitId,
    source: String,
}

impl GeneratedCBridgeSourceUnitV1 {
    pub const fn unit(&self) -> scoop_lir::GeneratedBridgeUnitId {
        self.unit
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Complete canonical generated-C source set for one sealed strong LIR
/// product. The retained plan is the sole authority for source sharding and
/// for the atoms each translation unit is allowed to materialize.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCBridgeSourceSetV1 {
    plan: scoop_lir::GeneratedBridgePlanSetV1,
    units: Vec<GeneratedCBridgeSourceUnitV1>,
}

impl GeneratedCBridgeSourceSetV1 {
    pub const fn plan(&self) -> &scoop_lir::GeneratedBridgePlanSetV1 {
        &self.plan
    }

    pub fn units(&self) -> &[GeneratedCBridgeSourceUnitV1] {
        &self.units
    }
}

/// Render exactly one C translation unit for every canonical generated
/// bridge unit. Empty bridge plans produce an empty source set.
pub fn render_c_bridge_source_set(
    input: &scoop_lir::ConeLirOutput,
) -> Result<GeneratedCBridgeSourceSetV1, CodegenError> {
    validation::validate_native_boundary(input.module())?;
    render_source_set(input.module(), input.foundation())
}

#[cfg(test)]
pub(crate) fn render_c_bridge_source_set_for_module(
    module: &Module,
) -> Result<GeneratedCBridgeSourceSetV1, CodegenError> {
    validation::validate_native_boundary(module)?;
    let foundation = scoop_lir::ConeLirFoundation::from_module(module)
        .map_err(|error| CodegenError(format!("cannot seal generated C bridge input: {error}")))?;
    render_source_set(module, &foundation)
}

fn render_source_set(
    module: &Module,
    foundation: &scoop_lir::ConeLirFoundation,
) -> Result<GeneratedCBridgeSourceSetV1, CodegenError> {
    let plan =
        scoop_lir::GeneratedBridgePlanSetV1::from_foundation(foundation).map_err(|error| {
            CodegenError(format!("cannot plan generated C bridge sources: {error}"))
        })?;
    let mut units = Vec::with_capacity(plan.units().len());
    for unit_plan in plan.units() {
        units.push(GeneratedCBridgeSourceUnitV1 {
            unit: unit_plan.unit(),
            source: render_unit(module, unit_plan)?,
        });
    }
    Ok(GeneratedCBridgeSourceSetV1 { plan, units })
}

fn render_unit(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
) -> Result<String, CodegenError> {
    match *plan.unit_authority().key() {
        scoop_lir::GeneratedBridgeUnitKey::OutboundFunction(_) => {
            let function = module
                .extern_functions
                .iter()
                .find_map(|(_, function)| match &function.kind {
                    ExternFunctionKind::C {
                        bridge, signature, ..
                    } if bridge.unit() == plan.unit() => {
                        Some((function, bridge.as_ref(), signature))
                    }
                    ExternFunctionKind::C { .. } | ExternFunctionKind::Scoop { .. } => None,
                })
                .ok_or_else(|| missing_unit_source(plan.unit()))?;
            require_exact_materialized_plan(plan, function.1, &[])?;
            render_outbound_function(module, plan, function.0, function.1, function.2)
        }
        scoop_lir::GeneratedBridgeUnitKey::GlobalRead(_) => {
            let global = module
                .native_globals
                .iter()
                .find_map(|(_, global)| {
                    let bridge = &module.native_global_bridges.gets[global.access.get()].identity;
                    (bridge.unit() == plan.unit()).then_some((global, bridge))
                })
                .ok_or_else(|| missing_unit_source(plan.unit()))?;
            require_exact_materialized_plan(plan, global.1, &[])?;
            render_native_global(
                module,
                plan,
                global.0,
                global.1,
                NativeGlobalBridgeKind::Read,
            )
        }
        scoop_lir::GeneratedBridgeUnitKey::GlobalWrite(_) => {
            let global = module
                .native_globals
                .iter()
                .find_map(|(_, global)| {
                    let scoop_lir::NativeGlobalAccess::Mutable { set, .. } = global.access else {
                        return None;
                    };
                    let bridge = &module.native_global_bridges.sets[set].identity;
                    (bridge.unit() == plan.unit()).then_some((global, bridge))
                })
                .ok_or_else(|| missing_unit_source(plan.unit()))?;
            require_exact_materialized_plan(plan, global.1, &[])?;
            render_native_global(
                module,
                plan,
                global.0,
                global.1,
                NativeGlobalBridgeKind::Write,
            )
        }
        scoop_lir::GeneratedBridgeUnitKey::GlobalAddress(_) => {
            let global = module
                .native_globals
                .iter()
                .find_map(|(_, global)| {
                    let bridge =
                        &module.native_global_bridges.addresses[global.access.address()].identity;
                    (bridge.unit() == plan.unit()).then_some((global, bridge))
                })
                .ok_or_else(|| missing_unit_source(plan.unit()))?;
            require_exact_materialized_plan(plan, global.1, &[])?;
            render_native_global(
                module,
                plan,
                global.0,
                global.1,
                NativeGlobalBridgeKind::Address,
            )
        }
        scoop_lir::GeneratedBridgeUnitKey::StaticCallbackTrampoline { .. } => {
            let callback = module
                .callback_bridges
                .iter()
                .find_map(|(_, callback)| {
                    (callback.trampoline.entry().unit() == plan.unit()).then_some(callback)
                })
                .ok_or_else(|| missing_unit_source(plan.unit()))?;
            require_exact_materialized_plan(plan, callback.trampoline.entry(), &[])?;
            render_static_callback(module, plan, callback)
        }
        scoop_lir::GeneratedBridgeUnitKey::CallbackTrampoline { .. } => {
            let callback = module
                .foreign_callback_bridges
                .iter()
                .find_map(|(_, callback)| {
                    (callback.trampoline.entry().unit() == plan.unit()).then_some(callback)
                })
                .ok_or_else(|| missing_unit_source(plan.unit()))?;
            require_exact_materialized_plan(
                plan,
                callback.trampoline.entry(),
                &[callback.trampoline.signature_descriptor_record()],
            )?;
            render_foreign_callback(module, plan, callback)
        }
    }
}

fn missing_unit_source(unit: scoop_lir::GeneratedBridgeUnitId) -> CodegenError {
    CodegenError(format!(
        "generated bridge unit {unit} has no matching LIR source entity"
    ))
}

fn require_exact_materialized_plan(
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    entry: &scoop_lir::GeneratedBridgeEntryIdentity,
    associated: &[&scoop_lir::GeneratedBridgeAtomAuthorityRecordV1],
) -> Result<(), CodegenError> {
    if plan.primary_atom_authority() != entry.primary_record() {
        return Err(CodegenError(format!(
            "generated bridge unit {} primary atom does not match its LIR source entity",
            plan.unit()
        )));
    }
    let planned = plan.materialized_associated_atom_authorities();
    if planned.len() != associated.len()
        || planned
            .iter()
            .zip(associated)
            .any(|(planned, actual)| planned != *actual)
    {
        return Err(CodegenError(format!(
            "generated bridge unit {} associated atom plan does not match its LIR source entity",
            plan.unit()
        )));
    }
    Ok(())
}

fn unit_prelude(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    surface: &CBridgeTypeSurface,
) -> Result<String, CodegenError> {
    c_layout_assertions_for_unit(module, surface, plan)
}

fn render_outbound_function(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    function: &scoop_lir::ExternFunction,
    bridge: &scoop_lir::GeneratedBridgeEntryIdentity,
    signature: &scoop_lir::CFunctionType,
) -> Result<String, CodegenError> {
    let surface = CBridgeTypeSurface::for_function(module, signature)?;
    let renderer = CTypeRenderer::new(surface.function_types());
    let mut out = unit_prelude(module, plan, &surface)?;
    let parameter_types = signature
        .params
        .iter()
        .map(|parameter| renderer.declaration(parameter, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let prototype_parameters = if parameter_types.is_empty() {
        "void".to_string()
    } else {
        parameter_types.join(", ")
    };
    let declarator = format!("{}({prototype_parameters})", function.native_symbol);
    out.push_str("extern ");
    out.push_str(&renderer.return_declaration(&signature.return_type, &declarator)?);
    out.push_str(";\n");

    let has_result = !signature.return_type.is_void();
    let mut wrapper_parameters = Vec::new();
    if has_result {
        wrapper_parameters.push("void *result".to_string());
    }
    wrapper_parameters.extend(
        signature
            .params
            .iter()
            .enumerate()
            .map(|(index, _)| format!("const void *arg{index}")),
    );
    if wrapper_parameters.is_empty() {
        wrapper_parameters.push("void".to_string());
    }
    out.push_str(&format!(
        "void {}({}) {{\n",
        bridge.symbol(),
        wrapper_parameters.join(", ")
    ));
    for (index, parameter) in signature.params.iter().enumerate() {
        let declaration = renderer.declaration(parameter, &format!("value{index}"))?;
        out.push_str(&format!(
            "  {declaration};\n  __builtin_memcpy(&value{index}, arg{index}, sizeof(value{index}));\n"
        ));
    }
    let arguments = (0..signature.params.len())
        .map(|index| format!("value{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    if has_result {
        let result_declaration =
            renderer.return_declaration(&signature.return_type, "native_result")?;
        out.push_str(&format!(
            "  {result_declaration} = {}({arguments});\n  __builtin_memcpy(result, &native_result, sizeof(native_result));\n",
            function.native_symbol
        ));
    } else {
        out.push_str(&format!("  {}({arguments});\n", function.native_symbol));
    }
    out.push_str("}\n");
    Ok(out)
}

#[derive(Clone, Copy)]
enum NativeGlobalBridgeKind {
    Read,
    Write,
    Address,
}

fn render_native_global(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    global: &scoop_lir::NativeGlobal,
    bridge: &scoop_lir::GeneratedBridgeEntryIdentity,
    kind: NativeGlobalBridgeKind,
) -> Result<String, CodegenError> {
    let surface = CBridgeTypeSurface::for_value(module, &global.c_type)?;
    let renderer = CTypeRenderer::new(surface.function_types());
    let mut out = unit_prelude(module, plan, &surface)?;
    let thread_local = if global.thread_local {
        "_Thread_local "
    } else {
        ""
    };
    let declaration = renderer.declaration(&global.c_type, &global.native_symbol)?;
    out.push_str(&format!("extern {thread_local}{declaration};\n"));
    match kind {
        NativeGlobalBridgeKind::Read => out.push_str(&format!(
            "void {}(void *result) {{\n  __builtin_memcpy(result, &{}, sizeof({}));\n}}\n",
            bridge.symbol(), global.native_symbol, global.native_symbol
        )),
        NativeGlobalBridgeKind::Write => out.push_str(&format!(
            "void {}(const void *value) {{\n  __builtin_memcpy(&{}, value, sizeof({}));\n}}\n",
            bridge.symbol(), global.native_symbol, global.native_symbol
        )),
        NativeGlobalBridgeKind::Address => out.push_str(&format!(
            "void {}(void *result) {{\n  void *native_address = (void *)&{};\n  __builtin_memcpy(result, &native_address, sizeof(native_address));\n}}\n",
            bridge.symbol(), global.native_symbol
        )),
    }
    Ok(out)
}

fn render_static_callback(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    callback: &scoop_lir::CallbackBridge,
) -> Result<String, CodegenError> {
    let surface =
        CBridgeTypeSurface::for_signature_parts(module, &callback.params, &callback.return_type)?;
    let renderer = CTypeRenderer::new(surface.function_types());
    let mut out = unit_prelude(module, plan, &surface)?;
    let bridge_symbol = match callback.bridge {
        scoop_lir::StaticCallbackTarget::Local(bridge) => module.functions
            [bridge.declaration().into_u32() as usize]
            .symbol()
            .to_string(),
        scoop_lir::StaticCallbackTarget::External(bridge) => module.meta.external_callables[bridge]
            .expected_symbol()
            .symbol()
            .to_string(),
    };
    let bridge_object_symbol = module
        .meta
        .target_profile
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(&bridge_symbol);
    let has_result = !callback.return_type.is_void();
    let mut storage_parameters = Vec::new();
    if has_result {
        storage_parameters.push("void *result".to_string());
    }
    storage_parameters.extend(
        callback
            .params
            .iter()
            .enumerate()
            .map(|(index, _)| format!("const void *arg{index}")),
    );
    if storage_parameters.is_empty() {
        storage_parameters.push("void".to_string());
    }
    out.push_str(&format!(
        "extern void scoop_callback_storage_bridge({}) __asm__(\"{}\");\n",
        storage_parameters.join(", "),
        bridge_object_symbol
    ));

    let callback_parameters = render_named_parameters(&renderer, &callback.params)?;
    let declarator = format!(
        "{}({callback_parameters})",
        callback.trampoline.entry().symbol()
    );
    out.push_str(&renderer.return_declaration(&callback.return_type, &declarator)?);
    out.push_str(" {\n");
    if has_result {
        let declaration = renderer.return_declaration(&callback.return_type, "result")?;
        out.push_str(&format!("  {declaration};\n"));
    }
    let mut storage_arguments = Vec::new();
    if has_result {
        storage_arguments.push("&result".to_string());
    }
    storage_arguments.extend((0..callback.params.len()).map(|index| format!("&arg{index}")));
    out.push_str(&format!(
        "  scoop_callback_storage_bridge({});\n",
        storage_arguments.join(", ")
    ));
    if has_result {
        out.push_str("  return result;\n");
    }
    out.push_str("}\n");
    Ok(out)
}

fn render_foreign_callback(
    module: &Module,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
    callback: &scoop_lir::ForeignCallbackBridge,
) -> Result<String, CodegenError> {
    let surface =
        CBridgeTypeSurface::for_signature_parts(module, &callback.params, &callback.return_type)?;
    let renderer = CTypeRenderer::new(surface.function_types());
    let mut out = unit_prelude(module, plan, &surface)?;
    out.push_str(&format!(
        "extern uint32_t {}(void *context, const void *signature, void *result, const void *const *arguments);\n\n",
        scoop_lir::RuntimeAbiSymbolV1::CallbackInvoke.logical_symbol()
    ));
    let section = match module.meta.target_profile.native_object_format() {
        scoop_lir::NativeObjectFormat::MachO64 => "__TEXT,__scoop_sig",
        scoop_lir::NativeObjectFormat::Elf64 => ELF_BRIDGE_SIGNATURE_SECTION,
    };
    out.push_str(&format!(
        "const unsigned char {} __attribute__((section(\"{section}\"))) = 0;\n",
        callback.trampoline.signature_descriptor_symbol()
    ));
    let parameters = render_named_parameters(&renderer, &callback.params)?;
    let declarator = format!("{}({parameters})", callback.trampoline.entry().symbol());
    out.push_str(&renderer.return_declaration(&callback.return_type, &declarator)?);
    out.push_str(" {\n");
    let has_result = !callback.return_type.is_void();
    if has_result {
        let declaration = renderer.return_declaration(&callback.return_type, "result")?;
        out.push_str(&format!("  {declaration} = {{0}};\n"));
    }
    let argument_indices = (0..callback.params.len())
        .filter(|index| *index != callback.context_index as usize)
        .collect::<Vec<_>>();
    if !argument_indices.is_empty() {
        out.push_str(&format!(
            "  const void *arguments[{}] = {{{}}};\n",
            argument_indices.len(),
            argument_indices
                .iter()
                .map(|index| format!("&arg{index}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out.push_str(&format!(
        "  (void){}(arg{}, &{}, {}, {});\n",
        scoop_lir::RuntimeAbiSymbolV1::CallbackInvoke.logical_symbol(),
        callback.context_index,
        callback.trampoline.signature_descriptor_symbol(),
        if has_result { "&result" } else { "NULL" },
        if argument_indices.is_empty() {
            "NULL"
        } else {
            "arguments"
        }
    ));
    if has_result {
        out.push_str("  return result;\n");
    }
    out.push_str("}\n");
    Ok(out)
}

fn render_named_parameters(
    renderer: &CTypeRenderer<'_>,
    parameters: &[scoop_lir::CType],
) -> Result<String, CodegenError> {
    if parameters.is_empty() {
        return Ok("void".to_string());
    }
    parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| renderer.declaration(parameter, &format!("arg{index}")))
        .collect::<Result<Vec<_>, _>>()
        .map(|parameters| parameters.join(", "))
}
