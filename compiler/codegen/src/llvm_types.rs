use super::*;

/// The LLVM type of a (non-void) LIR type: aggregates are literal
/// structs per the layout in LIR meta; Unit is the empty struct `{}`;
/// enums follow their fixed representation (spec 7.4).
pub(crate) fn basic_ty<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    ty: &LirType,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    Ok(match ty {
        LirType::Void => {
            return Err(CodegenError(
                "void is not a value type (locals, temps, call args)".to_string(),
            ));
        }
        LirType::I1 => context.bool_type().into(),
        LirType::I64 => context.i64_type().into(),
        LirType::Ptr(kind) => pointer_ty(context, managed_address_space, *kind).into(),
        LirType::ExceptionRecord => context
            .struct_type(
                &[
                    context.ptr_type(AddressSpace::default()).into(),
                    context.i32_type().into(),
                ],
                false,
            )
            .into(),
        LirType::Aggregate(elements) => {
            let fields: Vec<BasicTypeEnum> = elements
                .iter()
                .map(|element| basic_ty(context, structs, enums, managed_address_space, element))
                .collect::<Result<_, _>>()?;
            context.struct_type(&fields, false).into()
        }
        LirType::Struct(id) => {
            struct_ty(context, structs, enums, managed_address_space, *id)?.into()
        }
        LirType::Enum(id) => match &enums[*id].repr {
            // Niche optimization: the value is a bare pointer.
            EnumRepr::Niche { .. } => {
                if enums[*id].scan.contains_reference() {
                    managed_ptr_ty(context, managed_address_space).into()
                } else {
                    ptr_ty(context).into()
                }
            }
            EnumRepr::Tagged { size, align, .. } => tagged_ty(
                context,
                managed_address_space,
                *size,
                *align,
                &enums[*id].scan,
            )?
            .into(),
        },
    })
}

