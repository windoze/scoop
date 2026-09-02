use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::TargetData;
use inkwell::values::{GlobalValue, StructValue};
use la_arena::Arena;
use scoop_lir::{Global, GlobalInit, PointerKind, RefScan};

use super::type_descriptors::emit_ref_scan;
use super::{CodegenError, ptr_ty};

const MANAGED_GLOBALS_SYMBOL: &str = "scoop_image_managed_globals";
const MANAGED_GLOBAL_COUNT_SYMBOL: &str = "scoop_image_managed_global_count";
const IMMORTAL_OBJECTS_SYMBOL: &str = "scoop_image_immortal_objects";
const IMMORTAL_OBJECT_COUNT_SYMBOL: &str = "scoop_image_immortal_object_count";

/// Emit the complete image-level root metadata consumed during runtime
/// initialization. Empty tables still contain one null sentinel so every
/// image defines an addressable symbol; the count is authoritative.
pub(super) fn emit<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    source_globals: &Arena<Global>,
    llvm_globals: &[Option<GlobalValue<'ctx>>],
    string_td: GlobalValue<'ctx>,
) -> Result<(), CodegenError> {
    if source_globals.len() != llvm_globals.len() {
        return Err(CodegenError(format!(
            "global emission is incomplete: {} LIR globals but {} LLVM globals",
            source_globals.len(),
            llvm_globals.len()
        )));
    }
    let ptr = ptr_ty(context);
    let i64_ty = context.i64_type();
    let managed_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
    let immortal_ty = context.struct_type(&[ptr.into(), i64_ty.into(), ptr.into()], false);
    let mut managed = Vec::new();
    let mut immortal = Vec::new();

    for ((_, source), emitted) in source_globals.iter().zip(llvm_globals) {
        let emitted = emitted.ok_or_else(|| {
            CodegenError(format!(
                "global `@{}` has no emitted LLVM storage",
                source.symbol
            ))
        })?;
        match &source.init {
            GlobalInit::StringConst(_) => {
                require_global_shape(source, PointerKind::Managed, true)?;
                let object_size = target_data.get_store_size(&emitted.get_value_type());
                immortal.push(context.const_struct(
                    &[
                        emitted.as_pointer_value().into(),
                        i64_ty.const_int(object_size, false).into(),
                        string_td.as_pointer_value().into(),
                    ],
                    false,
                ));
            }
            GlobalInit::CString(_) => {
                require_global_shape(source, PointerKind::Raw, true)?;
            }
            GlobalInit::Storage { thread_local, .. } => {
                require_global_shape(source, PointerKind::Raw, false)?;
                if matches!(source.scan, RefScan::None) {
                    continue;
                }
                if *thread_local {
                    return Err(CodegenError(format!(
                        "thread-local global `@{}` contains managed references; per-thread image roots are not supported",
                        source.symbol
                    )));
                }
                let scan = emit_ref_scan(
                    context,
                    llvm,
                    &format!("{}.global_refs", source.symbol),
                    &source.scan,
                )
                .ok_or_else(|| {
                    CodegenError(format!(
                        "global `@{}` carries a non-canonical empty RefScan",
                        source.symbol
                    ))
                })?;
                managed.push(
                    context.const_struct(&[emitted.as_pointer_value().into(), scan.into()], false),
                );
            }
        }
    }

    let managed_count = managed.len() as u64;
    let immortal_count = immortal.len() as u64;
    emit_table(
        llvm,
        MANAGED_GLOBALS_SYMBOL,
        managed_ty.const_zero(),
        managed,
    );
    emit_count(llvm, MANAGED_GLOBAL_COUNT_SYMBOL, managed_count);
    emit_table(
        llvm,
        IMMORTAL_OBJECTS_SYMBOL,
        immortal_ty.const_zero(),
        immortal,
    );
    emit_count(llvm, IMMORTAL_OBJECT_COUNT_SYMBOL, immortal_count);
    Ok(())
}

fn require_global_shape(
    global: &Global,
    address_kind: PointerKind,
    require_no_scan: bool,
) -> Result<(), CodegenError> {
    if global.address_kind != address_kind {
        return Err(CodegenError(format!(
            "global `@{}` has {:?} address provenance, expected {address_kind:?}",
            global.symbol, global.address_kind
        )));
    }
    if require_no_scan && !matches!(global.scan, RefScan::None) {
        return Err(CodegenError(format!(
            "non-storage global `@{}` must carry RefScan::None",
            global.symbol
        )));
    }
    Ok(())
}

fn emit_table<'ctx>(
    llvm: &LlvmModule<'ctx>,
    symbol: &str,
    sentinel: StructValue<'ctx>,
    mut records: Vec<StructValue<'ctx>>,
) {
    if records.is_empty() {
        records.push(sentinel);
    }
    let ty = records[0].get_type();
    let initializer = ty.const_array(&records);
    let global = llvm.add_global(initializer.get_type(), None, symbol);
    global.set_linkage(Linkage::External);
    global.set_constant(true);
    global.set_initializer(&initializer);
}

fn emit_count(llvm: &LlvmModule<'_>, symbol: &str, count: u64) {
    let ty = llvm.get_context().i64_type();
    let global = llvm.add_global(ty, None, symbol);
    global.set_linkage(Linkage::External);
    global.set_constant(true);
    global.set_initializer(&ty.const_int(count, false));
}
