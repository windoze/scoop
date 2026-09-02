//! Codegen stage: mechanically translate LIR to LLVM IR, emit
//! TypeDescriptors, expand codegen-stage intrinsics, produce `.o`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.5 and
//! `docs/milestone2/DESIGN.md` section 2.5.
//!
//! All locals become `alloca`s at the top of the entry block; SSA
//! construction is left to LLVM's mem2reg. Temps are SSA values kept in a
//! map. Function and call-target signatures come from typed LIR; codegen
//! never reconstructs a callee ABI from operands, result temporaries, or a
//! symbol name.
//!
//! Every heap object carries the 16-byte header `{ td, gc_word }`; class
//! fields start at byte 16, boxed payload and array size live at 16, and
//! array elements start at `align_up(24, element_align)`. M13 allocation
//! sites inline the per-thread TLAB bump and use a GC-leaf finish helper;
//! only the slow path is a safepoint.
//!
//! M15 LIR already contains every entry/back-edge poll, image-unique
//! `SafepointId`, and protocol-specific root plan. Managed pointers become
//! LLVM AS1 while raw/code/metadata pointers stay AS0. Ordinary calls and
//! polls go through RS4GC after SROA; managed invokes use explicit zero-live
//! statepoints plus compiler-root frames because LLVM cannot relocate the
//! exceptional edge. Codegen consumes these plans mechanically and never
//! rediscovers CFG liveness. Every `HeapStore` / `ArraySet` still emits the
//! monotonic card mark reserved for a future generational collector.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::targets::{FileType, TargetMachine};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{
    BasicValue, BasicValueEnum, GlobalValue, InstructionValue, IntValue, PointerValue, ValueKind,
};
use inkwell::{AddressSpace, AtomicOrdering, AtomicRMWBinOp, IntPredicate};
use la_arena::{Arena, Idx};
use scoop_lir::{
    ArrayType, ArrayTypeId, BinOp, CallableRef, ConstantValue, DispatchEntry, EnumDef, EnumRepr,
    ExternFunctionKind, ExternFunctions, Function, Global, GlobalInit, Instruction, LirType,
    Module, NativeGlobal, RefScan, StructDef, TempId, Terminator, TypeDescriptor,
    TypeDescriptorRef, TypeDescriptorScan, UnOp, Value,
};

const SCAN_ARRAY: u64 = u64::MAX;
const SCAN_SEQUENCE: u64 = u64::MAX - 1;

/// The write barrier's card table (M9, runtime spec 3.6): the runtime
/// exports `extern unsigned char *scoop_gc_card_table` — a pointer
/// variable pre-biased with the arena base, loaded at every marking
/// site. The v1 collector ignores the table; the remembered-set
/// consumer arrives with generations.
const CARD_TABLE_SYMBOL: &str = "scoop_gc_card_table";

/// Card granularity of the write barrier: one card per 512 bytes.
const CARD_SHIFT: u64 = 9;

/// Error produced while translating LIR or emitting the object file.
#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodegenError {}

mod c_bridge;
mod function;
mod image_roots;
mod statepoint;
mod target;

pub use c_bridge::{c_bridge_source, c_layout_assertions};
use function::emit_function;
use target::ManagedAddressSpace;
pub use target::{LlvmVersion, TargetProfile, TargetProfileId, linked_llvm_version};

