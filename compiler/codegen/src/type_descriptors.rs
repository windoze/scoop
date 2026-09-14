use super::*;

/// Emit one recursive GC scan program. Child pointers are stored as
/// u64 constants because the C runtime descriptor is a word stream.
/// `None` has no global and is represented by a null pointer.
pub(super) fn emit_ref_scan<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    scan: &RefScan,
) -> Option<PointerValue<'ctx>> {
    let i64_ty = context.i64_type();
    let pointer_word = |pointer: Option<PointerValue<'ctx>>| {
        pointer.map_or_else(|| i64_ty.const_zero(), |value| value.const_to_int(i64_ty))
    };
    let words = match scan {
        RefScan::None => return None,
        RefScan::References(offsets) if offsets.is_empty() => return None,
        RefScan::References(offsets) => {
            let mut words = Vec::with_capacity(offsets.len() + 1);
            words.push(i64_ty.const_int(offsets.len() as u64, false));
            words.extend(
                offsets
                    .iter()
                    .map(|offset| i64_ty.const_int(*offset, false)),
            );
            words
        }
        RefScan::Sequence(parts) => {
            let children: Vec<_> = parts
                .iter()
                .enumerate()
                .filter_map(|(index, part)| {
                    emit_ref_scan(context, llvm, &format!("{name}.part.{index}"), part)
                })
                .collect();
            if children.is_empty() {
                return None;
            }
            let mut words = Vec::with_capacity(children.len() + 2);
            words.push(i64_ty.const_int(SCAN_SEQUENCE, false));
            words.push(i64_ty.const_int(children.len() as u64, false));
            words.extend(children.into_iter().map(|child| pointer_word(Some(child))));
            words
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let element = emit_ref_scan(
                context,
                llvm,
                &format!("{name}.element"),
                element.as_ref_scan(),
            )
            .expect("a NonEmptyRefScan always emits a physical scan program");
            vec![
                i64_ty.const_int(SCAN_ARRAY, false),
                i64_ty.const_int(*length_offset, false),
                i64_ty.const_int(*first_element_offset, false),
                i64_ty.const_int(stride.get(), false),
                element.const_to_int(i64_ty),
            ]
        }
    };
    let array = i64_ty.const_array(&words);
    Some(private_const_global(llvm, name, array.into()))
}

pub(super) fn type_descriptor_global<'ctx>(
    reference: TypeDescriptorRef,
    locals: &[GlobalValue<'ctx>],
    externals: &[GlobalValue<'ctx>],
) -> Result<GlobalValue<'ctx>, CodegenError> {
    match reference {
        TypeDescriptorRef::Local(id) => locals
            .get(arena_index(id))
            .copied()
            .ok_or_else(|| CodegenError(format!("invalid local TypeDescriptor id {id:?}"))),
        TypeDescriptorRef::CoreExternal(id) => externals
            .get(arena_index(id))
            .copied()
            .ok_or_else(|| CodegenError(format!("invalid external TypeDescriptor id {id:?}"))),
    }
}

/// Emit every local `ScoopTypeDescriptor` from its complete typed graph.
/// Runs after function declaration so typed dispatch entries resolve to
/// already-declared functions.
pub(super) fn emit_type_descriptors<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    type_globals: &[GlobalValue<'ctx>],
    external_type_globals: &[GlobalValue<'ctx>],
    module: &Module,
) -> Result<(), CodegenError> {
    let types = runtime_metadata_v1::RuntimeMetadataV1Types::new(context);
    // ScoopItableEntryV1: { ptr interface, ptr slots }.
    let emission = TypeDescriptorEmission {
        context,
        llvm,
        entry_ty: types.itable_entry(),
        type_instance_shape_ty: types.type_instance_shape(),
        byte_span_ty: types.byte_span(),
        type_globals,
        external_type_globals,
        module,
    };
    for ((_, td), global) in module.meta.type_descriptors.iter().zip(type_globals) {
        emit_type_descriptor(&emission, *global, td)?;
    }
    Ok(())
}

struct TypeDescriptorEmission<'a, 'ctx> {
    context: &'ctx Context,
    llvm: &'a LlvmModule<'ctx>,
    entry_ty: StructType<'ctx>,
    type_instance_shape_ty: StructType<'ctx>,
    byte_span_ty: StructType<'ctx>,
    type_globals: &'a [GlobalValue<'ctx>],
    external_type_globals: &'a [GlobalValue<'ctx>],
    module: &'a Module,
}

