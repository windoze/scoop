use super::*;
use crate::shape_definitions::{
    EmittedStrongShapeDefinitionsV1, descriptor_definition, descriptor_diagnostic_atom,
    emit_dispatch_definition_v1,
};

/// Emit one function-local recursive GC scan program. Persistent layout
/// scans use `shape_definitions`; this helper is reserved for ephemeral root
/// descriptors attached to generated call frames.
pub(super) fn emit_ref_scan<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    scan: &RefScan,
) -> Option<PointerValue<'ctx>> {
    let i64 = context.i64_type();
    let words = match scan {
        RefScan::None => return None,
        RefScan::References(offsets) if offsets.is_empty() => return None,
        RefScan::References(offsets) => std::iter::once(i64.const_int(offsets.len() as u64, false))
            .chain(offsets.iter().map(|offset| i64.const_int(*offset, false)))
            .collect(),
        RefScan::Sequence(parts) => {
            let children = parts
                .iter()
                .enumerate()
                .filter_map(|(index, part)| {
                    emit_ref_scan(context, llvm, &format!("{name}.part.{index}"), part)
                })
                .collect::<Vec<_>>();
            if children.is_empty() {
                return None;
            }
            std::iter::once(i64.const_int(SCAN_SEQUENCE, false))
                .chain(std::iter::once(i64.const_int(children.len() as u64, false)))
                .chain(children.into_iter().map(|child| child.const_to_int(i64)))
                .collect()
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let child = emit_ref_scan(
                context,
                llvm,
                &format!("{name}.element"),
                element.as_ref_scan(),
            )
            .expect("a NonEmptyRefScan always emits a physical scan program");
            vec![
                i64.const_int(SCAN_ARRAY, false),
                i64.const_int(*length_offset, false),
                i64.const_int(*first_element_offset, false),
                i64.const_int(stride.get(), false),
                child.const_to_int(i64),
            ]
        }
    };
    let value = i64.const_array(&words);
    Some(private_const_global(llvm, name, value.into()))
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
pub(super) fn emit_strong_type_descriptors_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    surface: &scoop_lir::StrongObjectSymbolSurfaceV1,
    type_globals: &[GlobalValue<'ctx>],
    external_type_globals: &[GlobalValue<'ctx>],
    module: &Module,
    shapes: &mut EmittedStrongShapeDefinitionsV1<'ctx>,
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
        surface,
    };
    for ((_, td), global) in module.meta.type_descriptors.iter().zip(type_globals) {
        emit_type_descriptor(&emission, *global, td, shapes)?;
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
    surface: &'a scoop_lir::StrongObjectSymbolSurfaceV1,
}

fn emit_type_descriptor<'ctx>(
    emission: &TypeDescriptorEmission<'_, 'ctx>,
    global: GlobalValue<'ctx>,
    descriptor: &TypeDescriptor,
    shapes: &mut EmittedStrongShapeDefinitionsV1<'ctx>,
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
    let definition = descriptor_definition(emission.surface, descriptor.identity.exact_type())?;
    if global.get_name().to_bytes() != definition.primary_symbol().symbol().as_str().as_bytes()
        || descriptor.identity.symbol_request() != definition.primary_symbol()
    {
        return Err(CodegenError(format!(
            "TypeDescriptor {} declaration diverges from canonical strong symbol `{}`",
            descriptor.identity.exact_type(),
            definition.primary_symbol().symbol()
        )));
    }
    let object_scan: BasicValueEnum = shapes
        .scan(descriptor.instance_layout.scan_record().id())?
        .runtime_pointer()
        .map_or_else(|| ptr.const_null().into(), Into::into);
    let inline_scan: BasicValueEnum = inline_scan_id(module, descriptor)?
        .map(|scan| shapes.scan(scan))
        .transpose()?
        .and_then(|scan| scan.runtime_pointer())
        .map_or_else(|| ptr.const_null().into(), Into::into);
    let parent: BasicValueEnum = match descriptor.parent {
        Some(reference) => type_descriptor_global(reference, type_globals, external_type_globals)?
            .as_pointer_value()
            .into(),
        None => ptr.const_null().into(),
    };
    let vtable_values = dispatch_values(
        llvm,
        descriptor.vtable.slots(),
        &module.functions,
        &module.meta.core_external_callables,
    )?;
    let vtable = emit_dispatch_definition_v1(
        context,
        llvm,
        emission.surface,
        descriptor.vtable.identity_record().id(),
        &vtable_values,
        shapes,
    )?
    .runtime_pointer()
    .map_or_else(|| ptr.const_null().into(), Into::into);
    let (itables, itable_count): (BasicValueEnum, u64) = if descriptor.itables.is_empty() {
        (ptr.const_null().into(), 0)
    } else {
        let mut entries = Vec::with_capacity(descriptor.itables.len());
        for record in &descriptor.itables {
            let interface =
                type_descriptor_global(record.interface(), type_globals, external_type_globals)?
                    .as_pointer_value();
            let values = dispatch_values(
                llvm,
                record.slots(),
                &module.functions,
                &module.meta.core_external_callables,
            )?;
            let slots = emit_dispatch_definition_v1(
                context,
                llvm,
                emission.surface,
                record.identity_record().id(),
                &values,
                shapes,
            )?
            .runtime_pointer()
            .unwrap_or_else(|| ptr.const_null());
            entries.push(context.const_struct(&[interface.into(), slots.into()], false));
        }
        let array = entry_ty.const_array(&entries);
        let itable_global = private_const_global(
            llvm,
            &format!("{}.itables", descriptor.identity.symbol()),
            array.into(),
        );
        (itable_global.into(), descriptor.itables.len() as u64)
    };
    let diagnostic_name = format!("{}.diagnostic", descriptor.identity.symbol());
    if llvm.get_global(&diagnostic_name).is_some() || llvm.get_function(&diagnostic_name).is_some()
    {
        return Err(CodegenError(format!(
            "TypeDescriptor diagnostic `{diagnostic_name}` collides with an LLVM value"
        )));
    }
    let name_value = context.const_string(descriptor.diagnostic_name.as_bytes(), false);
    let name_global = llvm.add_global(name_value.get_type(), None, &diagnostic_name);
    name_global.set_linkage(inkwell::module::Linkage::Private);
    name_global.set_constant(true);
    name_global.set_initializer(&name_value);
    let name = name_global.as_pointer_value();
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
    shapes.record_atom(definition.primary_atom(), global);
    shapes.record_atom(
        descriptor_diagnostic_atom(emission.surface, descriptor.identity.exact_type())?,
        name_global,
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

fn dispatch_values<'ctx>(
    llvm: &LlvmModule<'ctx>,
    slots: &[DispatchEntry],
    functions: &[Function],
    external_callables: &Arena<scoop_lir::CoreExternalCallable>,
) -> Result<Vec<PointerValue<'ctx>>, CodegenError> {
    let mut values = Vec::with_capacity(slots.len());
    for entry in slots {
        values.push(slot_fn_ptr(
            llvm,
            entry.callable,
            functions,
            external_callables,
        )?);
    }
    Ok(values)
}

