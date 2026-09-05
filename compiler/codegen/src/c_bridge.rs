use super::*;

mod layout;
mod render;

pub use layout::c_layout_assertions;
use render::{CTypeRenderer, collect_module_function_types};

/// Generate the host-C translation unit that owns outbound C ABI wrappers
/// and inbound callback trampolines. `None` means the module needs no second
/// object file.
pub fn c_bridge_source(module: &Module) -> Result<Option<String>, CodegenError> {
    validation::validate_module(module)?;
    let c_externs = module
        .extern_functions
        .iter()
        .filter(|(_, function)| matches!(function.kind, ExternFunctionKind::C { .. }))
        .collect::<Vec<_>>();
    if c_externs.is_empty()
        && module.native_globals.is_empty()
        && module.callback_bridges.is_empty()
        && module.foreign_callback_bridges.is_empty()
    {
        return Ok(None);
    }

    let function_types = collect_module_function_types(module)?;
    let renderer = CTypeRenderer::new(&function_types);
    let mut out = c_layout_assertions(module)?;
    out.push_str("#include <string.h>\n\n");
    let mut declared_symbols = HashSet::new();
    for (id, function) in c_externs {
        let ExternFunctionKind::C {
            bridge_symbol,
            signature,
        } = &function.kind
        else {
            unreachable!()
        };
        let params = &signature.params;
        let return_type = &signature.return_type;
        let parameter_names = params
            .iter()
            .map(|parameter| renderer.declaration(parameter, ""))
            .collect::<Result<Vec<_>, _>>()?;
        if declared_symbols.insert(function.native_symbol.clone()) {
            let prototype_params = if parameter_names.is_empty() {
                "void".to_string()
            } else {
                parameter_names.join(", ")
            };
            let declarator = format!("{}({prototype_params})", function.native_symbol);
            out.push_str("extern ");
            out.push_str(&renderer.return_declaration(return_type, &declarator)?);
            out.push_str(";\n");
        }

        let has_result = !return_type.is_void();
        let mut wrapper_params = Vec::new();
        if has_result {
            wrapper_params.push("void *result".to_string());
        }
        wrapper_params.extend(
            params
                .iter()
                .enumerate()
                .map(|(index, _)| format!("const void *arg{index}")),
        );
        if wrapper_params.is_empty() {
            wrapper_params.push("void".to_string());
        }
        out.push_str(&format!(
            "void {}({}) {{\n",
            bridge_symbol,
            wrapper_params.join(", ")
        ));
        for (index, parameter) in params.iter().enumerate() {
            let declaration = renderer.declaration(parameter, &format!("value{index}"))?;
            out.push_str(&format!(
                "  {declaration};\n  memcpy(&value{index}, arg{index}, sizeof(value{index}));\n"
            ));
        }
        let arguments = (0..params.len())
            .map(|index| format!("value{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        if has_result {
            let result_declaration = renderer.return_declaration(return_type, "native_result")?;
            out.push_str(&format!(
                "  {result_declaration} = {}({arguments});\n  memcpy(result, &native_result, sizeof(native_result));\n",
                function.native_symbol
            ));
        } else {
            out.push_str(&format!("  {}({arguments});\n", function.native_symbol));
        }
        out.push_str("}\n\n");
        let _ = id;
    }
    for (_, global) in module.native_globals.iter() {
        let get = &module.native_global_bridges.gets[global.access.get()].symbol;
        let address = &module.native_global_bridges.addresses[global.access.address()].symbol;
        let thread_local = if global.thread_local {
            "_Thread_local "
        } else {
            ""
        };
        if declared_symbols.insert(global.native_symbol.clone()) {
            let declaration = renderer.declaration(&global.c_type, &global.native_symbol)?;
            out.push_str(&format!("extern {thread_local}{declaration};\n"));
        }
        out.push_str(&format!(
            "void {}(void *result) {{\n  memcpy(result, &{}, sizeof({}));\n}}\n\n",
            get, global.native_symbol, global.native_symbol
        ));
        if let scoop_lir::NativeGlobalAccess::Mutable { set, .. } = global.access {
            let setter = &module.native_global_bridges.sets[set].symbol;
            out.push_str(&format!(
                "void {setter}(const void *value) {{\n  memcpy(&{}, value, sizeof({}));\n}}\n\n",
                global.native_symbol, global.native_symbol
            ));
        }
        out.push_str(&format!(
            "void {}(void *result) {{\n  void *native_address = (void *)&{};\n  memcpy(result, &native_address, sizeof(native_address));\n}}\n\n",
            address, global.native_symbol
        ));
    }
    for (_, callback) in module.callback_bridges.iter() {
        let has_result = !callback.return_type.is_void();
        let mut storage_params = Vec::new();
        if has_result {
            storage_params.push("void *result".to_string());
        }
        storage_params.extend(
            callback
                .params
                .iter()
                .enumerate()
                .map(|(index, _)| format!("const void *arg{index}")),
        );
        if storage_params.is_empty() {
            storage_params.push("void".to_string());
        }
        out.push_str(&format!(
            "extern void {}({});\n",
            callback.bridge_symbol,
            storage_params.join(", ")
        ));

        let callback_params = if callback.params.is_empty() {
            "void".to_string()
        } else {
            callback
                .params
                .iter()
                .enumerate()
                .map(|(index, parameter)| renderer.declaration(parameter, &format!("arg{index}")))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        };
        let declarator = format!("{}({callback_params})", callback.trampoline_symbol);
        out.push_str(&renderer.return_declaration(&callback.return_type, &declarator)?);
        out.push_str(" {\n");
        if has_result {
            let declaration = renderer.return_declaration(&callback.return_type, "result")?;
            out.push_str(&format!("  {declaration};\n"));
        }
        let mut storage_args = Vec::new();
        if has_result {
            storage_args.push("&result".to_string());
        }
        storage_args.extend((0..callback.params.len()).map(|index| format!("&arg{index}")));
        out.push_str(&format!(
            "  {}({});\n",
            callback.bridge_symbol,
            storage_args.join(", ")
        ));
        if has_result {
            out.push_str("  return result;\n");
        }
        out.push_str("}\n\n");
    }
    if !module.foreign_callback_bridges.is_empty() {
        out.push_str(
            "extern uint32_t scoop_runtime_callback_invoke(void *context, const void *signature, void *result, const void *const *arguments);\n\n",
        );
    }
    let mut emitted_foreign_trampolines = HashSet::new();
    for (_, callback) in module.foreign_callback_bridges.iter() {
        if !emitted_foreign_trampolines.insert(callback.trampoline_symbol.as_str()) {
            continue;
        }
        out.push_str(&format!(
            "const unsigned char {} = 0;\n",
            callback.signature_symbol
        ));
        let callback_params = callback
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| renderer.declaration(parameter, &format!("arg{index}")))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let callback_params = if callback_params.is_empty() {
            "void"
        } else {
            &callback_params
        };
        let declarator = format!("{}({callback_params})", callback.trampoline_symbol);
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
            "  (void)scoop_runtime_callback_invoke(arg{}, &{}, {}, {});\n",
            callback.context_index,
            callback.signature_symbol,
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
        out.push_str("}\n\n");
    }
    Ok(Some(out))
}