fn align_up(value: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

fn array_data_offset(element_align: u64) -> u64 {
    align_up(24, element_align)
}

fn mark_typed_managed_pointer_boundary(
    context: &Context,
    instruction: InstructionValue<'_>,
    boundary: statepoint::TypedManagedPointerBoundary,
) -> Result<(), CodegenError> {
    instruction
        .set_metadata(
            context.metadata_node(&[context.metadata_string(boundary.name()).into()]),
            context.get_kind_id(statepoint::TYPED_MANAGED_POINTER_BOUNDARY_METADATA),
        )
        .map_err(|error| CodegenError(format!("mark typed managed pointer boundary: {error}")))
}

/// Translate `module` to LLVM IR and emit an object file at `output` using the
/// complete target profile selected by the driver.
pub fn emit_object(
    module: &Module,
    output: &Path,
    profile: TargetProfile,
) -> Result<(), CodegenError> {
    let expected_safepoints = statepoint::expectations(module)?;
    let machine = profile.create_target_machine()?;
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, profile)?;

    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid LLVM module: {e}")))?;

    // M9 (milestone9 DESIGN 3.1, M0 spike): rewrite every call and
    // invoke in the GC-strategy functions into a `gc.statepoint`; the
    // object file's `__llvm_stackmaps` section is produced from them.
    // Runs after verification, right before object emission.
    statepoint::rewrite(&llvm, &machine)?;
    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid post-RS4GC LLVM module: {e}")))?;
    statepoint::verify_rewritten(&llvm, &expected_safepoints, profile)?;

    machine
        .write_to_file(&llvm, FileType::Object, output)
        .map_err(|e| CodegenError(format!("failed to write {}: {e}", output.display())))?;
    if let Err(error) = profile.verify_object(output, &expected_safepoints) {
        if let Err(remove_error) = std::fs::remove_file(output) {
            return Err(CodegenError(format!(
                "{error}; also failed to discard invalid object {}: {remove_error}",
                output.display()
            )));
        }
        return Err(error);
    }
    Ok(())
}

/// Test helper for constructing the one supported host profile. Production
/// code receives a profile selected by the driver.
#[cfg(test)]
fn host_target_machine() -> Result<TargetMachine, CodegenError> {
    TargetProfile::resolve_host()?.create_target_machine()
}

