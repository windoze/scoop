use super::*;

/// Generate the C declarations and static assertions used by the M12 C
/// bridge. Synthetic names intentionally do not expose Scoop source field
/// names; the bridge ABI promises byte layout, not a C-facing typedef API.
pub fn c_layout_assertions(module: &Module) -> Result<String, CodegenError> {
    fn visit(
        module: &Module,
        id: scoop_lir::StructDefId,
        visiting: &mut HashSet<usize>,
        visited: &mut HashSet<usize>,
        order: &mut Vec<scoop_lir::StructDefId>,
    ) -> Result<(), CodegenError> {
        let raw = arena_index(id);
        if visited.contains(&raw) {
            return Ok(());
        }
        if !visiting.insert(raw) {
            return Err(CodegenError(format!(
                "recursive by-value C layout `{}`",
                module.structs[id].name
            )));
        }
        for field in &module.structs[id].fields {
            if let LirType::Struct(nested) = &field.ty {
                if module.structs[*nested].c_layout.is_none() {
                    return Err(CodegenError(format!(
                        "C layout `{}` contains ordinary struct `{}`",
                        module.structs[id].name, module.structs[*nested].name
                    )));
                }
                visit(module, *nested, visiting, visited, order)?;
            }
        }
        visiting.remove(&raw);
        visited.insert(raw);
        order.push(id);
        Ok(())
    }

    fn c_type(module: &Module, ty: &LirType) -> Result<String, CodegenError> {
        Ok(match ty {
            LirType::I1 => "_Bool".to_string(),
            LirType::I64 => "uint64_t".to_string(),
            LirType::Ptr(_) => "void *".to_string(),
            LirType::Struct(id) if module.structs[*id].c_layout.is_some() => {
                format!("scoop_c_layout_{}", arena_index(*id))
            }
            LirType::Enum(id) if matches!(module.enums[*id].repr, EnumRepr::Niche { .. }) => {
                "void *".to_string()
            }
            other => {
                return Err(CodegenError(format!(
                    "non-C type {} reached C bridge layout generation",
                    other.dump()
                )));
            }
        })
    }

    let mut order = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for (id, definition) in module.structs.iter() {
        if definition.c_layout.is_some() {
            visit(module, id, &mut visiting, &mut visited, &mut order)?;
        }
    }

    let mut out = String::from("#include <stddef.h>\n#include <stdint.h>\n\n");
    for id in order {
        let definition = &module.structs[id];
        let name = format!("scoop_c_layout_{}", arena_index(id));
        out.push_str(&format!(
            "typedef struct __attribute__((packed, aligned({}))) {} {{\n",
            definition.align, name
        ));
        let mut cursor = 0u64;
        let mut padding_index = 0usize;
        for (field_index, field) in definition.fields.iter().enumerate() {
            let padding = field.layout.offset.checked_sub(cursor).ok_or_else(|| {
                CodegenError(format!(
                    "overlapping fields in C layout `{}`",
                    definition.name
                ))
            })?;
            if padding != 0 {
                out.push_str(&format!(
                    "  unsigned char _pad_{}[{}];\n",
                    padding_index, padding
                ));
                padding_index += 1;
            }
            out.push_str(&format!(
                "  {} _field_{};\n",
                c_type(module, &field.ty)?,
                field_index
            ));
            cursor = field.layout.offset + c_field_size(&module.structs, &module.enums, &field.ty)?;
        }
        let tail = definition
            .size
            .checked_sub(cursor)
            .ok_or_else(|| CodegenError(format!("fields exceed C layout `{}`", definition.name)))?;
        if tail != 0 {
            out.push_str(&format!(
                "  unsigned char _pad_{}[{}];\n",
                padding_index, tail
            ));
        }
        out.push_str(&format!("}} {};\n", name));
        out.push_str(&format!(
            "_Static_assert(sizeof({}) == {}, \"{} size\");\n",
            name, definition.size, name
        ));
        out.push_str(&format!(
            "_Static_assert(_Alignof({}) == {}, \"{} alignment\");\n",
            name, definition.align, name
        ));
        for (field_index, field) in definition.fields.iter().enumerate() {
            out.push_str(&format!(
                "_Static_assert(offsetof({}, _field_{}) == {}, \"{} field {} offset\");\n",
                name, field_index, field.layout.offset, name, field_index
            ));
        }
        out.push('\n');
    }
    Ok(out)
}