pub(crate) fn llvm_constant<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    globals: &[Option<GlobalValue<'ctx>>],
    managed_address_space: ManagedAddressSpace,
    ty: BasicTypeEnum<'ctx>,
    value: &ConstantValue,
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    Ok(match value {
        ConstantValue::Zero => ty.const_zero(),
        ConstantValue::Int(value) => context.i64_type().const_int(*value as u64, true).into(),
        ConstantValue::Bool(value) => context
            .bool_type()
            .const_int(u64::from(*value), false)
            .into(),
        ConstantValue::NullPointer(_) => ty.into_pointer_type().const_null().into(),
        ConstantValue::GlobalPointer { global, kind } => {
            let expected = pointer_ty(context, managed_address_space, *kind);
            if ty.into_pointer_type() != expected {
                return Err(CodegenError(
                    "global pointer constant does not match its storage type".to_string(),
                ));
            }
            let pointer = globals
                .get(global.into_raw().into_u32() as usize)
                .and_then(|global| *global)
                .ok_or_else(|| {
                    CodegenError(format!(
                        "global constant references unavailable global {}",
                        global.into_raw().into_u32()
                    ))
                })?
                .as_pointer_value();
            if pointer.get_type() != expected {
                return Err(CodegenError(
                    "referenced global does not have the declared pointer provenance".to_string(),
                ));
            }
            pointer.into()
        }
        ConstantValue::EnumUnit { enum_id, variant } => match &enums[*enum_id].repr {
            EnumRepr::Niche { payload_variant } => {
                if *variant > 1 || variant == payload_variant {
                    return Err(CodegenError(
                        "a payload enum variant cannot be encoded as a unit constant".to_string(),
                    ));
                }
                ty.into_pointer_type().const_null().into()
            }
            EnumRepr::Tagged { variants, .. } => {
                let Some(representation) = variants.get(*variant as usize) else {
                    return Err(CodegenError(format!(
                        "enum unit constant has invalid variant {variant}"
                    )));
                };
                if !representation.fields.is_empty() {
                    return Err(CodegenError(
                        "a payload enum variant cannot be encoded as a unit constant".to_string(),
                    ));
                }
                let struct_type = ty.into_struct_type();
                let mut values = struct_type
                    .get_field_types()
                    .into_iter()
                    .map(BasicTypeEnum::const_zero)
                    .collect::<Vec<_>>();
                values[0] = context.i64_type().const_int(*variant as u64, false).into();
                struct_type.const_named_struct(&values).into()
            }
        },
        ConstantValue::Struct { struct_id, fields } => {
            let definition = &structs[*struct_id];
            if fields.len() != definition.fields.len() {
                return Err(CodegenError(format!(
                    "global constant for `{}` has the wrong field count",
                    definition.name
                )));
            }
            let struct_type = ty.into_struct_type();
            if definition.c_layout.is_none() {
                let values = fields
                    .iter()
                    .zip(&definition.fields)
                    .map(|(value, field)| {
                        let ty =
                            basic_ty(context, structs, enums, managed_address_space, &field.ty)?;
                        llvm_constant(
                            context,
                            structs,
                            enums,
                            globals,
                            managed_address_space,
                            ty,
                            value,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                struct_type.const_named_struct(&values).into()
            } else {
                let payload_types =
                    c_payload_fields(context, structs, enums, managed_address_space, definition)?;
                let mut payload_values = Vec::with_capacity(payload_types.len());
                let mut cursor = 0u64;
                for (field, value) in definition.fields.iter().zip(fields) {
                    if field.layout.offset > cursor {
                        let ty = payload_types[payload_values.len()];
                        payload_values.push(ty.const_zero());
                    }
                    let field_ty =
                        basic_ty(context, structs, enums, managed_address_space, &field.ty)?;
                    payload_values.push(llvm_constant(
                        context,
                        structs,
                        enums,
                        globals,
                        managed_address_space,
                        field_ty,
                        value,
                    )?);
                    cursor = field.layout.offset + c_field_size(structs, enums, &field.ty)?;
                }
                if payload_values.len() < payload_types.len() {
                    payload_values.push(payload_types[payload_values.len()].const_zero());
                }
                let payload_ty = context.struct_type(&payload_types, true);
                let payload = payload_ty.const_named_struct(&payload_values);
                let anchor = struct_type
                    .get_field_type_at_index(0)
                    .expect("C layout has an alignment anchor")
                    .const_zero();
                struct_type
                    .const_named_struct(&[anchor, payload.into()])
                    .into()
            }
        }
    })
}

pub(crate) fn alignment_anchor<'ctx>(
    context: &'ctx Context,
    align: u64,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    let element: BasicTypeEnum = match align {
        1 => context.i8_type().into(),
        2 => context.i16_type().into(),
        4 => context.i32_type().into(),
        8 => context.i64_type().into(),
        16 => context.i64_type().vec_type(2).into(),
        _ => {
            return Err(CodegenError(format!(
                "unsupported aggregate alignment {align}"
            )));
        }
    };
    Ok(element.array_type(0).into())
}

pub(crate) fn c_field_size(
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    ty: &LirType,
) -> Result<u64, CodegenError> {
    Ok(match ty {
        LirType::I1 => 1,
        LirType::I64 | LirType::Ptr(_) => 8,
        LirType::Struct(id) => structs[*id].size,
        LirType::Enum(id) if matches!(enums[*id].repr, EnumRepr::Niche { .. }) => 8,
        other => {
            return Err(CodegenError(format!(
                "non-C field type {} reached a C-layout struct",
                other.dump()
            )));
        }
    })
}

pub(crate) fn c_payload_fields<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    definition: &StructDef,
) -> Result<Vec<BasicTypeEnum<'ctx>>, CodegenError> {
    let mut physical = Vec::new();
    let mut cursor = 0u64;
    for field in &definition.fields {
        let padding = field.layout.offset.checked_sub(cursor).ok_or_else(|| {
            CodegenError(format!(
                "overlapping fields in C layout `{}`",
                definition.name
            ))
        })?;
        if padding != 0 {
            physical.push(context.i8_type().array_type(padding as u32).into());
        }
        physical.push(basic_ty(
            context,
            structs,
            enums,
            managed_address_space,
            &field.ty,
        )?);
        cursor = field.layout.offset + c_field_size(structs, enums, &field.ty)?;
    }
    let tail = definition
        .size
        .checked_sub(cursor)
        .ok_or_else(|| CodegenError(format!("fields exceed C layout `{}`", definition.name)))?;
    if tail != 0 {
        physical.push(context.i8_type().array_type(tail as u32).into());
    }
    Ok(physical)
}