fn inline_scan_id(
    module: &Module,
    descriptor: &TypeDescriptor,
) -> Result<Option<scoop_lir::PersistentScanId>, CodegenError> {
    use scoop_lir::TypeInstanceKindV1;

    if !descriptor.instance_shape.inline_scan().contains_reference() {
        return Ok(None);
    }
    let exact = descriptor.identity.exact_type();
    let mut candidates = match descriptor.instance_shape.instance_kind() {
        TypeInstanceKindV1::InlineArray => module
            .meta
            .arrays
            .iter()
            .filter(|(_, array)| {
                array.identity.layout_record().key().exact_type() == exact
                    && &array.element_scan == descriptor.instance_shape.inline_scan()
            })
            .map(|(_, array)| array.identity.scan_record().id())
            .collect::<Vec<_>>(),
        TypeInstanceKindV1::BoxedValue => module
            .meta
            .layouts
            .iter()
            .filter(|(_, layout)| {
                layout.identity.layout_record().key().exact_type() == exact
                    && layout
                        .identity
                        .is_managed_value_of(exact, module.meta.target_profile)
                    && match &layout.kind {
                        scoop_lir::LayoutKind::Plain { scan }
                        | scoop_lir::LayoutKind::Enum { scan } => {
                            scan == descriptor.instance_shape.inline_scan()
                        }
                        scoop_lir::LayoutKind::Intrinsic(_) => false,
                    }
            })
            .map(|(_, layout)| layout.identity.scan_record().id())
            .collect::<Vec<_>>(),
        TypeInstanceKindV1::FixedObject
        | TypeInstanceKindV1::InlineBytes
        | TypeInstanceKindV1::AbstractRef => Vec::new(),
    };
    candidates.sort_unstable();
    candidates.dedup();
    match candidates.as_slice() {
        [scan] => Ok(Some(*scan)),
        [] => Err(CodegenError(format!(
            "TypeDescriptor {exact} has a nonempty inline scan without one typed scan definition"
        ))),
        _ => Err(CodegenError(format!(
            "TypeDescriptor {exact} has multiple typed inline scan definitions"
        ))),
    }
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