fn emit_type_descriptor<'ctx>(
    emission: &TypeDescriptorEmission<'_, 'ctx>,
    global: GlobalValue<'ctx>,
    descriptor: &TypeDescriptor,
) -> Result<(), CodegenError> {
    let context = emission.context;
    let llvm = emission.llvm;
    let entry_ty = emission.entry_ty;
    let type_globals = emission.type_globals;
    let external_type_globals = emission.external_type_globals;
    let module = emission.module;
    let i32_ty = context.i32_type();
    let i64_ty = context.i64_type();
    let ptr = ptr_ty(context);
    let object_scan: BasicValueEnum = emit_ref_scan(
        context,
        llvm,
        &format!("{}.object_scan", descriptor.identity.symbol()),
        descriptor.instance_shape.object_scan(),
    )
    .map_or_else(|| ptr.const_null().into(), Into::into);
    let inline_scan: BasicValueEnum = emit_ref_scan(
        context,
        llvm,
        &format!("{}.inline_scan", descriptor.identity.symbol()),
        descriptor.instance_shape.inline_scan(),
    )
    .map_or_else(|| ptr.const_null().into(), Into::into);
    let parent: BasicValueEnum = match descriptor.parent {
        Some(reference) => type_descriptor_global(reference, type_globals, external_type_globals)?
            .as_pointer_value()
            .into(),
        None => ptr.const_null().into(),
    };
    let vtable = emit_fn_table(
        context,
        llvm,
        &format!("{}.vtable", descriptor.identity.symbol()),
        descriptor.vtable.slots(),
        &module.functions,
        &module.meta.core_external_callables,
    )?;
    let (itables, itable_count): (BasicValueEnum, u64) = if descriptor.itables.is_empty() {
        (ptr.const_null().into(), 0)
    } else {
        let mut entries = Vec::with_capacity(descriptor.itables.len());
        for (record_index, record) in descriptor.itables.iter().enumerate() {
            let interface =
                type_descriptor_global(record.interface(), type_globals, external_type_globals)?
                    .as_pointer_value();
            let slots = emit_fn_table(
                context,
                llvm,
                &format!("{}.itables.{record_index}", descriptor.identity.symbol()),
                record.slots(),
                &module.functions,
                &module.meta.core_external_callables,
            )?;
            entries.push(context.const_struct(&[interface.into(), slots], false));
        }
        let array = entry_ty.const_array(&entries);
        let itable_global = private_const_global(
            llvm,
            &format!("{}.itables", descriptor.identity.symbol()),
            array.into(),
        );
        (itable_global.into(), descriptor.itables.len() as u64)
    };
    let name = private_bytes(
        context,
        llvm,
        &format!("{}.name", descriptor.identity.symbol()),
        descriptor.diagnostic_name.as_bytes(),
    );
    let shape = &descriptor.instance_shape;
    let instance_shape = emission.type_instance_shape_ty.const_named_struct(&[
        i32_ty
            .const_int(u64::from(shape.instance_kind().tag()), false)
            .into(),
        i32_ty
            .const_int(u64::from(shape.inline_storage_kind().tag()), false)
            .into(),
        i64_ty.const_int(shape.minimum_size(), false).into(),
        i64_ty.const_int(shape.instance_alignment(), false).into(),
        i64_ty.const_int(shape.inline_offset(), false).into(),
        i64_ty.const_int(shape.inline_size(), false).into(),
        i64_ty.const_int(shape.inline_stride(), false).into(),
        i64_ty.const_int(shape.inline_alignment(), false).into(),
        inline_scan,
    ]);
    let diagnostic_name = emission.byte_span_ty.const_named_struct(&[
        name.into(),
        i64_ty
            .const_int(descriptor.diagnostic_name.len() as u64, false)
            .into(),
    ]);
    global.set_constant(true);
    global.set_initializer(
        &context.const_struct(
            &[
                i64_ty
                    .const_int(
                        descriptor.identity.runtime_type().runtime_type().get(),
                        false,
                    )
                    .into(),
                instance_shape.into(),
                object_scan,
                parent,
                vtable,
                itables,
                i64_ty.const_int(itable_count, false).into(),
                diagnostic_name.into(),
            ],
            false,
        ),
    );
    Ok(())
}

/// A private constant global holding `value`; returns its address.
pub(super) fn private_const_global<'ctx>(
    llvm: &LlvmModule<'ctx>,
    name: &str,
    value: BasicValueEnum<'ctx>,
) -> PointerValue<'ctx> {
    let global = llvm.add_global(value.get_type(), None, name);
    global.set_constant(true);
    global.set_linkage(inkwell::module::Linkage::Private);
    global.set_initializer(&value);
    global.as_pointer_value()
}

/// A private exact byte sequence whose address is stable for the lifetime of
/// the generated module.
fn private_bytes<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    value: &[u8],
) -> PointerValue<'ctx> {
    private_const_global(llvm, name, context.const_string(value, false).into())
}

/// A global `[N x ptr]` of function addresses (a vtable or one itable's
/// slots), or null when the table is empty.
fn emit_fn_table<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    slots: &[DispatchEntry],
    functions: &[Function],
    external_callables: &Arena<scoop_lir::CoreExternalCallable>,
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    let ptr = ptr_ty(context);
    if slots.is_empty() {
        return Ok(ptr.const_null().into());
    }
    let mut values = Vec::with_capacity(slots.len());
    for entry in slots {
        values.push(slot_fn_ptr(
            llvm,
            entry.callable,
            functions,
            external_callables,
        )?);
    }
    let array = ptr.const_array(&values);
    Ok(private_const_global(llvm, name, array.into()).into())
}

/// Address of the exact module function named by a vtable / itable slot.
/// All dispatch entries are declared in the first pass; codegen never guesses
/// a missing function's signature from its symbol.
fn slot_fn_ptr<'ctx>(
    llvm: &LlvmModule<'ctx>,
    callable: CallableRef,
    functions: &[Function],
    external_callables: &Arena<scoop_lir::CoreExternalCallable>,
) -> Result<PointerValue<'ctx>, CodegenError> {
    let symbol = match callable {
        CallableRef::Local(id) => functions
            .get(id.into_u32() as usize)
            .ok_or_else(|| CodegenError(format!("invalid local callable id {id:?}")))?
            .symbol(),
        CallableRef::Runtime(function) => function.symbol(),
        CallableRef::CoreExternal(id) => {
            let symbol = external_callables[id].expected_symbol().symbol();
            return llvm
                .get_function(symbol.as_str())
                .map(|function| function.as_global_value().as_pointer_value())
                .ok_or_else(|| {
                    CodegenError(format!(
                        "typed dispatch callable {callable:?} (`@{symbol}`) is not declared"
                    ))
                });
        }
    };
    llvm.get_function(symbol)
        .map(|function| function.as_global_value().as_pointer_value())
        .ok_or_else(|| {
            CodegenError(format!(
                "typed dispatch callable {callable:?} (`@{symbol}`) is not declared"
            ))
        })
}
