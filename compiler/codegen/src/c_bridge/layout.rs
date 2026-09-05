use super::render::{CTypeRenderer, c_struct_forward_declarations, collect_module_function_types};
use super::*;

/// Generate the C declarations and static assertions used by the M12 C
/// bridge. Synthetic names intentionally do not expose Scoop source field
/// names; the bridge ABI promises byte layout, not a C-facing typedef API.
pub fn c_layout_assertions(module: &Module) -> Result<String, CodegenError> {
    validation::validate_c_layouts(module)?;
    let function_types = collect_module_function_types(module)?;
    let renderer = CTypeRenderer::new(&function_types);

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
        let fields = module.structs[id]
            .c_fields()
            .expect("C-layout traversal starts from a C struct");
        for field in fields {
            if let scoop_lir::CType::Struct(reference) = field.ty {
                let nested = reference.definition();
                if !module.structs[nested].is_c_layout() {
                    return Err(CodegenError(format!(
                        "C layout `{}` contains ordinary struct `{}`",
                        module.structs[id].name, module.structs[nested].name
                    )));
                }
                visit(module, nested, visiting, visited, order)?;
            }
        }
        visiting.remove(&raw);
        visited.insert(raw);
        order.push(id);
        Ok(())
    }

    let mut order = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for (id, definition) in module.structs.iter() {
        if definition.is_c_layout() {
            visit(module, id, &mut visiting, &mut visited, &mut order)?;
        }
    }

    let mut out = target_profile_assertions(module.meta.target_profile)?;
    out.push_str(&c_struct_forward_declarations(module));
    out.push_str(&renderer.function_pointer_typedefs()?);
    for id in order {
        let definition = &module.structs[id];
        let fields = definition
            .c_fields()
            .expect("only C-layout structs enter C assertion emission");
        let name = format!("scoop_c_layout_{}", arena_index(id));
        out.push_str(&format!(
            "struct __attribute__((packed, aligned({}))) {} {{\n",
            definition.align, name
        ));
        let mut cursor = 0u64;
        let mut padding_index = 0usize;
        for (field_index, field) in fields.iter().enumerate() {
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
            let field_declaration =
                renderer.declaration(&field.ty, &format!("_field_{field_index}"))?;
            out.push_str(&format!("  {field_declaration};\n"));
            cursor = field.layout.offset
                + c_field_size(&module.structs, &module.enums, &field.ty.storage_type())?;
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
        out.push_str("};\n");
        out.push_str(&format!(
            "_Static_assert(sizeof({}) == {}, \"{} size\");\n",
            name, definition.size, name
        ));
        out.push_str(&format!(
            "_Static_assert(_Alignof({}) == {}, \"{} alignment\");\n",
            name, definition.align, name
        ));
        for (field_index, field) in fields.iter().enumerate() {
            out.push_str(&format!(
                "_Static_assert(offsetof({}, _field_{}) == {}, \"{} field {} offset\");\n",
                name, field_index, field.layout.offset, name, field_index
            ));
        }
        out.push('\n');
    }
    Ok(out)
}

fn target_profile_assertions(profile: scoop_lir::LirTargetProfile) -> Result<String, CodegenError> {
    let data_pointer = profile.data_pointer();
    let code_pointer = profile.code_pointer();
    if data_pointer.null_encoding() != scoop_lir::PointerNullEncoding::AllZeroBits
        || code_pointer.null_encoding() != scoop_lir::PointerNullEncoding::AllZeroBits
        || data_pointer.carrier() != scoop_lir::InternalPointerCarrier::BitPreservingU64
        || code_pointer.carrier() != scoop_lir::InternalPointerCarrier::BitPreservingU64
    {
        return Err(CodegenError(format!(
            "target profile `{}` has no qualified C pointer representation",
            profile.id().canonical_name(),
        )));
    }

    let data_layout = data_pointer.layout();
    let code_layout = code_pointer.layout();
    Ok(format!(
        "#include <limits.h>\n#include <stddef.h>\n#include <stdint.h>\n\n\
typedef void (*scoop_target_function_pointer)(void);\n\
_Static_assert(CHAR_BIT == 8, \"Scoop requires 8-bit bytes\");\n\
_Static_assert(UINTPTR_MAX == UINT64_MAX, \"Scoop uintptr_t value width\");\n\
_Static_assert(sizeof(uintptr_t) == {data_size}, \"Scoop uintptr_t size\");\n\
_Static_assert(_Alignof(uintptr_t) == {data_align}, \"Scoop uintptr_t alignment\");\n\
_Static_assert(sizeof(void *) == {data_size}, \"Scoop data pointer size\");\n\
_Static_assert(_Alignof(void *) == {data_align}, \"Scoop data pointer alignment\");\n\
_Static_assert(sizeof(scoop_target_function_pointer) == {code_size}, \"Scoop function pointer size\");\n\
_Static_assert(_Alignof(scoop_target_function_pointer) == {code_align}, \"Scoop function pointer alignment\");\n\n",
        data_size = data_layout.size_bytes(),
        data_align = data_layout.alignment_bytes(),
        code_size = code_layout.size_bytes(),
        code_align = code_layout.alignment_bytes(),
    ))
}
