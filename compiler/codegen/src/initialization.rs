use std::collections::HashSet;

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::{GlobalValue, StructValue};

use super::{CodegenError, arena_index, ptr_ty};

const UNITS_SYMBOL: &str = "scoop_image_initialization_units";
const UNIT_COUNT_SYMBOL: &str = "scoop_image_initialization_unit_count";

/// Emit one address-stable cell/descriptor per typed unit and a second table
/// sorted by stable key for deterministic eager startup. Generated ensure
/// functions use the individual descriptor addresses; both views point at the
/// same cell and managed root slots.
pub(super) fn emit<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    module: &scoop_lir::Module,
    globals: &[Option<GlobalValue<'ctx>>],
) -> Result<Vec<GlobalValue<'ctx>>, CodegenError> {
    let ptr = ptr_ty(context);
    let i64_ty = context.i64_type();
    let cell_ty = context.struct_type(&[i64_ty.into(), ptr.into()], false);
    let descriptor_ty = context.struct_type(
        &[
            i64_ty.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
        ],
        false,
    );
    let mut keys = HashSet::new();
    let mut descriptors = Vec::with_capacity(module.initialization_units.len());
    let mut records = Vec::with_capacity(module.initialization_units.len());

    for (id, unit) in module.initialization_units.iter() {
        if !keys.insert(unit.stable_key.as_str()) {
            return Err(CodegenError(format!(
                "duplicate initialization stable key `{}`",
                unit.stable_key
            )));
        }
        let raw = id.into_raw().into_u32();
        let key_bytes = unit.stable_key.as_bytes();
        let key_global = llvm.add_global(
            context.i8_type().array_type(key_bytes.len() as u32 + 1),
            None,
            &format!("scoop.init.key.{raw}"),
        );
        key_global.set_linkage(Linkage::Private);
        key_global.set_constant(true);
        key_global.set_initializer(&context.const_string(key_bytes, true));
        let display_bytes = unit.display_name.as_bytes();
        let display_global = llvm.add_global(
            context.i8_type().array_type(display_bytes.len() as u32 + 1),
            None,
            &format!("scoop.init.display.{raw}"),
        );
        display_global.set_linkage(Linkage::Private);
        display_global.set_constant(true);
        display_global.set_initializer(&context.const_string(display_bytes, true));

        let cell = llvm.add_global(cell_ty, None, &format!("scoop.init.cell.{raw}"));
        cell.set_linkage(Linkage::Private);
        cell.set_initializer(&cell_ty.const_zero());

        let storage = emitted_global(globals, unit.kind.storage())?;
        let failure = emitted_global(globals, unit.failure_root)?;
        let initializer = function_pointer(llvm, module, unit.initializer)?;
        let ensure = function_pointer(llvm, module, unit.ensure)?;
        let schedule = match unit.schedule {
            scoop_lir::InitializationSchedule::EagerStartup => 0,
            scoop_lir::InitializationSchedule::LazyAccess => 1,
        };
        let record = context.const_struct(
            &[
                i64_ty.const_int(schedule, false).into(),
                key_global.as_pointer_value().into(),
                display_global.as_pointer_value().into(),
                cell.as_pointer_value().into(),
                storage.as_pointer_value().into(),
                failure.as_pointer_value().into(),
                initializer.into(),
                ensure.into(),
            ],
            false,
        );
        let descriptor =
            llvm.add_global(descriptor_ty, None, &format!("scoop.init.descriptor.{raw}"));
        descriptor.set_linkage(Linkage::Private);
        descriptor.set_constant(true);
        descriptor.set_initializer(&record);
        descriptors.push(descriptor);
        records.push((unit.stable_key.as_str(), record));
    }

    records.sort_by(|left, right| left.0.cmp(right.0));
    let count = records.len() as u64;
    let table_values = if records.is_empty() {
        vec![descriptor_ty.const_zero()]
    } else {
        records.into_iter().map(|(_, record)| record).collect()
    };
    emit_table(llvm, descriptor_ty, table_values);
    let count_global = llvm.add_global(i64_ty, None, UNIT_COUNT_SYMBOL);
    count_global.set_linkage(Linkage::External);
    count_global.set_constant(true);
    count_global.set_initializer(&i64_ty.const_int(count, false));
    Ok(descriptors)
}

fn emitted_global<'ctx>(
    globals: &[Option<GlobalValue<'ctx>>],
    id: scoop_lir::GlobalId,
) -> Result<GlobalValue<'ctx>, CodegenError> {
    globals
        .get(arena_index(id))
        .copied()
        .flatten()
        .ok_or_else(|| {
            CodegenError(format!(
                "initialization descriptor references unavailable global {}",
                id.into_raw().into_u32()
            ))
        })
}

fn function_pointer<'ctx>(
    llvm: &LlvmModule<'ctx>,
    module: &scoop_lir::Module,
    function: scoop_lir::ManagedLocalFunctionRef,
) -> Result<inkwell::values::PointerValue<'ctx>, CodegenError> {
    let index = function.declaration().into_u32() as usize;
    let declaration = module.functions.get(index).ok_or_else(|| {
        CodegenError(format!(
            "initialization descriptor references unavailable function {index}"
        ))
    })?;
    llvm.get_function(declaration.symbol())
        .map(|function| function.as_global_value().as_pointer_value())
        .ok_or_else(|| {
            CodegenError(format!(
                "initialization function `{}` was not declared",
                declaration.symbol()
            ))
        })
}

fn emit_table<'ctx>(
    llvm: &LlvmModule<'ctx>,
    descriptor_ty: inkwell::types::StructType<'ctx>,
    records: Vec<StructValue<'ctx>>,
) {
    let initializer = descriptor_ty.const_array(&records);
    let table = llvm.add_global(initializer.get_type(), None, UNITS_SYMBOL);
    table.set_linkage(Linkage::External);
    table.set_constant(true);
    table.set_initializer(&initializer);
}