/// Translate `module` to an (unverified) LLVM module: globals,
/// TypeDescriptors, and every function.
fn emit_llvm_module<'ctx>(
    context: &'ctx Context,
    module: &Module,
    machine: &TargetMachine,
    profile: TargetProfile,
) -> Result<LlvmModule<'ctx>, CodegenError> {
    let managed_address_space = profile.managed_address_space_contract();
    let llvm = context.create_module("scoop");
    let builder = context.create_builder();
    let target_data = machine.get_target_data();
    llvm.set_triple(&machine.get_triple());
    llvm.set_data_layout(&target_data.get_data_layout());

    let ptr_ty = context.ptr_type(AddressSpace::default());
    let i8_ty = context.i8_type();
    let i64_ty = context.i64_type();

    // ScoopTypeDescriptor (runtime spec 2.2, full M6 form):
    // { i64 type_id, i64 size, i64 align, ptr ref_offsets, ptr parent,
    //   ptr vtable, ptr itables, i64 itable_count, ptr name }.
    let td_ty = context.struct_type(
        &[
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            ptr_ty.into(),
            ptr_ty.into(),
            ptr_ty.into(),
            ptr_ty.into(),
            i64_ty.into(),
            ptr_ty.into(),
        ],
        false,
    );
    // Declare every local and external descriptor before building any
    // initializer. Semantic edges resolve through typed ids; symbols are read
    // only from the selected entity at final emission.
    let type_tds: Vec<GlobalValue> = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| llvm.add_global(td_ty, None, &descriptor.symbol))
        .collect();
    let external_type_tds: Vec<GlobalValue> = module
        .meta
        .external_type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let global = llvm.add_global(td_ty, None, &descriptor.symbol);
            global.set_linkage(inkwell::module::Linkage::External);
            global
        })
        .collect();
    let string_td = type_descriptor_global(
        module.meta.well_known_type_descriptors.string,
        &type_tds,
        &external_type_tds,
    )?;
    let array_tds: Vec<GlobalValue> = module
        .meta
        .arrays
        .iter()
        .map(|(_, array)| {
            type_descriptor_global(array.type_descriptor, &type_tds, &external_type_tds)
        })
        .collect::<Result<_, _>>()?;

    // Shared "array index out of bounds" message (only when the module
    // performs a checked array access); trap blocks reference it.
    let bounds_message = if module_uses_bounds_checks(module) {
        let bytes = b"array index out of bounds";
        let ty = i8_ty.array_type(bytes.len() as u32 + 1);
        let global = llvm.add_global(ty, None, "scoop.trap.bounds");
        global.set_constant(true);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_initializer(&context.const_string(bytes, true));
        Some(global)
    } else {
        None
    };

    // Ordinary globals are disjoint from descriptor identities.
    let mut globals: Vec<Option<GlobalValue>> = Vec::with_capacity(module.globals.len());
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                // { ptr td, i64 gc_word, i64 len, [N x i8] data }
                // (runtime spec 2.4; the 16-byte header is M9).
                let bytes = value.as_bytes();
                let ty = context.struct_type(
                    &[
                        ptr_ty.into(),
                        i64_ty.into(),
                        i64_ty.into(),
                        i8_ty.array_type(bytes.len() as u32).into(),
                    ],
                    false,
                );
                let llvm_global =
                    llvm.add_global(ty, Some(managed_address_space.inkwell()), &global.symbol);
                llvm_global.set_constant(true);
                llvm_global.set_initializer(&context.const_struct(
                    &[
                        string_td.as_pointer_value().into(),
                        i64_ty.const_zero().into(),
                        i64_ty.const_int(bytes.len() as u64, false).into(),
                        context.const_string(bytes, false).into(),
                    ],
                    false,
                ));
                globals.push(Some(llvm_global));
            }
            GlobalInit::CString(value) => {
                // [N+1 x i8] c"...\00" (e.g. trap messages); private,
                // only referenced from within the module.
                let bytes = value.as_bytes();
                let ty = i8_ty.array_type(bytes.len() as u32 + 1);
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
                llvm_global.set_constant(true);
                llvm_global.set_linkage(inkwell::module::Linkage::Private);
                llvm_global.set_initializer(&context.const_string(bytes, true));
                globals.push(Some(llvm_global));
            }
            GlobalInit::Storage {
                ty,
                initializer,
                thread_local,
            } => {
                let ty = basic_ty(
                    context,
                    &module.structs,
                    &module.enums,
                    managed_address_space,
                    ty,
                )?;
                let value = llvm_constant(
                    context,
                    &module.structs,
                    &module.enums,
                    managed_address_space,
                    ty,
                    initializer,
                )?;
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
                llvm_global.set_initializer(&value);
                llvm_global.set_thread_local(*thread_local);
                globals.push(Some(llvm_global));
            }
        }
    }
    image_roots::emit(
        context,
        &llvm,
        &target_data,
        &module.globals,
        &globals,
        string_td,
    )?;

    // Two passes: declare every function first so call sites never
    // create shadow extern declarations (a forward call would
    // otherwise declare the symbol as extern, and the later definition
    // would be renamed with a `.N` suffix by LLVM, breaking the link).
    let module_ctx = ModuleCtx {
        managed_address_space,
        functions: &module.functions,
        structs: &module.structs,
        enums: &module.enums,
        extern_functions: &module.extern_functions,
        native_globals: &module.native_globals,
        native_global_bridges: &module.native_global_bridges,
        foreign_callback_bridges: &module.foreign_callback_bridges,
        globals_arena: &module.globals,
        globals: &globals,
        arrays: &module.meta.arrays,
        array_tds: &array_tds,
        type_tds: &type_tds,
        external_type_tds: &external_type_tds,
        target_data: &target_data,
        bounds_message,
    };
    for function in &module.functions {
        declare_function(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            profile,
            function,
        )?;
    }
    for (_, callback) in module.callback_bridges.iter() {
        declare_callback_trampoline(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callback,
        )?;
    }
    let mut declared_foreign_trampolines = HashSet::new();
    for (_, callback) in module.foreign_callback_bridges.iter() {
        if !declared_foreign_trampolines.insert(callback.trampoline_symbol.as_str()) {
            continue;
        }
        declare_foreign_callback_trampoline(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callback,
        )?;
        let descriptor = llvm.add_global(context.i8_type(), None, &callback.signature_symbol);
        descriptor.set_linkage(inkwell::module::Linkage::External);
    }
    // Meta TypeDescriptors reference module functions (vtable / itable
    // slots), so they are emitted after the declare pass.
    emit_type_descriptors(context, &llvm, &type_tds, &external_type_tds, module)?;
    for function in &module.functions {
        emit_function(context, &llvm, &builder, &module_ctx, function)?;
    }
    Ok(llvm)
}