/// LLVM field index of a logical C-layout field inside the packed
/// payload struct. Explicit padding arrays occupy physical fields but
/// are deliberately absent from LIR's source-level field numbering.
pub(crate) fn c_physical_field_index(
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    definition: &StructDef,
    logical_index: u32,
) -> Result<u32, CodegenError> {
    let mut physical_index = 0u32;
    let mut cursor = 0u64;
    for (index, field) in definition.fields.iter().enumerate() {
        if field.layout.offset > cursor {
            physical_index += 1;
        }
        if index == logical_index as usize {
            return Ok(physical_index);
        }
        physical_index += 1;
        cursor = field.layout.offset + c_field_size(structs, enums, &field.ty)?;
    }
    Err(CodegenError(format!(
        "field {logical_index} out of range for C layout `{}`",
        definition.name
    )))
}

pub(crate) fn struct_ty<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    id: scoop_lir::StructDefId,
) -> Result<StructType<'ctx>, CodegenError> {
    let definition = &structs[id];
    if definition.c_layout.is_none() {
        let fields = definition
            .fields
            .iter()
            .map(|field| basic_ty(context, structs, enums, managed_address_space, &field.ty))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(context.struct_type(&fields, false));
    }
    let anchor = alignment_anchor(context, definition.align)?;
    let payload = context.struct_type(
        &c_payload_fields(context, structs, enums, managed_address_space, definition)?,
        true,
    );
    Ok(context.struct_type(&[anchor, payload.into()], false))
}

/// Aggregate values cannot be returned directly from a statepoint call:
/// LLVM's statepoint rewrite lowers such results incompletely on the
/// supported native targets. Keep LIR's value-returning contract, but use
/// an explicit caller-provided result slot in the physical LLVM ABI.
pub(crate) fn uses_return_slot(enums: &Arena<EnumDef>, ty: &LirType) -> bool {
    match ty {
        LirType::Aggregate(_) | LirType::Struct(_) | LirType::ExceptionRecord => true,
        LirType::Enum(id) => matches!(enums[*id].repr, EnumRepr::Tagged { .. }),
        LirType::Void | LirType::I1 | LirType::I64 | LirType::Ptr(_) => false,
    }
}

/// Physical storage for a tagged enum. Non-reference payload remains opaque,
/// but every fixed GC slot is an AS1 pointer field so SROA cannot turn a
/// managed reference into integer/byte fragments.
pub(crate) fn tagged_ty<'ctx>(
    context: &'ctx Context,
    managed_address_space: ManagedAddressSpace,
    size: u64,
    align: u64,
    scan: &RefScan,
) -> Result<inkwell::types::StructType<'ctx>, CodegenError> {
    if size < 8 || align < 8 || !align.is_power_of_two() || size % align != 0 {
        return Err(CodegenError(format!(
            "invalid tagged enum size/alignment {size}/{align}"
        )));
    }
    let mut references = Vec::new();
    flatten_ref_scan(scan, &mut references);
    if references.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CodegenError(
            "tagged enum reference offsets are not strictly ordered".to_string(),
        ));
    }

    let mut fields: Vec<BasicTypeEnum> = vec![context.i64_type().into()];
    let mut cursor = 8u64;
    for offset in references {
        let end = offset.checked_add(8).ok_or_else(|| {
            CodegenError("tagged enum reference offset overflows u64".to_string())
        })?;
        if offset < cursor || offset % 8 != 0 || end > size {
            return Err(CodegenError(format!(
                "invalid tagged enum reference slot [{offset}, {end}) for size {size}"
            )));
        }
        push_byte_padding(context, &mut fields, offset - cursor)?;
        fields.push(managed_ptr_ty(context, managed_address_space).into());
        cursor = end;
    }
    push_byte_padding(context, &mut fields, size - cursor)?;
    if align > 8 {
        fields.push(alignment_anchor(context, align)?);
    }
    Ok(context.struct_type(&fields, false))
}

