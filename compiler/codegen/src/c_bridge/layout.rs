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