/// The LLVM type of a (non-void) LIR type: aggregates are literal
/// structs per the layout in LIR meta; Unit is the empty struct `{}`;
/// enums follow their fixed representation (spec 7.4).
fn basic_ty<'ctx>(
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

fn llvm_constant<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    ty: BasicTypeEnum<'ctx>,
    value: &ConstantValue,
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    Ok(match value {
        ConstantValue::Int(value) => context.i64_type().const_int(*value as u64, true).into(),
        ConstantValue::Bool(value) => context
            .bool_type()
            .const_int(u64::from(*value), false)
            .into(),
        ConstantValue::NullPointer(_) => ty.into_pointer_type().const_null().into(),
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
                        llvm_constant(context, structs, enums, managed_address_space, ty, value)
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

fn alignment_anchor<'ctx>(
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

fn c_field_size(
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

fn c_payload_fields<'ctx>(
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
fn c_physical_field_index(
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

fn struct_ty<'ctx>(
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
fn uses_return_slot(enums: &Arena<EnumDef>, ty: &LirType) -> bool {
    match ty {
        LirType::Aggregate(_) | LirType::Struct(_) | LirType::ExceptionRecord => true,
        LirType::Enum(id) => matches!(enums[*id].repr, EnumRepr::Tagged { .. }),
        LirType::Void | LirType::I1 | LirType::I64 | LirType::Ptr(_) => false,
    }
}

/// Physical storage for a tagged enum. Non-reference payload remains opaque,
/// but every fixed GC slot is an AS1 pointer field so SROA cannot turn a
/// managed reference into integer/byte fragments.
fn tagged_ty<'ctx>(
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

fn flatten_ref_scan(scan: &RefScan, offsets: &mut Vec<u64>) {
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

fn push_byte_padding<'ctx>(
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

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

/// Whether any function needs the shared array-bounds trap message.
/// Array parameters can be indexed without any array being allocated
/// in this module, so this is intentionally independent of generated
/// array TypeDescriptors.
fn module_uses_bounds_checks(module: &Module) -> bool {
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

/// Native/code/metadata pointer type.
fn ptr_ty(context: &Context) -> inkwell::types::PointerType<'_> {
    context.ptr_type(AddressSpace::default())
}

/// Moving-GC object-start pointer type. Address space 1 is part of the fixed
/// LLVM 22.1 backend contract and is the only address space RS4GC traces.
fn managed_ptr_ty(
    context: &Context,
    managed_address_space: ManagedAddressSpace,
) -> inkwell::types::PointerType<'_> {
    context.ptr_type(managed_address_space.inkwell())
}

fn pointer_ty(
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

mod artifact;
mod type_descriptors;

use type_descriptors::{emit_ref_scan, emit_type_descriptors, type_descriptor_global};

/// Translate one LIR function. Signature (parameters and return type)
/// comes from LIR; parameters are SSA values (`Value::Param`).
fn fn_type_of<'ctx>(
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

/// Declare a function with its final symbol and signature. Managed
/// functions carry the GC strategy (M9, milestone9 DESIGN 5.5):
/// `rewrite-statepoints-for-gc` rewrites the body's call sites into
/// statepoints and LLVM emits their stackmaps. Only module functions
/// get it — the runtime declarations created at call sites are not
/// managed code.
fn declare_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    profile: TargetProfile,
    function: &Function,
) -> Result<(), CodegenError> {
    let fn_ty = fn_type_of(
        context,
        structs,
        enums,
        profile.managed_address_space_contract(),
        function,
    )?;
    let llvm_function = llvm.add_function(&function.symbol, fn_ty, None);
    statepoint::configure_function(context, llvm_function, function.gc_effect, profile);
    Ok(())
}

fn c_basic_ty<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    ty: &scoop_lir::CType,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    Ok(match ty {
        scoop_lir::CType::Int | scoop_lir::CType::UInt => context.i64_type().into(),
        scoop_lir::CType::Boolean => context.bool_type().into(),
        scoop_lir::CType::Pointer | scoop_lir::CType::FunctionPointer { .. } => {
            ptr_ty(context).into()
        }
        scoop_lir::CType::Struct(id) => {
            struct_ty(context, structs, enums, managed_address_space, *id)?.into()
        }
        scoop_lir::CType::Unit => {
            return Err(CodegenError(
                "Unit cannot be a C callback parameter type".to_string(),
            ));
        }
    })
}

fn declare_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    callback: &scoop_lir::CallbackBridge,
) -> Result<(), CodegenError> {
    let params = callback
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, managed_address_space, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = if callback.return_type == scoop_lir::CType::Unit {
        context.void_type().fn_type(&params, false)
    } else {
        c_basic_ty(
            context,
            structs,
            enums,
            managed_address_space,
            &callback.return_type,
        )?
        .fn_type(&params, false)
    };
    llvm.add_function(&callback.trampoline_symbol, fn_ty, None);
    Ok(())
}

fn declare_foreign_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    callback: &scoop_lir::ForeignCallbackBridge,
) -> Result<(), CodegenError> {
    let params = callback
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, managed_address_space, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = if callback.return_type == scoop_lir::CType::Unit {
        context.void_type().fn_type(&params, false)
    } else {
        c_basic_ty(
            context,
            structs,
            enums,
            managed_address_space,
            &callback.return_type,
        )?
        .fn_type(&params, false)
    };
    llvm.add_function(&callback.trampoline_symbol, fn_ty, None);
    Ok(())
}

/// Module-level data function emission needs, bundled to keep
/// signatures small.
struct ModuleCtx<'a, 'ctx> {
    managed_address_space: ManagedAddressSpace,
    functions: &'a [Function],
    structs: &'a Arena<StructDef>,
    enums: &'a Arena<EnumDef>,
    extern_functions: &'a ExternFunctions,
    native_globals: &'a Arena<NativeGlobal>,
    native_global_bridges: &'a scoop_lir::NativeGlobalBridges,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    arrays: &'a Arena<ArrayType>,
    array_tds: &'a [GlobalValue<'ctx>],
    type_tds: &'a [GlobalValue<'ctx>],
    external_type_tds: &'a [GlobalValue<'ctx>],
    target_data: &'a inkwell::targets::TargetData,
    bounds_message: Option<GlobalValue<'ctx>>,
}