pub(crate) fn flatten_ref_scan(scan: &RefScan, offsets: &mut Vec<u64>) {
    match scan {
        RefScan::None => {}
        RefScan::References(references) => offsets.extend(references),
        RefScan::Sequence(parts) => {
            for part in parts {
                flatten_ref_scan(part, offsets);
            }
        }
    }
}

pub(crate) fn push_byte_padding<'ctx>(
    context: &'ctx Context,
    fields: &mut Vec<BasicTypeEnum<'ctx>>,
    bytes: u64,
) -> Result<(), CodegenError> {
    if bytes == 0 {
        return Ok(());
    }
    let bytes = u32::try_from(bytes)
        .map_err(|_| CodegenError("tagged enum padding exceeds u32::MAX".to_string()))?;
    fields.push(context.i8_type().array_type(bytes).into());
    Ok(())
}

pub(crate) fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

/// Whether any function needs the shared array-bounds trap message.
/// Array parameters can be indexed without any array being allocated
/// in this module, so this is intentionally independent of generated
/// array TypeDescriptors.
pub(crate) fn module_uses_bounds_checks(module: &Module) -> bool {
    module.functions.iter().any(|function| {
        function.blocks.iter().any(|(_, block)| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    Instruction::ArrayGet { .. } | Instruction::ArraySet { .. }
                )
            })
        })
    })
}

pub(crate) fn module_uses_array_assembly(module: &Module) -> bool {
    module.functions.iter().any(|function| {
        function.blocks.iter().any(|(_, block)| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::ArrayAssembly { .. }))
        })
    })
}

/// Native/code/metadata pointer type.
pub(crate) fn ptr_ty(context: &Context) -> inkwell::types::PointerType<'_> {
    context.ptr_type(AddressSpace::default())
}

/// Moving-GC object-start pointer type. Address space 1 is part of the fixed
/// LLVM 22.1 backend contract and is the only address space RS4GC traces.
pub(crate) fn managed_ptr_ty(
    context: &Context,
    managed_address_space: ManagedAddressSpace,
) -> inkwell::types::PointerType<'_> {
    context.ptr_type(managed_address_space.inkwell())
}

pub(crate) fn pointer_ty(
    context: &Context,
    managed_address_space: ManagedAddressSpace,
    kind: scoop_lir::PointerKind,
) -> inkwell::types::PointerType<'_> {
    match kind {
        scoop_lir::PointerKind::Managed => managed_ptr_ty(context, managed_address_space),
        scoop_lir::PointerKind::Raw
        | scoop_lir::PointerKind::Code
        | scoop_lir::PointerKind::Metadata => ptr_ty(context),
    }
}

/// Translate one LIR function. Signature (parameters and return type)
/// comes from LIR; parameters are SSA values (`Value::Param`).
pub(crate) fn fn_type_of<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    function: &Function,
) -> Result<inkwell::types::FunctionType<'ctx>, CodegenError> {
    let mut param_tys: Vec<BasicMetadataTypeEnum> = function
        .params
        .iter()
        .map(|ty| basic_ty(context, structs, enums, managed_address_space, ty).map(Into::into))
        .collect::<Result<_, _>>()?;
    if uses_return_slot(enums, &function.return_ty) {
        param_tys.insert(0, ptr_ty(context).into());
        return Ok(context.void_type().fn_type(&param_tys, false));
    }
    Ok(match &function.return_ty {
        LirType::Void => context.void_type().fn_type(&param_tys, false),
        return_ty => basic_ty(context, structs, enums, managed_address_space, return_ty)?
            .fn_type(&param_tys, false),
    })
}