/// Generate the host-C translation unit that owns outbound C ABI wrappers
/// and inbound callback trampolines. `None` means the module needs no second
/// object file.
pub fn c_bridge_source(module: &Module) -> Result<Option<String>, CodegenError> {
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

    fn collect_function_pointers(ty: &scoop_lir::CType, found: &mut Vec<scoop_lir::CType>) {
        if let scoop_lir::CType::FunctionPointer {
            params,
            return_type,
        } = ty
        {
            for parameter in params {
                collect_function_pointers(parameter, found);
            }
            collect_function_pointers(return_type, found);
            if !found.contains(ty) {
                found.push(ty.clone());
            }
        }
    }

    fn type_name(ty: &scoop_lir::CType, function_pointers: &[scoop_lir::CType]) -> String {
        match ty {
            scoop_lir::CType::Unit => "void".to_string(),
            scoop_lir::CType::Int => "int64_t".to_string(),
            scoop_lir::CType::UInt => "uint64_t".to_string(),
            scoop_lir::CType::Boolean => "_Bool".to_string(),
            scoop_lir::CType::Pointer => "void *".to_string(),
            scoop_lir::CType::Struct(id) => {
                format!("scoop_c_layout_{}", arena_index(*id))
            }
            scoop_lir::CType::FunctionPointer { .. } => {
                let index = function_pointers
                    .iter()
                    .position(|candidate| candidate == ty)
                    .expect("function pointer type was collected");
                format!("scoop_c_funptr_{index}")
            }
        }
    }

    let mut function_pointers = Vec::new();
    for (_, function) in &c_externs {
        let ExternFunctionKind::C {
            params,
            return_type,
            ..
        } = &function.kind
        else {
            unreachable!()
        };
        for parameter in params {
            collect_function_pointers(parameter, &mut function_pointers);
        }
        collect_function_pointers(return_type, &mut function_pointers);
    }
    for (_, global) in module.native_globals.iter() {
        collect_function_pointers(&global.c_type, &mut function_pointers);
    }
    for (_, callback) in module.callback_bridges.iter() {
        for parameter in &callback.params {
            collect_function_pointers(parameter, &mut function_pointers);
        }
        collect_function_pointers(&callback.return_type, &mut function_pointers);
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        for parameter in &callback.params {
            collect_function_pointers(parameter, &mut function_pointers);
        }
        collect_function_pointers(&callback.return_type, &mut function_pointers);
    }

    let mut out = c_layout_assertions(module)?;
    out.push_str("#include <string.h>\n\n");
    for (index, ty) in function_pointers.iter().enumerate() {
        let scoop_lir::CType::FunctionPointer {
            params,
            return_type,
        } = ty
        else {
            unreachable!()
        };
        let params = if params.is_empty() {
            "void".to_string()
        } else {
            params
                .iter()
                .map(|parameter| type_name(parameter, &function_pointers))
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "typedef {} (*scoop_c_funptr_{index})({params});\n",
            type_name(return_type, &function_pointers)
        ));
    }
    if !function_pointers.is_empty() {
        out.push('\n');
    }

    let mut declared_symbols = HashSet::new();
    for (id, function) in c_externs {
        let ExternFunctionKind::C {
            bridge_symbol,
            params,
            return_type,
        } = &function.kind
        else {
            unreachable!()
        };
        let parameter_names = params
            .iter()
            .map(|parameter| type_name(parameter, &function_pointers))
            .collect::<Vec<_>>();
        if declared_symbols.insert(function.native_symbol.clone()) {
            let prototype_params = if parameter_names.is_empty() {
                "void".to_string()
            } else {
                parameter_names.join(", ")
            };
            out.push_str(&format!(
                "extern {} {}({});\n",
                type_name(return_type, &function_pointers),
                function.native_symbol,
                prototype_params
            ));
        }

        let has_result = *return_type != scoop_lir::CType::Unit;
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
            let name = type_name(parameter, &function_pointers);
            out.push_str(&format!(
                "  {name} value{index};\n  memcpy(&value{index}, arg{index}, sizeof(value{index}));\n"
            ));
        }
        let arguments = (0..params.len())
            .map(|index| format!("value{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        if has_result {
            let result_name = type_name(return_type, &function_pointers);
            out.push_str(&format!(
                "  {result_name} native_result = {}({arguments});\n  memcpy(result, &native_result, sizeof(native_result));\n",
                function.native_symbol
            ));
        } else {
            out.push_str(&format!("  {}({arguments});\n", function.native_symbol));
        }
        out.push_str("}\n\n");
        let _ = id;
    }
    for (_, global) in module.native_globals.iter() {
        let native_type = type_name(&global.c_type, &function_pointers);
        let thread_local = if global.thread_local {
            "_Thread_local "
        } else {
            ""
        };
        if declared_symbols.insert(global.native_symbol.clone()) {
            out.push_str(&format!(
                "extern {thread_local}{native_type} {};\n",
                global.native_symbol
            ));
        }
        out.push_str(&format!(
            "void {}(void *result) {{\n  memcpy(result, &{}, sizeof({}));\n}}\n\n",
            global.get_bridge_symbol, global.native_symbol, global.native_symbol
        ));
        if let Some(setter) = &global.set_bridge_symbol {
            out.push_str(&format!(
                "void {setter}(const void *value) {{\n  memcpy(&{}, value, sizeof({}));\n}}\n\n",
                global.native_symbol, global.native_symbol
            ));
        }
        out.push_str(&format!(
            "void {}(void *result) {{\n  void *native_address = (void *)&{};\n  memcpy(result, &native_address, sizeof(native_address));\n}}\n\n",
            global.address_bridge_symbol, global.native_symbol
        ));
    }
    for (_, callback) in module.callback_bridges.iter() {
        let has_result = callback.return_type != scoop_lir::CType::Unit;
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
                .map(|(index, parameter)| {
                    format!("{} arg{index}", type_name(parameter, &function_pointers))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "{} {}({callback_params}) {{\n",
            type_name(&callback.return_type, &function_pointers),
            callback.trampoline_symbol
        ));
        if has_result {
            out.push_str(&format!(
                "  {} result;\n",
                type_name(&callback.return_type, &function_pointers)
            ));
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
            .map(|(index, parameter)| {
                format!("{} arg{index}", type_name(parameter, &function_pointers))
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "{} {}({}) {{\n",
            type_name(&callback.return_type, &function_pointers),
            callback.trampoline_symbol,
            if callback_params.is_empty() {
                "void"
            } else {
                &callback_params
            }
        ));
        let has_result = callback.return_type != scoop_lir::CType::Unit;
        if has_result {
            out.push_str(&format!(
                "  {} result = {{0}};\n",
                type_name(&callback.return_type, &function_pointers)
            ));
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