fn compiler_root_source_key(source: scoop_lir::CallerRootSource) -> (u8, u32) {
    match source {
        scoop_lir::CallerRootSource::Param(index) => (0, index),
        scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
        scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
    }
}

fn root_storage_sources(function: &Function) -> Vec<scoop_lir::CallerRootSource> {
    let mut sources = HashSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            match instruction {
                Instruction::ManagedPoll { site } => {
                    sources.extend(site.live.as_slice().iter().map(|value| value.source));
                }
                Instruction::ArrayAlloc { live, .. } | Instruction::ArrayClone { live, .. } => {
                    sources.extend(live.as_slice().iter().map(|value| value.source));
                }
                Instruction::Call { site } => match site {
                    scoop_lir::CallSite::Managed(site) => {
                        sources.extend(site.live.as_slice().iter().map(|value| value.source));
                    }
                    scoop_lir::CallSite::NativeSafe(site) => {
                        sources.extend(site.roots.as_slice().iter().map(|root| root.source));
                    }
                    scoop_lir::CallSite::NativeBorrowed(site) => {
                        sources.extend(site.roots.as_slice().iter().map(|root| root.source));
                    }
                    scoop_lir::CallSite::NoGc(_) => {}
                },
                Instruction::Invoke {
                    site: scoop_lir::InvokeSite::Managed(site),
                } => {
                    sources.extend(site.roots.as_slice().iter().map(|root| root.root.source));
                }
                Instruction::Invoke {
                    site: scoop_lir::InvokeSite::NoGc(_),
                } => {}
                Instruction::NativeGlobalLoad { roots, .. }
                | Instruction::NativeGlobalStore { roots, .. }
                | Instruction::NativeGlobalAddress { roots, .. } => {
                    sources.extend(roots.as_slice().iter().map(|root| root.source));
                }
                _ => {}
            }
        }
    }
    let mut sources = sources.into_iter().collect::<Vec<_>>();
    sources.sort_by_key(|source| compiler_root_source_key(*source));
    sources
}

