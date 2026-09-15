use super::render::{CBridgeTypeSurface, CTypeRenderer, c_struct_forward_declarations};
use super::*;

/// Generate the C declarations and static assertions used by the M12 C
/// bridge. Synthetic names intentionally do not expose Scoop source field
/// names; the bridge ABI promises byte layout, not a C-facing typedef API.
pub fn c_layout_assertions(module: &Module) -> Result<String, CodegenError> {
    validation::validate_module(module)?;
    let surface = CBridgeTypeSurface::for_module(module)?;
    render_c_layout_assertions(module, &surface)
}

pub(super) fn c_layout_assertions_for_unit(
    module: &Module,
    surface: &CBridgeTypeSurface,
    plan: &scoop_lir::GeneratedBridgeUnitPlanV1,
) -> Result<String, CodegenError> {
    let planned_layouts = plan
        .static_assert_atom_authorities()
        .iter()
        .map(|atom| match atom.key().atom() {
            scoop_lir::GeneratedBridgeAtomRoleKey::StaticAssertSupport { unit, layout }
                if unit == plan.unit() =>
            {
                Ok(layout)
            }
            _ => Err(CodegenError(format!(
                "generated bridge unit {} carries a non-local static-assert atom",
                plan.unit()
            ))),
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let actual_layouts = physical_layout_fingerprints(module, surface, &planned_layouts)?;
    if actual_layouts != planned_layouts {
        return Err(CodegenError(format!(
            "generated bridge unit {} static-assert plan does not match its physical C type surface",
            plan.unit()
        )));
    }
    render_c_layout_assertions(module, surface)
}

fn physical_layout_fingerprints(
    module: &Module,
    surface: &CBridgeTypeSurface,
    planned: &std::collections::BTreeSet<scoop_lir::CanonicalCAbiLayoutFingerprint>,
) -> Result<std::collections::BTreeSet<scoop_lir::CanonicalCAbiLayoutFingerprint>, CodegenError> {
    if surface.definition_count() != planned.len() {
        return Err(CodegenError(format!(
            "planned {} C layout assertions but the physical C type surface requires {}",
            planned.len(),
            surface.definition_count()
        )));
    }
    let records = module.meta.canonical_c_abi.layouts();
    let mut actual = std::collections::BTreeSet::new();
    for (id, definition) in module.structs.iter() {
        if !definition.is_c_layout() || !surface.defines_struct(id) {
            continue;
        }
        let matches = records
            .iter()
            .filter(|record| planned.contains(&record.fingerprint()))
            .filter(|record| {
                physical_layout_matches(module, id, record.layout(), records, &mut HashSet::new())
            })
            .map(scoop_lir::CanonicalCAbiLayoutFingerprintRecord::fingerprint)
            .collect::<Vec<_>>();
        let [fingerprint] = matches.as_slice() else {
            return Err(CodegenError(format!(
                "physical C layout `{}` does not match exactly one planned canonical layout",
                definition.name
            )));
        };
        if !actual.insert(*fingerprint) {
            return Err(CodegenError(format!(
                "multiple physical C layouts match canonical layout {fingerprint}"
            )));
        }
    }
    Ok(actual)
}

fn physical_layout_matches(
    module: &Module,
    id: scoop_lir::StructDefId,
    canonical: &scoop_lir::CanonicalCAbiLayout,
    records: &[scoop_lir::CanonicalCAbiLayoutFingerprintRecord],
    visiting: &mut HashSet<(usize, scoop_lir::CanonicalCAbiLayoutFingerprint)>,
) -> bool {
    let definition = &module.structs[id];
    let Some(contract) = definition.c_layout() else {
        return false;
    };
    if definition.size != canonical.byte_size()
        || definition.align != canonical.alignment().get()
        || c_layout_override(contract.aligned) != canonical.aligned()
        || c_layout_override(contract.packed) != canonical.packed()
    {
        return false;
    }
    let Some(fields) = definition.c_fields() else {
        return false;
    };
    if fields.len() != canonical.fields().len() {
        return false;
    }
    fields
        .iter()
        .zip(canonical.fields())
        .all(|(physical, canonical)| {
            physical.identity == canonical.field()
                && physical.layout.offset == canonical.offset()
                && physical_storage_matches(
                    module,
                    &physical.ty,
                    canonical.storage(),
                    records,
                    visiting,
                )
        })
}

fn physical_storage_matches(
    module: &Module,
    physical: &scoop_lir::CType,
    canonical: scoop_lir::CanonicalCAbiStorageType,
    records: &[scoop_lir::CanonicalCAbiLayoutFingerprintRecord],
    visiting: &mut HashSet<(usize, scoop_lir::CanonicalCAbiLayoutFingerprint)>,
) -> bool {
    match (physical, canonical) {
        (
            scoop_lir::CType::Integer(kind),
            scoop_lir::CanonicalCAbiStorageType::Integer {
                signedness,
                bit_width,
                ..
            },
        ) => {
            let expected_signedness = match kind.signedness() {
                scoop_lir::IntegerSignedness::Signed => {
                    scoop_lir::CanonicalIntegerSignedness::Signed
                }
                scoop_lir::IntegerSignedness::Unsigned => {
                    scoop_lir::CanonicalIntegerSignedness::Unsigned
                }
            };
            let expected_width = match kind.width() {
                scoop_lir::IntegerWidth::W8 => scoop_lir::CanonicalIntegerBitWidth::Bits8,
                scoop_lir::IntegerWidth::W16 => scoop_lir::CanonicalIntegerBitWidth::Bits16,
                scoop_lir::IntegerWidth::W32 => scoop_lir::CanonicalIntegerBitWidth::Bits32,
                scoop_lir::IntegerWidth::W64 => scoop_lir::CanonicalIntegerBitWidth::Bits64,
            };
            signedness == expected_signedness && bit_width == expected_width
        }
        (scoop_lir::CType::Boolean, scoop_lir::CanonicalCAbiStorageType::Boolean { .. }) => true,
        (
            scoop_lir::CType::DataPointer { pointee, storage },
            scoop_lir::CanonicalCAbiStorageType::DataPointer {
                pointee: canonical_pointee,
                storage: canonical_storage,
                ..
            },
        ) => {
            matches!(
                (pointee, canonical_pointee),
                (
                    scoop_lir::CDataPointee::OpaqueVoid,
                    scoop_lir::CanonicalCDataPointee::OpaqueUnit
                ) | (
                    scoop_lir::CDataPointee::Object(_),
                    scoop_lir::CanonicalCDataPointee::ExactObject(_)
                )
            ) && pointer_storage_matches_data(storage, canonical_storage)
        }
        (
            scoop_lir::CType::CodePointer { storage, .. },
            scoop_lir::CanonicalCAbiStorageType::CodePointer {
                storage: canonical_storage,
                ..
            },
        ) => pointer_storage_matches_code(storage, canonical_storage),
        (
            scoop_lir::CType::Struct(reference),
            scoop_lir::CanonicalCAbiStorageType::Struct { layout, .. },
        ) => {
            let pair = (arena_index(reference.definition()), layout);
            if !visiting.insert(pair) {
                return false;
            }
            let matches = records
                .iter()
                .find(|record| record.fingerprint() == layout)
                .is_some_and(|record| {
                    physical_layout_matches(
                        module,
                        reference.definition(),
                        record.layout(),
                        records,
                        visiting,
                    )
                });
            visiting.remove(&pair);
            matches
        }
        _ => false,
    }
}

fn pointer_storage_matches_data(
    physical: &scoop_lir::CDataPointerStorage,
    canonical: scoop_lir::CanonicalCPointerStorage,
) -> bool {
    matches!(
        (physical, canonical),
        (
            scoop_lir::CDataPointerStorage::Direct,
            scoop_lir::CanonicalCPointerStorage::Direct
        ) | (
            scoop_lir::CDataPointerStorage::Nullable(_),
            scoop_lir::CanonicalCPointerStorage::NullableWrapper(_)
        )
    )
}

fn pointer_storage_matches_code(
    physical: &scoop_lir::CCodePointerStorage,
    canonical: scoop_lir::CanonicalCPointerStorage,
) -> bool {
    matches!(
        (physical, canonical),
        (
            scoop_lir::CCodePointerStorage::Direct,
            scoop_lir::CanonicalCPointerStorage::Direct
        ) | (
            scoop_lir::CCodePointerStorage::Nullable(_),
            scoop_lir::CanonicalCPointerStorage::NullableWrapper(_)
        )
    )
}

fn c_layout_override(value: scoop_lir::LirCLayoutValue) -> scoop_lir::CanonicalCLayoutOverride {
    match value {
        scoop_lir::LirCLayoutValue::Natural => scoop_lir::CanonicalCLayoutOverride::Natural,
        scoop_lir::LirCLayoutValue::A1 => scoop_lir::CanonicalCLayoutOverride::Bytes(
            scoop_lir::CanonicalCLayoutByteAlignment::Bytes1,
        ),
        scoop_lir::LirCLayoutValue::A2 => scoop_lir::CanonicalCLayoutOverride::Bytes(
            scoop_lir::CanonicalCLayoutByteAlignment::Bytes2,
        ),
        scoop_lir::LirCLayoutValue::A4 => scoop_lir::CanonicalCLayoutOverride::Bytes(
            scoop_lir::CanonicalCLayoutByteAlignment::Bytes4,
        ),
        scoop_lir::LirCLayoutValue::A8 => scoop_lir::CanonicalCLayoutOverride::Bytes(
            scoop_lir::CanonicalCLayoutByteAlignment::Bytes8,
        ),
        scoop_lir::LirCLayoutValue::A16 => scoop_lir::CanonicalCLayoutOverride::Bytes(
            scoop_lir::CanonicalCLayoutByteAlignment::Bytes16,
        ),
    }
}

fn render_c_layout_assertions(
    module: &Module,
    surface: &CBridgeTypeSurface,
) -> Result<String, CodegenError> {
    let renderer = CTypeRenderer::new(surface.function_types());

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
        if definition.is_c_layout() && surface.defines_struct(id) {
            visit(module, id, &mut visiting, &mut visited, &mut order)?;
        }
    }

    let mut out = target_profile_assertions(module.meta.target_profile)?;
    out.push_str(&c_struct_forward_declarations(module, surface));
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
