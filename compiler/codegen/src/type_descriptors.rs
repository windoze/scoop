use super::*;
use crate::shape_definitions::{
    EmittedStrongShapeDefinitionsV1, descriptor_definition, descriptor_diagnostic_atom,
    descriptor_itable_directory_atom, emit_dispatch_definition_v1,
};

#[derive(Clone, Copy)]
pub(super) struct TypeDescriptorGlobals<'a, 'ctx> {
    pub(super) local: &'a [GlobalValue<'ctx>],
    pub(super) core_external: &'a [GlobalValue<'ctx>],
    pub(super) dependency_external: &'a [GlobalValue<'ctx>],
}

pub(super) fn type_descriptor_global<'ctx>(
    reference: TypeDescriptorRef,
    globals: TypeDescriptorGlobals<'_, 'ctx>,
) -> Result<GlobalValue<'ctx>, CodegenError> {
    match reference {
        TypeDescriptorRef::Local(id) => globals
            .local
            .get(arena_index(id))
            .copied()
            .ok_or_else(|| CodegenError(format!("invalid local TypeDescriptor id {id:?}"))),
        TypeDescriptorRef::CoreExternal(id) => globals
            .core_external
            .get(arena_index(id))
            .copied()
            .ok_or_else(|| CodegenError(format!("invalid core TypeDescriptor id {id:?}"))),
        TypeDescriptorRef::DependencyExternal(id) => globals
            .dependency_external
            .get(arena_index(id))
            .copied()
            .ok_or_else(|| CodegenError(format!("invalid dependency TypeDescriptor id {id:?}"))),
    }
}

/// Emit every local `ScoopTypeDescriptor` from its complete typed graph.
/// Runs after function declaration so typed dispatch entries resolve to
/// already-declared functions.
pub(super) fn emit_strong_type_descriptors_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    surface: &scoop_lir::StrongObjectSymbolSurfaceV1,
    type_globals: TypeDescriptorGlobals<'_, 'ctx>,
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
        module,
        surface,
    };
    for ((_, td), global) in module.meta.type_descriptors.iter().zip(type_globals.local) {
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
    type_globals: TypeDescriptorGlobals<'a, 'ctx>,
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
    let inline_scan: BasicValueEnum = match descriptor.inline_scan {
        scoop_lir::TypeDescriptorInlineScanV1::Null => {
            if descriptor.instance_shape.inline_scan().contains_reference() {
                return Err(CodegenError(format!(
                    "TypeDescriptor {} has a nonempty inline scan without its typed definition",
                    descriptor.identity.exact_type()
                )));
            }
            ptr.const_null().into()
        }
        scoop_lir::TypeDescriptorInlineScanV1::Defined(scan) => shapes
            .scan(scan)?
            .runtime_pointer()
            .ok_or_else(|| {
                CodegenError(format!(
                    "TypeDescriptor {} references empty typed inline scan {scan}",
                    descriptor.identity.exact_type()
                ))
            })?
            .into(),
    };
    let parent: BasicValueEnum = match descriptor.parent {
        Some(reference) => type_descriptor_global(reference, type_globals)?
            .as_pointer_value()
            .into(),
        None => ptr.const_null().into(),
    };
    let vtable_values = dispatch_values(
        llvm,
        descriptor.vtable.slots(),
        &module.functions,
        &module.meta.core_external_callables,
        &module.meta.dependency_external_callables,
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
                type_descriptor_global(record.interface(), type_globals)?.as_pointer_value();
            let values = dispatch_values(
                llvm,
                record.slots(),
                &module.functions,
                &module.meta.core_external_callables,
                &module.meta.dependency_external_callables,
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
        let directory_name = format!("{}.itables", descriptor.identity.symbol());
        if llvm.get_global(&directory_name).is_some()
            || llvm.get_function(&directory_name).is_some()
        {
            return Err(CodegenError(format!(
                "TypeDescriptor itable directory `{directory_name}` collides with an LLVM value"
            )));
        }
        let itable_global = llvm.add_global(array.get_type(), None, &directory_name);
        itable_global.set_linkage(inkwell::module::Linkage::Private);
        itable_global.set_constant(true);
        itable_global.set_initializer(&array);
        shapes.record_atom(
            descriptor_itable_directory_atom(emission.surface, descriptor.identity.exact_type())?,
            itable_global,
        );
        (
            itable_global.as_pointer_value().into(),
            descriptor.itables.len() as u64,
        )
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
    core_external_callables: &Arena<scoop_lir::CoreExternalCallable>,
    dependency_external_callables: &Arena<scoop_lir::DependencyExternalCallable>,
) -> Result<Vec<PointerValue<'ctx>>, CodegenError> {
    let mut values = Vec::with_capacity(slots.len());
    for entry in slots {
        values.push(slot_fn_ptr(
            llvm,
            entry.callable,
            functions,
            core_external_callables,
            dependency_external_callables,
        )?);
    }
    Ok(values)
}

/// Address of the exact module function named by a vtable / itable slot.
/// All dispatch entries are declared in the first pass; codegen never guesses
/// a missing function's signature from its symbol.
fn slot_fn_ptr<'ctx>(
    llvm: &LlvmModule<'ctx>,
    callable: CallableRef,
    functions: &[Function],
    core_external_callables: &Arena<scoop_lir::CoreExternalCallable>,
    dependency_external_callables: &Arena<scoop_lir::DependencyExternalCallable>,
) -> Result<PointerValue<'ctx>, CodegenError> {
    let symbol = match callable {
        CallableRef::Local(id) => functions
            .get(id.into_u32() as usize)
            .ok_or_else(|| CodegenError(format!("invalid local callable id {id:?}")))?
            .symbol(),
        CallableRef::Runtime(function) => function.symbol(),
        CallableRef::CoreExternal(id) => {
            let symbol = core_external_callables[id].expected_symbol().symbol();
            return llvm
                .get_function(symbol.as_str())
                .map(|function| function.as_global_value().as_pointer_value())
                .ok_or_else(|| {
                    CodegenError(format!(
                        "typed dispatch callable {callable:?} (`@{symbol}`) is not declared"
                    ))
                });
        }
        CallableRef::DependencyExternal(id) => {
            let symbol = dependency_external_callables[id].expected_symbol().symbol();
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