fn instruction_temp_defs(instruction: &Instruction) -> [Option<TempId>; 2] {
    let first = match instruction {
        Instruction::BinOp { out, .. }
        | Instruction::UnaryOp { out, .. }
        | Instruction::MakeAggregate { out, .. }
        | Instruction::ExtractValue { out, .. }
        | Instruction::HeapLoad { out, .. }
        | Instruction::AtomicLoad { out, .. }
        | Instruction::AtomicCompareExchange { out, .. }
        | Instruction::GlobalLoad { out, .. }
        | Instruction::GlobalAddress { out, .. }
        | Instruction::NativeGlobalLoad { out, .. }
        | Instruction::NativeGlobalAddress { out, .. }
        | Instruction::FunctionAddress { out, .. }
        | Instruction::ForeignCallbackRegister { out, .. }
        | Instruction::IntToPtr { out, .. }
        | Instruction::PtrToInt { out, .. }
        | Instruction::RawLoad { out, .. }
        | Instruction::PtrOffset { out, .. }
        | Instruction::LocalAddress { out, .. }
        | Instruction::BeginCatch { out, .. }
        | Instruction::ArrayAlloc { out, .. }
        | Instruction::ArrayLen { out, .. }
        | Instruction::ArrayGet { out, .. }
        | Instruction::ArrayClone { out, .. }
        | Instruction::EnumWrap { out, .. }
        | Instruction::EnumTag { out, .. }
        | Instruction::EnumField { out, .. } => Some(*out),
        Instruction::Call { site } => site.direct_out(),
        Instruction::Invoke { site } => site.direct_out(),
        Instruction::LandingPad { record, .. } | Instruction::CleanupPad { record, .. } => {
            Some(*record)
        }
        Instruction::ForeignCallbackOperation(operation) => operation.out(),
        Instruction::Store { .. }
        | Instruction::GlobalStore { .. }
        | Instruction::NativeGlobalStore { .. }
        | Instruction::HeapStore { .. }
        | Instruction::AtomicStore { .. }
        | Instruction::RawStore { .. }
        | Instruction::ManagedPoll { .. }
        | Instruction::EndCatch
        | Instruction::Throw { .. }
        | Instruction::ArraySet { .. } => None,
    };
    let second = match instruction {
        Instruction::LandingPad { raw, .. } | Instruction::CleanupPad { raw, .. } => Some(*raw),
        _ => None,
    };
    [first, second]
}

fn compiler_unwind_plan(
    function: &Function,
) -> (
    HashMap<scoop_lir::BlockId, Vec<scoop_lir::CallerRootSource>>,
    HashSet<scoop_lir::BlockId>,
) {
    let mut sources = HashMap::<_, HashSet<_>>::new();
    let mut blocks = HashSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            let Instruction::Invoke { site } = instruction else {
                continue;
            };
            blocks.insert(site.unwind());
            if let scoop_lir::InvokeSite::Managed(site) = site {
                let entry = sources.entry(site.unwind).or_default();
                entry.extend(
                    site.roots
                        .as_slice()
                        .iter()
                        .filter(|root| root.unwind_live)
                        .map(|root| root.root.source),
                );
            }
        }
    }
    let sources = sources
        .into_iter()
        .map(|(block, set)| {
            let mut set = set.into_iter().collect::<Vec<_>>();
            set.sort_by_key(|source| compiler_root_source_key(*source));
            (block, set)
        })
        .collect();
    (sources, blocks)
}

#[cfg(test)]
mod tests;
