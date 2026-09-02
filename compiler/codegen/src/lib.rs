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
//! M9: GC support (milestone9 DESIGN section 3.1). Every heap object
//! carries the 16-byte header `{ td, gc_word }`: class fields start at
//! their natural byte offsets from 16, the boxed payload and array size
//! live at offset 16, array elements at `align_up(24, element_align)`, and string constants get a zeroed GC word
//! between the TD and the length. M13 allocation sites inline the per-thread
//! TLAB bump and use a GC-leaf finish helper to initialize both header words;
//! a failed bump calls the collecting slow path. Every managed function is
//! declared with the `statepoint-example` GC strategy and the whole
//! module runs through `rewrite-statepoints-for-gc` before object
//! emission, so the `.o` carries the `__llvm_stackmaps` section the
//! runtime's stack scan reads (impl spec 2.4's statepoint insertion
//! happens here rather than in LIR: it is an instrumentation of the
//! emitted LLVM, and keeping it out of LIR keeps the LIR dumps
//! stable). Verified `@NoGC` functions carry no strategy or polls.
//! Safepoint polls — plain calls to the runtime's `scoop_rt_safepoint` —
//! are emitted at every managed function entry and at
//! the loop-header blocks `loop_headers` finds. Every `HeapStore` /
//! `ArraySet` is followed by the write-barrier card mark
//! (`atomicrmw or scoop_gc_card_table[addr >> 9], 1 monotonic`, runtime spec 3.6).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{FileType, TargetMachine};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicValue, BasicValueEnum, GlobalValue, IntValue, PointerValue, ValueKind};
use inkwell::{AddressSpace, AtomicOrdering, AtomicRMWBinOp, IntPredicate};
use la_arena::{Arena, Idx};
use scoop_lir::{
    ArrayType, ArrayTypeId, BinOp, CallableRef, ConstantValue, DispatchEntry, EnumDef, EnumRepr,
    ExternFunction, ExternFunctionKind, Function, GcEffect, Global, GlobalInit, Instruction,
    LirType, Module, NativeGlobal, RefScan, StructDef, TempId, Terminator, TypeDescriptor,
    TypeDescriptorRef, TypeDescriptorScan, UnOp, Value,
};

const SCAN_ARRAY: u64 = u64::MAX;
const SCAN_SEQUENCE: u64 = u64::MAX - 1;

/// The GC strategy set on every generated function (M9, milestone9
/// DESIGN 5.5): LLVM's built-in statepoint strategy, verified by the
/// M0 spike. Runtime functions get no strategy — they are not managed
/// code.
const GC_STRATEGY: &str = "statepoint-example";

/// Runtime safepoint poll (M9, milestone9 DESIGN 3.1): a `void()`
/// call the GC can suspend the polling thread at; emitted at every
/// function entry and loop header.
const SAFEPOINT_SYMBOL: &str = "scoop_rt_safepoint";

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
mod target;

pub use c_bridge::{c_bridge_source, c_layout_assertions};
pub use target::{LlvmVersion, TargetProfile, TargetProfileId, linked_llvm_version};

fn align_up(value: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

fn array_data_offset(element_align: u64) -> u64 {
    align_up(24, element_align)
}

/// Translate `module` to LLVM IR and emit an object file at `output` using the
/// complete target profile selected by the driver.
pub fn emit_object(
    module: &Module,
    output: &Path,
    profile: TargetProfile,
) -> Result<(), CodegenError> {
    let machine = profile.create_target_machine()?;
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine)?;

    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid LLVM module: {e}")))?;

    // M9 (milestone9 DESIGN 3.1, M0 spike): rewrite every call and
    // invoke in the GC-strategy functions into a `gc.statepoint`; the
    // object file's `__llvm_stackmaps` section is produced from them.
    // Runs after verification, right before object emission.
    llvm.run_passes(
        "rewrite-statepoints-for-gc",
        &machine,
        PassBuilderOptions::create(),
    )
    .map_err(|e| CodegenError(format!("rewrite-statepoints-for-gc failed: {e}")))?;

    machine
        .write_to_file(&llvm, FileType::Object, output)
        .map_err(|e| CodegenError(format!("failed to write {}: {e}", output.display())))?;
    Ok(())
}

/// Test helper for constructing the one supported host profile. Production
/// code receives a profile selected by the driver.
#[cfg(test)]
fn host_target_machine() -> Result<TargetMachine, CodegenError> {
    let triple = TargetMachine::get_default_triple();
    let triple = triple
        .as_str()
        .to_str()
        .map_err(|error| CodegenError(format!("host target triple is not UTF-8: {error}")))?;
    TargetProfile::resolve(triple)?.create_target_machine()
}

/// Translate `module` to an (unverified) LLVM module: globals,
/// TypeDescriptors, and every function.
fn emit_llvm_module<'ctx>(
    context: &'ctx Context,
    module: &Module,
    machine: &TargetMachine,
) -> Result<LlvmModule<'ctx>, CodegenError> {
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
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
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
                let ty = basic_ty(context, &module.structs, &module.enums, ty)?;
                let value =
                    llvm_constant(context, &module.structs, &module.enums, ty, initializer)?;
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
                llvm_global.set_initializer(&value);
                llvm_global.set_thread_local(*thread_local);
                globals.push(Some(llvm_global));
            }
        }
    }

    // Two passes: declare every function first so call sites never
    // create shadow extern declarations (a forward call would
    // otherwise declare the symbol as extern, and the later definition
    // would be renamed with a `.N` suffix by LLVM, breaking the link).
    let module_ctx = ModuleCtx {
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
        declare_function(context, &llvm, &module.structs, &module.enums, function)?;
    }
    for (_, callback) in module.callback_bridges.iter() {
        declare_callback_trampoline(context, &llvm, &module.structs, &module.enums, callback)?;
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
        LirType::Ptr(_) => context.ptr_type(AddressSpace::default()).into(),
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
                .map(|element| basic_ty(context, structs, enums, element))
                .collect::<Result<_, _>>()?;
            context.struct_type(&fields, false).into()
        }
        LirType::Struct(id) => struct_ty(context, structs, enums, *id)?.into(),
        LirType::Enum(id) => match &enums[*id].repr {
            // Niche optimization: the value is a bare pointer.
            EnumRepr::Niche { .. } => context.ptr_type(AddressSpace::default()).into(),
            EnumRepr::Tagged { size, align, .. } => tagged_ty(context, *size, *align)?.into(),
        },
    })
}

fn llvm_constant<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    ty: BasicTypeEnum<'ctx>,
    value: &ConstantValue,
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    Ok(match value {
        ConstantValue::Int(value) => context.i64_type().const_int(*value as u64, true).into(),
        ConstantValue::Bool(value) => context
            .bool_type()
            .const_int(u64::from(*value), false)
            .into(),
        ConstantValue::NullPointer(_) => ptr_ty(context).const_null().into(),
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
                        let ty = basic_ty(context, structs, enums, &field.ty)?;
                        llvm_constant(context, structs, enums, ty, value)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                struct_type.const_named_struct(&values).into()
            } else {
                let payload_types = c_payload_fields(context, structs, enums, definition)?;
                let mut payload_values = Vec::with_capacity(payload_types.len());
                let mut cursor = 0u64;
                for (field, value) in definition.fields.iter().zip(fields) {
                    if field.layout.offset > cursor {
                        let ty = payload_types[payload_values.len()];
                        payload_values.push(ty.const_zero());
                    }
                    let field_ty = basic_ty(context, structs, enums, &field.ty)?;
                    payload_values.push(llvm_constant(context, structs, enums, field_ty, value)?);
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
        physical.push(basic_ty(context, structs, enums, &field.ty)?);
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
    id: scoop_lir::StructDefId,
) -> Result<StructType<'ctx>, CodegenError> {
    let definition = &structs[id];
    if definition.c_layout.is_none() {
        let fields = definition
            .fields
            .iter()
            .map(|field| basic_ty(context, structs, enums, &field.ty))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(context.struct_type(&fields, false));
    }
    let anchor = alignment_anchor(context, definition.align)?;
    let payload = context.struct_type(
        &c_payload_fields(context, structs, enums, definition)?,
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

/// Opaque physical storage for a tagged enum. LIR has already assigned
/// the shared pure-value region and every disjoint ref-bearing slot.
fn tagged_ty(
    context: &Context,
    size: u64,
    align: u64,
) -> Result<inkwell::types::StructType<'_>, CodegenError> {
    let bytes = size
        .checked_sub(8)
        .ok_or_else(|| CodegenError(format!("tagged enum size {size} is smaller than its tag")))?;
    let mut fields: Vec<BasicTypeEnum> = vec![
        context.i64_type().into(),
        context.i8_type().array_type(bytes as u32).into(),
    ];
    if align > 8 {
        fields.push(alignment_anchor(context, align)?);
    }
    Ok(context.struct_type(&fields, false))
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

/// The (opaque) pointer type shared by all `LirType::Ptr` values.
fn ptr_ty(context: &Context) -> inkwell::types::PointerType<'_> {
    context.ptr_type(AddressSpace::default())
}

mod type_descriptors;

use type_descriptors::{emit_ref_scan, emit_type_descriptors, type_descriptor_global};

/// Per-function emission state: everything instruction translation
/// needs, bundled to keep signatures small.
struct FnEmitter<'a, 'ctx> {
    context: &'ctx Context,
    llvm: &'a LlvmModule<'ctx>,
    builder: &'a inkwell::builder::Builder<'ctx>,
    function: &'a Function,
    llvm_function: inkwell::values::FunctionValue<'ctx>,
    /// Entry block; enum temporaries are alloca'd here (see
    /// `entry_alloca`).
    entry_block: inkwell::basic_block::BasicBlock<'ctx>,
    /// Every LLVM basic block of the function, indexed by `BlockId`
    /// (invoke targets).
    llvm_blocks: &'a [inkwell::basic_block::BasicBlock<'ctx>],
    functions: &'a [Function],
    structs: &'a Arena<StructDef>,
    enums: &'a Arena<EnumDef>,
    extern_functions: &'a Arena<ExternFunction>,
    native_globals: &'a Arena<NativeGlobal>,
    native_global_bridges: &'a scoop_lir::NativeGlobalBridges,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    /// Complete array metadata and descriptor globals, indexed directly by
    /// `ArrayTypeId`.
    arrays: &'a Arena<ArrayType>,
    array_tds: &'a [GlobalValue<'ctx>],
    type_tds: &'a [GlobalValue<'ctx>],
    external_type_tds: &'a [GlobalValue<'ctx>],
    target_data: &'a inkwell::targets::TargetData,
    /// Hidden result pointer for a physically indirect aggregate return.
    return_slot: Option<PointerValue<'ctx>>,
    /// LIR parameters start after the hidden result pointer when present.
    param_offset: u32,
    allocas: Vec<PointerValue<'ctx>>,
    temps: HashMap<TempId, BasicValueEnum<'ctx>>,
    /// Parameters reloaded from compiler caller-root spills after a native
    /// transition. Locals reload from their existing allocas; temp reloads
    /// replace the corresponding entry in `temps`.
    param_reloads: HashMap<u32, BasicValueEnum<'ctx>>,
    native_call_index: u32,
    allocation_index: u32,
    /// Lazily-created shared bounds-check trap block of this function
    /// (one per function, reused by every ArrayGet / ArraySet) and the
    /// module-level "array index out of bounds" message global it
    /// references (`Some` whenever the module uses arrays).
    bounds_trap_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    bounds_message: Option<GlobalValue<'ctx>>,
}

struct PublishedRoot<'ctx> {
    source: scoop_lir::CallerRootSource,
    storage: PointerValue<'ctx>,
    ty: BasicTypeEnum<'ctx>,
}

struct NativeTransition<'ctx> {
    frame: PointerValue<'ctx>,
    transition: PointerValue<'ctx>,
    roots: Vec<PublishedRoot<'ctx>>,
}

enum TypedCallResult<'a> {
    Void,
    Direct {
        out: TempId,
        ty: &'a LirType,
        scan: &'a RefScan,
    },
    Indirect {
        storage: scoop_lir::LocalId,
        ty: &'a LirType,
        scan: &'a RefScan,
    },
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Materialize an operand as an LLVM value: locals are loaded from
    /// their stack slot, temps are SSA values, globals are addressed by
    /// pointer.
    fn value(&self, value: Value) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let context = self.context;
        let function = self.function;
        Ok(match value {
            Value::Local(id) => {
                let ty = basic_ty(context, self.structs, self.enums, &function.locals[id].ty)?;
                self.builder
                    .build_load(ty, self.allocas[arena_index(id)], &function.locals[id].name)
                    .map_err(|e| CodegenError(format!("load %{}: {e}", function.locals[id].name)))?
            }
            Value::Param(index) => match self.param_reloads.get(&index) {
                Some(value) => *value,
                None => self
                    .llvm_function
                    .get_nth_param(index + self.param_offset)
                    .ok_or_else(|| CodegenError(format!("param {index} out of range")))?,
            },
            Value::Temp(id) => *self.temps.get(&id).ok_or_else(|| {
                CodegenError(format!(
                    "temp t{} used before definition",
                    id.into_raw().into_u32()
                ))
            })?,
            Value::IntConst(value) => context.i64_type().const_int(value as u64, true).into(),
            Value::BoolConst(value) => context.bool_type().const_int(value as u64, false).into(),
            Value::NullPointer(_) => ptr_ty(context).const_null().into(),
            Value::TypeDescriptor(reference) => {
                type_descriptor_global(reference, self.type_tds, self.external_type_tds)?
                    .as_pointer_value()
                    .into()
            }
            Value::Global(id) => self.globals[arena_index(id)]
                .expect("ordinary globals are emitted")
                .as_pointer_value()
                .into(),
        })
    }

    fn instruction(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::BinOp { out, op, lhs, rhs } => {
                let lhs = self.value(*lhs)?;
                let rhs = self.value(*rhs)?;
                // Reference equality (`===` / `!==` and `==` on
                // reference types) compares pointers: normalize both
                // sides to i64 before the integer compare.
                let lhs = match lhs {
                    BasicValueEnum::PointerValue(ptr) => builder
                        .build_ptr_to_int(ptr, context.i64_type(), "ptr_as_i64")
                        .map_err(|e| CodegenError(e.to_string()))?,
                    other => other.into_int_value(),
                };
                let rhs = match rhs {
                    BasicValueEnum::PointerValue(ptr) => builder
                        .build_ptr_to_int(ptr, context.i64_type(), "ptr_as_i64")
                        .map_err(|e| CodegenError(e.to_string()))?,
                    other => other.into_int_value(),
                };
                let name = format!("t{}", out.into_raw().into_u32());
                let result: IntValue = match op {
                    BinOp::Add => builder.build_int_add(lhs, rhs, &name),
                    BinOp::Sub => builder.build_int_sub(lhs, rhs, &name),
                    BinOp::Mul => builder.build_int_mul(lhs, rhs, &name),
                    BinOp::SDiv => builder.build_int_signed_div(lhs, rhs, &name),
                    // Signed comparisons; icmp works for both i64 and i1 (== / !=).
                    BinOp::Lt => builder.build_int_compare(IntPredicate::SLT, lhs, rhs, &name),
                    BinOp::Le => builder.build_int_compare(IntPredicate::SLE, lhs, rhs, &name),
                    BinOp::Gt => builder.build_int_compare(IntPredicate::SGT, lhs, rhs, &name),
                    BinOp::Ge => builder.build_int_compare(IntPredicate::SGE, lhs, rhs, &name),
                    BinOp::Eq => builder.build_int_compare(IntPredicate::EQ, lhs, rhs, &name),
                    BinOp::Ne => builder.build_int_compare(IntPredicate::NE, lhs, rhs, &name),
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::UnaryOp { out, op, operand } => {
                let operand = self.value(*operand)?.into_int_value();
                let name = format!("t{}", out.into_raw().into_u32());
                let result = match op {
                    // Neg is `0 - x` (no dedicated LLVM neg instruction).
                    UnOp::Neg => {
                        builder.build_int_sub(operand.get_type().const_zero(), operand, &name)
                    }
                    // Not is `xor x, true`: flips the i1 bit directly.
                    UnOp::Not => {
                        builder.build_xor(operand, operand.get_type().const_all_ones(), &name)
                    }
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::MakeAggregate { out, elements } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let lir_ty = &function.temps[*out].ty;
                let ty = basic_ty(context, self.structs, self.enums, lir_ty)?.into_struct_type();
                let aggregate = if let LirType::Struct(id) = lir_ty
                    && self.structs[*id].c_layout.is_some()
                {
                    let definition = &self.structs[*id];
                    let payload_ty = ty
                        .get_field_type_at_index(1)
                        .expect("C-layout struct has an aligned payload")
                        .into_struct_type();
                    let mut payload = payload_ty.get_undef();
                    for (index, element) in elements.iter().enumerate() {
                        let physical = c_physical_field_index(
                            self.structs,
                            self.enums,
                            definition,
                            index as u32,
                        )?;
                        payload = builder
                            .build_insert_value(payload, self.value(*element)?, physical, &name)
                            .map_err(|e| {
                                CodegenError(format!(
                                    "insert C-layout field @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                            .into_struct_value();
                    }
                    builder
                        .build_insert_value(ty.get_undef(), payload, 1, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "insert C-layout payload @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                        .into_struct_value()
                } else {
                    let mut aggregate = ty.get_undef();
                    for (index, element) in elements.iter().enumerate() {
                        aggregate = builder
                            .build_insert_value(
                                aggregate,
                                self.value(*element)?,
                                index as u32,
                                &name,
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "insertvalue @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                            .into_struct_value();
                    }
                    aggregate
                };
                self.temps.insert(*out, aggregate.into());
            }
            Instruction::ExtractValue {
                out,
                aggregate,
                index,
            } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let aggregate_ty = function.value_ty(self.globals_arena, *aggregate);
                let aggregate = self.value(*aggregate)?.into_struct_value();
                let element = if let LirType::Struct(id) = aggregate_ty
                    && self.structs[id].c_layout.is_some()
                {
                    let payload = builder
                        .build_extract_value(aggregate, 1, "c_layout_payload")
                        .map_err(|e| {
                            CodegenError(format!(
                                "extract C-layout payload @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                        .into_struct_value();
                    let physical = c_physical_field_index(
                        self.structs,
                        self.enums,
                        &self.structs[id],
                        *index,
                    )?;
                    builder
                        .build_extract_value(payload, physical, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "extract C-layout field @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                } else {
                    builder
                        .build_extract_value(aggregate, *index, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "extractvalue @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                };
                self.temps.insert(*out, element);
            }
            Instruction::HeapLoad {
                out,
                object,
                offset,
            } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "field_ptr")?;
                let field_ty =
                    basic_ty(context, self.structs, self.enums, &function.temps[*out].ty)?;
                let element = builder
                    .build_load(field_ty, field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "heap load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::AtomicLoad {
                out,
                object,
                offset,
            } => {
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "atomic_load @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let field_ty =
                    basic_ty(context, self.structs, self.enums, &function.temps[*out].ty)?;
                let element = builder
                    .build_load(field_ty, field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                element
                    .as_instruction_value()
                    .expect("a load is an instruction")
                    .set_atomic_ordering(AtomicOrdering::Acquire)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic load ordering @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::HeapStore {
                object,
                offset,
                value,
            } => {
                // Object fields begin after the 16-byte header.
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "heap_store @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "field_ptr")?;
                builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "heap_store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card.
                self.card_mark(field_ptr)?;
            }
            Instruction::AtomicStore {
                object,
                offset,
                value,
            } => {
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "atomic_store @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let store = builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                store
                    .set_atomic_ordering(AtomicOrdering::Release)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic store ordering @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
            }
            Instruction::AtomicCompareExchange {
                out,
                object,
                offset,
                expected,
                replacement,
            } => {
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "atomic_cmpxchg @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let pair = builder
                    .build_cmpxchg(
                        field_ptr,
                        self.value(*expected)?.into_int_value(),
                        self.value(*replacement)?.into_int_value(),
                        AtomicOrdering::AcquireRelease,
                        AtomicOrdering::Acquire,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic cmpxchg @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let old = builder
                    .build_extract_value(pair, 0, &format!("t{}", out.into_raw().into_u32()))
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic cmpxchg result @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, old);
            }
            Instruction::FunctionAddress { out, symbol } => {
                let function_value = self.llvm.get_function(symbol).ok_or_else(|| {
                    CodegenError(format!(
                        "function_address @{}: unknown function @{}",
                        function.symbol, symbol
                    ))
                })?;
                self.temps.insert(
                    *out,
                    function_value.as_global_value().as_pointer_value().into(),
                );
            }
            Instruction::ForeignCallbackRegister {
                out,
                bridge,
                closure,
            } => {
                let bridge = &self.foreign_callback_bridges[*bridge];
                let closure = self.value(*closure)?.into_pointer_value();
                let adapter = self
                    .llvm
                    .get_function(&bridge.adapter_symbol)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "foreign callback adapter @{} was not emitted",
                            bridge.adapter_symbol
                        ))
                    })?
                    .as_global_value()
                    .as_pointer_value();
                let signature = self
                    .llvm
                    .get_global(&bridge.signature_symbol)
                    .expect("foreign callback signature descriptor is declared")
                    .as_pointer_value();
                let register = self.runtime_fn(
                    "scoop_runtime_callback_register",
                    ptr_ty(context).fn_type(
                        &[
                            ptr_ty(context).into(),
                            ptr_ty(context).into(),
                            ptr_ty(context).into(),
                            context.i32_type().into(),
                        ],
                        false,
                    ),
                );
                let mode = match bridge.mode {
                    scoop_lir::ForeignCallbackMode::Reusable => 0,
                    scoop_lir::ForeignCallbackMode::OneShot => 1,
                };
                let callback_context = builder
                    .build_call(
                        register,
                        &[
                            closure.into(),
                            adapter.into(),
                            signature.into(),
                            context.i32_type().const_int(mode, false).into(),
                        ],
                        "foreign_callback_context",
                    )
                    .map_err(|error| {
                        CodegenError(format!("foreign callback registration: {error}"))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .expect("callback registration returns a context")
                    .into_pointer_value();
                let trampoline = self
                    .llvm
                    .get_function(&bridge.trampoline_symbol)
                    .expect("foreign callback trampoline is declared")
                    .as_global_value()
                    .as_pointer_value();
                let ty = basic_ty(context, self.structs, self.enums, &function.temps[*out].ty)?
                    .into_struct_type();
                let value = builder
                    .build_insert_value(ty.get_undef(), trampoline, 0, "callback_function")
                    .and_then(|value| {
                        builder.build_insert_value(
                            value.into_struct_value(),
                            callback_context,
                            1,
                            "callback_value",
                        )
                    })
                    .map_err(|error| {
                        CodegenError(format!("construct foreign callback value: {error}"))
                    })?;
                self.temps.insert(*out, value.into_struct_value().into());
            }
            Instruction::ForeignCallbackOperation(operation) => {
                let callback = self.value(operation.callback())?.into_struct_value();
                let function_pointer = builder
                    .build_extract_value(callback, 0, "callback_function")
                    .map_err(|error| CodegenError(format!("extract callback function: {error}")))?
                    .into_pointer_value();
                let callback_context = builder
                    .build_extract_value(callback, 1, "callback_context")
                    .map_err(|error| CodegenError(format!("extract callback context: {error}")))?
                    .into_pointer_value();
                match *operation {
                    scoop_lir::ForeignCallbackOperation::Retain { out, .. } => {
                        let retain = self.runtime_fn(
                            "scoop_runtime_callback_retain",
                            ptr_ty(context).fn_type(&[ptr_ty(context).into()], false),
                        );
                        let retained = builder
                            .build_call(retain, &[callback_context.into()], "retained_context")
                            .map_err(|error| {
                                CodegenError(format!("retain foreign callback: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("retain returns a context")
                            .into_pointer_value();
                        let ty =
                            basic_ty(context, self.structs, self.enums, &function.temps[out].ty)?
                                .into_struct_type();
                        let value = builder
                            .build_insert_value(
                                ty.get_undef(),
                                function_pointer,
                                0,
                                "callback_function",
                            )
                            .and_then(|value| {
                                builder.build_insert_value(
                                    value.into_struct_value(),
                                    retained,
                                    1,
                                    "retained_callback",
                                )
                            })
                            .map_err(|error| {
                                CodegenError(format!("construct retained callback: {error}"))
                            })?;
                        self.temps.insert(out, value.into_struct_value().into());
                    }
                    scoop_lir::ForeignCallbackOperation::Release { .. } => {
                        let release = self.runtime_fn(
                            "scoop_runtime_callback_release",
                            context
                                .void_type()
                                .fn_type(&[ptr_ty(context).into()], false),
                        );
                        builder
                            .build_call(release, &[callback_context.into()], "")
                            .map_err(|error| {
                                CodegenError(format!("release foreign callback: {error}"))
                            })?;
                    }
                    scoop_lir::ForeignCallbackOperation::Failure { out, .. } => {
                        let failure = self.runtime_fn(
                            "scoop_runtime_callback_failure",
                            ptr_ty(context).fn_type(&[ptr_ty(context).into()], false),
                        );
                        let value = builder
                            .build_call(failure, &[callback_context.into()], "callback_failure")
                            .map_err(|error| {
                                CodegenError(format!("query callback failure: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("failure query returns a nullable managed reference");
                        self.temps.insert(out, value);
                    }
                    scoop_lir::ForeignCallbackOperation::State { out, .. } => {
                        let state = self.runtime_fn(
                            "scoop_runtime_callback_state",
                            context.i32_type().fn_type(&[ptr_ty(context).into()], false),
                        );
                        let state = builder
                            .build_call(state, &[callback_context.into()], "callback_state")
                            .map_err(|error| {
                                CodegenError(format!("query callback state: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("state query returns a tag")
                            .into_int_value();
                        let state = builder
                            .build_int_z_extend(state, context.i64_type(), "callback_state_tag")
                            .map_err(|error| {
                                CodegenError(format!("extend callback state: {error}"))
                            })?;
                        let LirType::Enum(enum_id) = function.temps[out].ty else {
                            return Err(CodegenError(
                                "callback state result is not an enum".to_string(),
                            ));
                        };
                        let EnumRepr::Tagged { size, align, .. } = &self.enums[enum_id].repr else {
                            return Err(CodegenError(
                                "callback state enum unexpectedly uses a niche".to_string(),
                            ));
                        };
                        let ty = tagged_ty(context, *size, *align)?;
                        let slot = self.entry_alloca(ty.into(), "callback_state_value")?;
                        builder
                            .build_store(slot, ty.const_zero())
                            .map_err(|error| {
                                CodegenError(format!("zero callback state: {error}"))
                            })?;
                        let tag = self.tag_ptr(slot, ty)?;
                        builder.build_store(tag, state).map_err(|error| {
                            CodegenError(format!("construct callback state: {error}"))
                        })?;
                        let value = builder
                            .build_load(ty, slot, "callback_state_value")
                            .map_err(|error| {
                                CodegenError(format!("load callback state: {error}"))
                            })?;
                        self.temps.insert(out, value);
                    }
                }
            }
            Instruction::IntToPtr { out, value } => {
                let value = self.value(*value)?.into_int_value();
                let result = builder
                    .build_int_to_ptr(value, ptr_ty(context), "raw_ptr")
                    .map_err(|e| CodegenError(format!("inttoptr @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::PtrToInt { out, value } => {
                let value = self.value(*value)?.into_pointer_value();
                let result = builder
                    .build_ptr_to_int(value, context.i64_type(), "raw_uint")
                    .map_err(|e| CodegenError(format!("ptrtoint @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::RawLoad {
                out,
                pointer,
                align,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let ty = basic_ty(context, self.structs, self.enums, &function.temps[*out].ty)?;
                let value = builder
                    .build_load(ty, pointer, "raw_load")
                    .map_err(|e| CodegenError(format!("raw load @{}: {e}", function.symbol)))?;
                value
                    .as_instruction_value()
                    .expect("a non-constant load is an instruction")
                    .set_alignment(*align as u32)
                    .map_err(|e| {
                        CodegenError(format!("raw load alignment @{}: {e}", function.symbol))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::RawStore {
                pointer,
                value,
                align,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let store = builder
                    .build_store(pointer, self.value(*value)?)
                    .map_err(|e| CodegenError(format!("raw store @{}: {e}", function.symbol)))?;
                store.set_alignment(*align as u32).map_err(|e| {
                    CodegenError(format!("raw store alignment @{}: {e}", function.symbol))
                })?;
            }
            Instruction::PtrOffset {
                out,
                pointer,
                bytes,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let bytes = self.value(*bytes)?.into_int_value();
                // SAFETY: source semantics make raw pointer arithmetic unsafe;
                // validity of the resulting address remains the caller's duty.
                let result = unsafe {
                    builder.build_gep(context.i8_type(), pointer, &[bytes], "raw_offset")
                }
                .map_err(|e| CodegenError(format!("raw gep @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::LocalAddress { out, local } => {
                self.temps
                    .insert(*out, self.allocas[arena_index(*local)].into());
            }
            Instruction::GlobalLoad { out, global } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                let ty = basic_ty(context, self.structs, self.enums, &function.temps[*out].ty)?;
                let value = builder
                    .build_load(ty, llvm_global.as_pointer_value(), "global_load")
                    .map_err(|error| CodegenError(format!("global load: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::GlobalStore { global, value } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                builder
                    .build_store(llvm_global.as_pointer_value(), self.value(*value)?)
                    .map_err(|error| CodegenError(format!("global store: {error}")))?;
            }
            Instruction::GlobalAddress { out, global } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                self.temps
                    .insert(*out, llvm_global.as_pointer_value().into());
            }
            Instruction::NativeGlobalLoad { out, global, roots } => {
                let native = &self.native_globals[*global];
                let ty = basic_ty(context, self.structs, self.enums, &native.ty)?;
                let slot = self.entry_alloca(ty, "native_global_result")?;
                let symbol = &self.native_global_bridges.gets[native.access.get()].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition =
                    self.publish_native_roots(roots, None, scoop_lir::CallEffect::NativeSafe)?;
                builder
                    .build_call(callee, &[slot.into()], "native_global_get")
                    .map_err(|error| CodegenError(format!("native global read: {error}")))?;
                self.finish_native_transition(transition, scoop_lir::CallEffect::NativeSafe)?;
                let value = builder
                    .build_load(ty, slot, "native_global_value")
                    .map_err(|error| CodegenError(format!("native global load: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::NativeGlobalStore {
                global,
                value,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty = basic_ty(context, self.structs, self.enums, &native.ty)?;
                let slot = self.entry_alloca(ty, "native_global_argument")?;
                builder
                    .build_store(slot, self.value(*value)?)
                    .map_err(|error| CodegenError(format!("native global spill: {error}")))?;
                let scoop_lir::NativeGlobalAccess::Mutable { set, .. } = native.access else {
                    return Err(CodegenError(format!(
                        "native global store targets readonly `{}`",
                        native.source_name
                    )));
                };
                let symbol = &self.native_global_bridges.sets[set].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition =
                    self.publish_native_roots(roots, None, scoop_lir::CallEffect::NativeSafe)?;
                builder
                    .build_call(callee, &[slot.into()], "native_global_set")
                    .map_err(|error| CodegenError(format!("native global write: {error}")))?;
                self.finish_native_transition(transition, scoop_lir::CallEffect::NativeSafe)?;
            }
            Instruction::NativeGlobalAddress { out, global, roots } => {
                let native = &self.native_globals[*global];
                let ty: BasicTypeEnum = ptr_ty(context).into();
                let slot = self.entry_alloca(ty, "native_global_address")?;
                let symbol = &self.native_global_bridges.addresses[native.access.address()].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition =
                    self.publish_native_roots(roots, None, scoop_lir::CallEffect::NativeSafe)?;
                builder
                    .build_call(callee, &[slot.into()], "native_global_address")
                    .map_err(|error| CodegenError(format!("native global address: {error}")))?;
                self.finish_native_transition(transition, scoop_lir::CallEffect::NativeSafe)?;
                let value = builder
                    .build_load(ty, slot, "native_global_pointer")
                    .map_err(|error| CodegenError(format!("native global pointer: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::Store { local, value: v } => {
                let operand = self.value(*v)?;
                builder
                    .build_store(self.allocas[arena_index(*local)], operand)
                    .map_err(|e| {
                        CodegenError(format!("store %{}: {e}", function.locals[*local].name))
                    })?;
            }
            Instruction::NativeCall { site, roots } => {
                self.emit_typed_call(site, Some(roots), None)?;
            }
            Instruction::Call { site } => {
                self.emit_typed_call(site, None, None)?;
            }
            Instruction::Invoke {
                site,
                normal,
                unwind,
            } => {
                self.emit_typed_call(site, None, Some((*normal, *unwind)))?;
            }
            Instruction::LandingPad { record, raw } => {
                // Catch-all landing pad (M8, runtime spec 5). Keep the
                // `{ ptr, i32 }` record and its raw exception pointer
                // separate from BeginCatch: cleanup chains can forward
                // the same exception to an enclosing dispatch block.
                let personality =
                    self.llvm_function
                        .get_personality_function()
                        .ok_or_else(|| {
                            CodegenError(format!(
                                "landingpad @{symbol}: function has no personality function",
                                symbol = function.symbol
                            ))
                        })?;
                let exception_ty = context
                    .struct_type(&[ptr_ty(context).into(), context.i32_type().into()], false);
                let catch_all: BasicValueEnum = ptr_ty(context).const_null().into();
                let landing_pad = builder
                    .build_landing_pad(
                        exception_ty,
                        personality,
                        &[catch_all],
                        false,
                        &format!("t{}", record.into_raw().into_u32()),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "landingpad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let exception_ptr = builder
                    .build_extract_value(landing_pad.into_struct_value(), 0, "exception_ptr")
                    .map_err(|e| {
                        CodegenError(format!(
                            "landingpad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*record, landing_pad);
                self.temps.insert(*raw, exception_ptr);
            }
            Instruction::CleanupPad { record, raw } => {
                // Cleanup-only landing pad for leaving an active catch
                // because of a new exception or a rethrow. It does not
                // call begin_catch; lir-lower either forwards the
                // captured record through ordinary continuation blocks
                // or resumes it after EndCatch balances the handler.
                let personality =
                    self.llvm_function
                        .get_personality_function()
                        .ok_or_else(|| {
                            CodegenError(format!(
                                "cleanup pad @{symbol}: function has no personality function",
                                symbol = function.symbol
                            ))
                        })?;
                let exception_ty =
                    basic_ty(context, self.structs, self.enums, &LirType::ExceptionRecord)?
                        .into_struct_type();
                let landing_pad = builder
                    .build_landing_pad(
                        exception_ty,
                        personality,
                        &[],
                        true,
                        &format!("t{}", record.into_raw().into_u32()),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "cleanup pad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let exception_ptr = builder
                    .build_extract_value(landing_pad.into_struct_value(), 0, "exception_ptr")
                    .map_err(|e| {
                        CodegenError(format!(
                            "cleanup pad @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*record, landing_pad);
                self.temps.insert(*raw, exception_ptr);
            }
            Instruction::BeginCatch { out, raw } => {
                let begin_catch = self.runtime_fn(
                    "__cxa_begin_catch",
                    ptr_ty(context).fn_type(&[ptr_ty(context).into()], false),
                );
                let name = format!("t{}", out.into_raw().into_u32());
                let object = builder
                    .build_call(begin_catch, &[self.value(*raw)?.into()], &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "begin_catch @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| CodegenError("__cxa_begin_catch returned void".to_string()))?;
                self.temps.insert(*out, object);
            }
            Instruction::EndCatch => {
                let end_catch =
                    self.runtime_fn("__cxa_end_catch", context.void_type().fn_type(&[], false));
                builder
                    .build_call(end_catch, &[], "")
                    .map_err(|e| CodegenError(format!("end_catch @{}: {e}", function.symbol)))?;
            }
            Instruction::Throw { exception } => {
                // `void scoop_rt_throw(ptr)` (noreturn; runtime spec 5).
                // The block's Unreachable terminator emits the LLVM
                // `unreachable` after the call, like the trap path.
                let throw = self.runtime_fn(
                    "scoop_rt_throw",
                    context
                        .void_type()
                        .fn_type(&[ptr_ty(context).into()], false),
                );
                throw.add_attribute(
                    AttributeLoc::Function,
                    context.create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
                );
                builder
                    .build_call(throw, &[self.value(*exception)?.into()], "")
                    .map_err(|e| {
                        CodegenError(format!("throw @{symbol}: {e}", symbol = function.symbol))
                    })?;
            }
            Instruction::ArrayAlloc {
                out,
                elements,
                array_type,
            } => {
                // `{ ptr td, i64 gc_word, i64 size, [n x elem] }`
                // (runtime spec 2.5; the 16-byte header is M9):
                // allocate align_up(24, element_align) + n * stride
                // bytes, store the size at offset 16, then store each
                // element in order. The rounded data offset is visible
                // for over-aligned C-layout elements.
                let (array_metadata, td) = self.array_type(*array_type);
                let element_ty =
                    basic_ty(context, self.structs, self.enums, &array_metadata.element)?;
                let stride = array_metadata.element_size;
                let data_offset = array_data_offset(array_metadata.element_align);
                let td = td.as_pointer_value();
                let total = data_offset + elements.len() as u64 * stride;
                let array =
                    self.managed_alloc_value(td, context.i64_type().const_int(total, false))?;
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                builder
                    .build_store(
                        size_ptr,
                        context.i64_type().const_int(elements.len() as u64, false),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array size @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                for (index, element_value) in elements.iter().enumerate() {
                    let element_ptr = self.element_ptr(
                        array,
                        element_ty,
                        context.i64_type().const_int(index as u64, false),
                        "element_ptr",
                    )?;
                    builder
                        .build_store(element_ptr, self.value(*element_value)?)
                        .map_err(|e| {
                            CodegenError(format!(
                                "array element @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                }
                self.temps.insert(*out, array.into());
            }
            Instruction::ArrayLen { out, operand, .. } => {
                let array = self.value(*operand)?.into_pointer_value();
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let size = builder
                    .build_load(context.i64_type(), size_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_len @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, size);
            }
            Instruction::ArrayGet {
                out,
                array,
                index,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let element_ty =
                    basic_ty(context, self.structs, self.enums, &array_metadata.element)?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                let element_ptr = self.element_ptr(array, element_ty, index, "element_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let value = builder
                    .build_load(element_ty, element_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_get @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let element_ty =
                    basic_ty(context, self.structs, self.enums, &array_metadata.element)?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                let element_ptr = self.element_ptr(array, element_ty, index, "element_ptr")?;
                builder
                    .build_store(element_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_set @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card
                // (array element stores are heap stores too).
                self.card_mark(element_ptr)?;
            }
            Instruction::ArrayClone {
                out,
                operand,
                array_type,
            } => {
                // The target descriptor is explicit: converting Array<T> to
                // MutableArray<T> (or back) changes nominal runtime identity.
                let (array_metadata, target_td) = self.array_type(*array_type);
                let stride = array_metadata.element_size;
                let data_offset = array_data_offset(array_metadata.element_align);
                let clone = self.runtime_fn(
                    scoop_lir::ARRAY_CLONE_SYMBOL,
                    ptr_ty(context).fn_type(
                        &[
                            ptr_ty(context).into(),
                            ptr_ty(context).into(),
                            context.i64_type().into(),
                            context.i64_type().into(),
                        ],
                        false,
                    ),
                );
                let name = format!("t{}", out.into_raw().into_u32());
                let result = builder
                    .build_call(
                        clone,
                        &[
                            self.value(*operand)?.into(),
                            target_td.as_pointer_value().into(),
                            context.i64_type().const_int(stride, false).into(),
                            context.i64_type().const_int(data_offset, false).into(),
                        ],
                        &name,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_clone @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "call @{} produced no value",
                            scoop_lir::ARRAY_CLONE_SYMBOL
                        ))
                    })?;
                self.temps.insert(*out, result);
            }
            Instruction::EnumWrap {
                out,
                enum_id,
                variant,
                fields,
            } => {
                let def = &self.enums[*enum_id];
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    EnumRepr::Niche { payload_variant } => {
                        if *variant == *payload_variant {
                            // The payload is the bare pointer itself.
                            self.value(fields[0])?
                        } else {
                            // The payload-less variant is the null pointer.
                            ptr_ty(context).const_null().into()
                        }
                    }
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        // The full value is zero before tag/payload writes,
                        // so every inactive ref-bearing slot is safe for
                        // unconditional GC scanning.
                        let ty = tagged_ty(context, *size, *align)?;
                        let slot = self.entry_alloca(ty.into(), "enum_wrap")?;
                        builder.build_store(slot, ty.const_zero()).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap zero @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let tag_ptr = self.tag_ptr(slot, ty)?;
                        builder
                            .build_store(
                                tag_ptr,
                                context.i64_type().const_int(*variant as u64, false),
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_wrap tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?;
                        let variant_repr = &variants[*variant as usize];
                        for (value, field) in fields.iter().zip(&variant_repr.fields) {
                            let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                            builder
                                .build_store(field_ptr, self.value(*value)?)
                                .map_err(|e| {
                                    CodegenError(format!(
                                        "enum_wrap field @{symbol}: {e}",
                                        symbol = function.symbol
                                    ))
                                })?;
                        }
                        builder.build_load(ty, slot, &name).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::EnumTag {
                out,
                enum_id,
                operand,
            } => {
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the tag value is the variant index — null ↔
                    // the payload-less variant, non-null ↔ the payload
                    // variant.
                    EnumRepr::Niche { payload_variant } => {
                        let non_null = builder
                            .build_int_compare(
                                IntPredicate::NE,
                                operand.into_pointer_value(),
                                ptr_ty(context).const_null(),
                                "non_null",
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?;
                        let i64_ty = context.i64_type();
                        builder
                            .build_select(
                                non_null,
                                i64_ty.const_int(*payload_variant as u64, false),
                                i64_ty.const_int((1 - *payload_variant) as u64, false),
                                &name,
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                    EnumRepr::Tagged { .. } => builder
                        .build_extract_value(operand.into_struct_value(), 0, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "enum_tag @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?,
                };
                self.temps.insert(*out, result);
            }
            Instruction::EnumField {
                out,
                enum_id,
                variant,
                index,
                operand,
            } => {
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the payload is the pointer itself; lir-lower
                    // only emits this on paths where the tag is known.
                    EnumRepr::Niche { .. } => operand,
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        // Reverse of EnumWrap: spill the aggregate into an
                        // entry-block alloca, then load the field out of
                        // the payload area.
                        let ty = tagged_ty(context, *size, *align)?;
                        let slot = self.entry_alloca(ty.into(), "enum_field")?;
                        builder.build_store(slot, operand).map_err(|e| {
                            CodegenError(format!(
                                "enum_field @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let variant_repr = &variants[*variant as usize];
                        let field = &variant_repr.fields[*index as usize];
                        let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                        let field_ty = basic_ty(context, self.structs, self.enums, &field.ty)?;
                        builder
                            .build_load(field_ty, field_ptr, &name)
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_field @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                };
                self.temps.insert(*out, result);
            }
        }
        Ok(())
    }

    /// Allocate a temporary in the entry block. Enum values are
    /// materialized through memory (see EnumWrap / EnumField); allocas
    /// must dominate every use, so they go before the first instruction
    /// of the entry block alongside the locals' slots.
    fn entry_alloca(
        &self,
        ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let builder = self.builder;
        let current = builder
            .get_insert_block()
            .ok_or_else(|| CodegenError("builder has no insertion block".to_string()))?;
        match self.entry_block.get_first_instruction() {
            Some(first) => builder.position_at(self.entry_block, &first),
            None => builder.position_at_end(self.entry_block),
        }
        let alloca = builder.build_alloca(ty, name).map_err(|e| {
            CodegenError(format!(
                "alloca {name} @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })?;
        builder.position_at_end(current);
        Ok(alloca)
    }

    /// Emit one typed direct/dispatch call. Destination, physical signature,
    /// return convention, and GC/native effect all come from the target entity
    /// referenced by `site`; none are reconstructed from operands or symbols.
    fn emit_typed_call(
        &mut self,
        site: &scoop_lir::CallSite,
        native_roots: Option<&[scoop_lir::CallerRoot]>,
        invoke: Option<(scoop_lir::BlockId, scoop_lir::BlockId)>,
    ) -> Result<(), CodegenError> {
        let targets = &self.function.call_targets;
        let (destination, effect, params, result) = match *site {
            scoop_lir::CallSite::Void { target, .. } => {
                let target = &targets.void_targets[target];
                let signature = &targets.void_signatures[target.signature];
                (
                    target.destination,
                    target.effect,
                    signature.params.as_slice(),
                    TypedCallResult::Void,
                )
            }
            scoop_lir::CallSite::Direct { target, out, .. } => {
                let target = &targets.direct_targets[target];
                let signature = &targets.direct_signatures[target.signature];
                (
                    target.destination,
                    target.effect,
                    signature.params.as_slice(),
                    TypedCallResult::Direct {
                        out,
                        ty: &signature.result,
                        scan: &signature.result_scan,
                    },
                )
            }
            scoop_lir::CallSite::IndirectResult {
                target, storage, ..
            } => {
                let target = &targets.indirect_result_targets[target];
                let signature = &targets.indirect_result_signatures[target.signature];
                (
                    target.destination,
                    target.effect,
                    signature.params.as_slice(),
                    TypedCallResult::Indirect {
                        storage,
                        ty: &signature.result.ty,
                        scan: &signature.result.scan,
                    },
                )
            }
        };

        let is_native = matches!(
            effect,
            scoop_lir::CallEffect::NativeSafe | scoop_lir::CallEffect::NativeBorrowed
        );
        if is_native != native_roots.is_some() {
            return Err(CodegenError(format!(
                "typed call @{}: native instruction/effect mismatch",
                self.function.symbol
            )));
        }
        if is_native && invoke.is_some() {
            return Err(CodegenError(format!(
                "typed call @{}: native calls cannot unwind through managed code",
                self.function.symbol
            )));
        }
        if site.args().len() != params.len() {
            return Err(CodegenError(format!(
                "typed call @{}: signature has {} parameters but call has {} arguments",
                self.function.symbol,
                params.len(),
                site.args().len()
            )));
        }

        match &result {
            TypedCallResult::Void => {}
            TypedCallResult::Direct { out, ty, .. } => {
                if &self.function.temps[*out].ty != *ty {
                    return Err(CodegenError(format!(
                        "typed call @{}: direct result temp does not match its signature",
                        self.function.symbol
                    )));
                }
            }
            TypedCallResult::Indirect { storage, ty, .. } => {
                if &self.function.locals[*storage].ty != *ty {
                    return Err(CodegenError(format!(
                        "typed call @{}: result storage does not match its signature",
                        self.function.symbol
                    )));
                }
            }
        }

        // Allocation is the one codegen-expanded runtime primitive. Its typed
        // identity selects the expansion; its symbol is not inspected.
        if destination == scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::Alloc) {
            let TypedCallResult::Direct { out, .. } = result else {
                return Err(CodegenError(format!(
                    "typed allocation @{} must have a direct result",
                    self.function.symbol
                )));
            };
            if effect != scoop_lir::CallEffect::ManagedSafepoint || invoke.is_some() {
                return Err(CodegenError(format!(
                    "typed allocation @{} has an invalid effect or unwind edge",
                    self.function.symbol
                )));
            }
            return self.managed_alloc(out, site.args());
        }

        let mut param_tys = params
            .iter()
            .map(|ty| basic_ty(self.context, self.structs, self.enums, ty).map(Into::into))
            .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
        let fn_ty = match &result {
            TypedCallResult::Void => self.context.void_type().fn_type(&param_tys, false),
            TypedCallResult::Direct { ty, .. } => {
                basic_ty(self.context, self.structs, self.enums, ty)?.fn_type(&param_tys, false)
            }
            TypedCallResult::Indirect { .. } => {
                param_tys.insert(0, ptr_ty(self.context).into());
                self.context.void_type().fn_type(&param_tys, false)
            }
        };

        let mut call_args = site
            .args()
            .iter()
            .map(|argument| self.value(*argument))
            .collect::<Result<Vec<BasicValueEnum<'ctx>>, _>>()?;
        if let TypedCallResult::Indirect { storage, .. } = &result {
            call_args.insert(0, self.allocas[arena_index(*storage)].into());
        }

        let native_result_storage = if native_roots.is_some() {
            match &result {
                TypedCallResult::Direct { ty, scan, .. } if **scan != RefScan::None => {
                    let llvm_ty = basic_ty(self.context, self.structs, self.enums, ty)?;
                    let storage = self.entry_alloca(llvm_ty, "native_result")?;
                    self.builder
                        .build_store(storage, llvm_ty.const_zero())
                        .map_err(|error| CodegenError(format!("zero native result: {error}")))?;
                    Some((storage, llvm_ty, *scan))
                }
                TypedCallResult::Indirect { storage, ty, scan } if **scan != RefScan::None => {
                    let llvm_ty = basic_ty(self.context, self.structs, self.enums, ty)?;
                    let storage = self.allocas[arena_index(*storage)];
                    self.builder
                        .build_store(storage, llvm_ty.const_zero())
                        .map_err(|error| CodegenError(format!("zero native result: {error}")))?;
                    Some((storage, llvm_ty, *scan))
                }
                TypedCallResult::Void
                | TypedCallResult::Direct { .. }
                | TypedCallResult::Indirect { .. } => None,
            }
        } else {
            None
        };
        let transition = if let Some(roots) = native_roots {
            self.publish_native_roots(
                roots,
                native_result_storage.map(|(storage, _, scan)| (storage, scan)),
                effect,
            )
            .map(Some)?
        } else {
            None
        };

        let callee = match destination {
            scoop_lir::CallDestination::Dispatch { .. } => None,
            _ => Some(self.typed_callee(destination, fn_ty)?),
        };
        let call = match (destination, invoke) {
            (scoop_lir::CallDestination::Dispatch { table, slot }, None) => {
                let pointer = self.dispatch_function_pointer(table, slot)?;
                let arguments = call_args
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                self.builder
                    .build_indirect_call(fn_ty, pointer, &arguments, "typed_call")
                    .map_err(|error| CodegenError(format!("typed dispatch call: {error}")))?
            }
            (scoop_lir::CallDestination::Dispatch { table, slot }, Some((normal, unwind))) => {
                let pointer = self.dispatch_function_pointer(table, slot)?;
                self.builder
                    .build_indirect_invoke(
                        fn_ty,
                        pointer,
                        &call_args,
                        self.llvm_blocks[arena_index(normal)],
                        self.llvm_blocks[arena_index(unwind)],
                        "typed_invoke",
                    )
                    .map_err(|error| CodegenError(format!("typed dispatch invoke: {error}")))?
            }
            (_, None) => {
                let arguments = call_args
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                self.builder
                    .build_call(
                        callee.expect("direct destination has a callee"),
                        &arguments,
                        "typed_call",
                    )
                    .map_err(|error| CodegenError(format!("typed direct call: {error}")))?
            }
            (_, Some((normal, unwind))) => self
                .builder
                .build_invoke(
                    callee.expect("direct destination has a callee"),
                    &call_args,
                    self.llvm_blocks[arena_index(normal)],
                    self.llvm_blocks[arena_index(unwind)],
                    "typed_invoke",
                )
                .map_err(|error| CodegenError(format!("typed direct invoke: {error}")))?,
        };
        self.apply_call_effect(call, destination, effect);

        let direct_value = match &result {
            TypedCallResult::Direct { .. } => match call.try_as_basic_value() {
                ValueKind::Basic(value) => Some(value),
                ValueKind::Instruction(_) => {
                    return Err(CodegenError(format!(
                        "typed call @{} produced no direct value",
                        self.function.symbol
                    )));
                }
            },
            TypedCallResult::Void | TypedCallResult::Indirect { .. } => None,
        };

        if let (Some(value), TypedCallResult::Direct { .. }, Some((storage, _, _))) =
            (direct_value, &result, native_result_storage)
        {
            self.builder
                .build_store(storage, value)
                .map_err(|error| CodegenError(format!("store native result: {error}")))?;
        }
        if let Some(transition) = transition {
            self.finish_native_transition(transition, effect)?;
        }

        if let TypedCallResult::Direct { out, .. } = result {
            let value = if let Some((storage, ty, _)) = native_result_storage {
                self.builder
                    .build_load(ty, storage, "native_result")
                    .map_err(|error| CodegenError(format!("load native result: {error}")))?
            } else {
                direct_value.expect("direct typed call produced a value")
            };
            self.temps.insert(out, value);
        }
        Ok(())
    }

    fn typed_callee(
        &self,
        destination: scoop_lir::CallDestination,
        fn_ty: inkwell::types::FunctionType<'ctx>,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, CodegenError> {
        let symbol = match destination {
            scoop_lir::CallDestination::Local(id) => {
                let symbol = self
                    .functions
                    .get(id.into_u32() as usize)
                    .ok_or_else(|| {
                        CodegenError(format!("invalid local function id {}", id.into_u32()))
                    })?
                    .symbol
                    .as_str();
                let function = self.llvm.get_function(symbol).ok_or_else(|| {
                    CodegenError(format!(
                        "typed local target `{symbol}` was not declared in the module pass"
                    ))
                })?;
                if function.get_type() != fn_ty {
                    return Err(CodegenError(format!(
                        "typed target `{symbol}` disagrees with its existing declaration"
                    )));
                }
                return Ok(function);
            }
            scoop_lir::CallDestination::Runtime(function) => function.symbol(),
            scoop_lir::CallDestination::Extern(id) => match &self.extern_functions[id].kind {
                ExternFunctionKind::C { bridge_symbol, .. } => bridge_symbol,
                ExternFunctionKind::Scoop { .. } => &self.extern_functions[id].native_symbol,
            },
            scoop_lir::CallDestination::Dispatch { .. } => {
                unreachable!("dispatch destinations have no direct callee")
            }
        };
        if let Some(function) = self.llvm.get_function(symbol) {
            if function.get_type() != fn_ty {
                return Err(CodegenError(format!(
                    "typed target `{symbol}` disagrees with its existing declaration"
                )));
            }
            Ok(function)
        } else {
            Ok(self.llvm.add_function(symbol, fn_ty, None))
        }
    }

    fn dispatch_function_pointer(
        &self,
        table: Value,
        slot: scoop_lir::DispatchSlotId,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let table = self.value(table)?.into_pointer_value();
        let slot = self.function.call_targets.dispatch_slots[slot];
        // SAFETY: the typed dispatch slot is assigned by lir-lower from the
        // complete vtable/itable/closure layout.
        let slot_pointer = unsafe {
            self.builder.build_gep(
                ptr_ty(self.context),
                table,
                &[self.context.i32_type().const_int(slot.index.into(), false)],
                "dispatch_slot",
            )
        }
        .map_err(|error| CodegenError(format!("typed dispatch slot: {error}")))?;
        self.builder
            .build_load(ptr_ty(self.context), slot_pointer, "dispatch_function")
            .map(BasicValueEnum::into_pointer_value)
            .map_err(|error| CodegenError(format!("typed dispatch load: {error}")))
    }

    fn apply_call_effect(
        &self,
        call: inkwell::values::CallSiteValue<'ctx>,
        destination: scoop_lir::CallDestination,
        effect: scoop_lir::CallEffect,
    ) {
        if matches!(
            effect,
            scoop_lir::CallEffect::NoGc
                | scoop_lir::CallEffect::NativeSafe
                | scoop_lir::CallEffect::NativeBorrowed
        ) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context.create_string_attribute("gc-leaf-function", ""),
            );
        }
        if matches!(
            effect,
            scoop_lir::CallEffect::NativeSafe | scoop_lir::CallEffect::NativeBorrowed
        ) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context
                    .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
            );
        }
        if matches!(
            destination,
            scoop_lir::CallDestination::Runtime(
                scoop_lir::RuntimeFunction::Trap
                    | scoop_lir::RuntimeFunction::Throw
                    | scoop_lir::RuntimeFunction::Rethrow
            )
        ) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context
                    .create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
            );
        }
    }

    fn native_boundary_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(symbol, ty);
        function.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        function
    }

    fn publish_native_roots(
        &mut self,
        roots: &[scoop_lir::CallerRoot],
        result_root: Option<(PointerValue<'ctx>, &RefScan)>,
        effect: scoop_lir::CallEffect,
    ) -> Result<NativeTransition<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let i64_ty = context.i64_type();
        let call_index = self.native_call_index;
        self.native_call_index += 1;

        let mut published = Vec::with_capacity(roots.len());
        let mut entries = Vec::with_capacity(roots.len() + usize::from(result_root.is_some()));
        for (index, root) in roots.iter().enumerate() {
            let (storage, ty) = match root.source {
                scoop_lir::CallerRootSource::Local(id) => (
                    self.allocas[arena_index(id)],
                    basic_ty(
                        context,
                        self.structs,
                        self.enums,
                        &self.function.locals[id].ty,
                    )?,
                ),
                scoop_lir::CallerRootSource::Param(parameter) => {
                    let ty = basic_ty(
                        context,
                        self.structs,
                        self.enums,
                        &self.function.params[parameter as usize],
                    )?;
                    let storage = self.entry_alloca(ty, "caller_root_param")?;
                    builder
                        .build_store(storage, self.value(Value::Param(parameter))?)
                        .map_err(|error| {
                            CodegenError(format!("spill caller-root parameter: {error}"))
                        })?;
                    (storage, ty)
                }
                scoop_lir::CallerRootSource::Temp(temp) => {
                    let ty = basic_ty(
                        context,
                        self.structs,
                        self.enums,
                        &self.function.temps[temp].ty,
                    )?;
                    let storage = self.entry_alloca(ty, "caller_root_temp")?;
                    builder
                        .build_store(storage, self.value(Value::Temp(temp))?)
                        .map_err(|error| {
                            CodegenError(format!("spill caller-root temporary: {error}"))
                        })?;
                    (storage, ty)
                }
            };
            let descriptor = emit_ref_scan(
                context,
                self.llvm,
                &format!("{}.native.{call_index}.root.{index}", self.function.symbol),
                root.scan.as_ref_scan(),
            )
            .expect("LIR caller roots always carry a non-empty scan");
            entries.push((storage, descriptor));
            published.push(PublishedRoot {
                source: root.source,
                storage,
                ty,
            });
        }
        if let Some((storage, scan)) = result_root {
            let descriptor = emit_ref_scan(
                context,
                self.llvm,
                &format!("{}.native.{call_index}.result", self.function.symbol),
                scan,
            )
            .expect("a native result root always carries a non-empty scan");
            entries.push((storage, descriptor));
        }

        let entry_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
        let entries_pointer = if entries.is_empty() {
            ptr.const_null()
        } else {
            let array_ty = entry_ty.array_type(entries.len() as u32);
            let array = self.entry_alloca(array_ty.into(), "caller_root_entries")?;
            for (index, (base, scan)) in entries.into_iter().enumerate() {
                // SAFETY: `index` is within the statically-sized entries array.
                let entry = unsafe {
                    builder.build_gep(
                        array_ty,
                        array,
                        &[
                            context.i32_type().const_zero(),
                            context.i32_type().const_int(index as u64, false),
                        ],
                        "caller_root_entry",
                    )
                }
                .map_err(|error| CodegenError(format!("caller-root entry GEP: {error}")))?;
                let base_field = builder
                    .build_struct_gep(entry_ty, entry, 0, "caller_root_base")
                    .map_err(|error| CodegenError(format!("caller-root base GEP: {error}")))?;
                let scan_field = builder
                    .build_struct_gep(entry_ty, entry, 1, "caller_root_scan")
                    .map_err(|error| CodegenError(format!("caller-root scan GEP: {error}")))?;
                builder
                    .build_store(base_field, base)
                    .map_err(|error| CodegenError(format!("publish caller-root base: {error}")))?;
                builder
                    .build_store(scan_field, scan)
                    .map_err(|error| CodegenError(format!("publish caller-root scan: {error}")))?;
            }
            array
        };

        let frame_ty = context.struct_type(&[ptr.into(), ptr.into(), i64_ty.into()], false);
        let frame = self.entry_alloca(frame_ty.into(), "caller_root_frame")?;
        builder
            .build_store(frame, frame_ty.const_zero())
            .map_err(|error| CodegenError(format!("zero caller-root frame: {error}")))?;
        let push = self.native_boundary_fn(
            "scoop_rt_push_caller_roots",
            context
                .void_type()
                .fn_type(&[ptr.into(), ptr.into(), i64_ty.into()], false),
        );
        builder
            .build_call(
                push,
                &[
                    frame.into(),
                    entries_pointer.into(),
                    i64_ty
                        .const_int(
                            (roots.len() + usize::from(result_root.is_some())) as u64,
                            false,
                        )
                        .into(),
                ],
                "push_caller_roots",
            )
            .map_err(|error| CodegenError(format!("push caller roots: {error}")))?;

        let transition_ty = context.struct_type(
            &[
                ptr.into(),
                ptr.into(),
                i64_ty.into(),
                i64_ty.into(),
                context.i32_type().into(),
                context.i32_type().into(),
            ],
            false,
        );
        let transition = self.entry_alloca(transition_ty.into(), "native_transition")?;
        builder
            .build_store(transition, transition_ty.const_zero())
            .map_err(|error| CodegenError(format!("zero native transition: {error}")))?;
        let stack_pointer = builder
            .build_ptr_to_int(transition, i64_ty, "managed_stack_pointer")
            .map_err(|error| CodegenError(format!("managed stack pointer: {error}")))?;
        let enter_symbol = match effect {
            scoop_lir::CallEffect::NativeSafe => "scoop_rt_enter_native_safe",
            scoop_lir::CallEffect::NativeBorrowed => "scoop_rt_enter_native_borrowed",
            scoop_lir::CallEffect::ManagedSafepoint | scoop_lir::CallEffect::NoGc => {
                unreachable!("non-native effect cannot enter a native transition")
            }
        };
        let enter = self.native_boundary_fn(
            enter_symbol,
            context
                .void_type()
                .fn_type(&[ptr.into(), i64_ty.into()], false),
        );
        builder
            .build_call(
                enter,
                &[transition.into(), stack_pointer.into()],
                "enter_native",
            )
            .map_err(|error| CodegenError(format!("enter native transition: {error}")))?;

        Ok(NativeTransition {
            frame,
            transition,
            roots: published,
        })
    }

    fn finish_native_transition(
        &mut self,
        native: NativeTransition<'ctx>,
        effect: scoop_lir::CallEffect,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let ptr = ptr_ty(context);
        let leave_symbol = match effect {
            scoop_lir::CallEffect::NativeSafe => "scoop_rt_leave_native_safe",
            scoop_lir::CallEffect::NativeBorrowed => "scoop_rt_leave_native_borrowed",
            scoop_lir::CallEffect::ManagedSafepoint | scoop_lir::CallEffect::NoGc => {
                unreachable!("non-native effect cannot leave a native transition")
            }
        };
        let leave = self.native_boundary_fn(
            leave_symbol,
            context.void_type().fn_type(&[ptr.into()], false),
        );
        self.builder
            .build_call(leave, &[native.transition.into()], "leave_native")
            .map_err(|error| CodegenError(format!("leave native transition: {error}")))?;

        for root in native.roots {
            match root.source {
                scoop_lir::CallerRootSource::Local(_) => {}
                scoop_lir::CallerRootSource::Param(parameter) => {
                    let value = self
                        .builder
                        .build_load(root.ty, root.storage, "caller_root_reload")
                        .map_err(|error| {
                            CodegenError(format!("reload caller-root parameter: {error}"))
                        })?;
                    self.param_reloads.insert(parameter, value);
                }
                scoop_lir::CallerRootSource::Temp(temp) => {
                    let value = self
                        .builder
                        .build_load(root.ty, root.storage, "caller_root_reload")
                        .map_err(|error| {
                            CodegenError(format!("reload caller-root temporary: {error}"))
                        })?;
                    self.temps.insert(temp, value);
                }
            }
        }

        let pop = self.native_boundary_fn(
            "scoop_rt_pop_caller_roots",
            context.void_type().fn_type(&[ptr.into()], false),
        );
        self.builder
            .build_call(pop, &[native.frame.into()], "pop_caller_roots")
            .map_err(|error| CodegenError(format!("pop caller roots: {error}")))?;
        Ok(())
    }

    /// Get or declare a runtime function with the given signature.
    fn runtime_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        self.llvm
            .get_function(symbol)
            .unwrap_or_else(|| self.llvm.add_function(symbol, ty, None))
    }

    fn native_global_bridge(&self, symbol: &str) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(
            symbol,
            self.context
                .void_type()
                .fn_type(&[ptr_ty(self.context).into()], false),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        function
    }

    /// M13 managed allocation fast path. Small objects are bumped directly
    /// from the current thread's public two-pointer allocation context. A
    /// failed bump calls the collecting slow path; a successful bump calls a
    /// GC-leaf helper that clears the object, initializes its header, and
    /// atomically records the object start.
    fn managed_alloc(&mut self, out: TempId, args: &[Value]) -> Result<(), CodegenError> {
        let [descriptor, requested_size] = args else {
            return Err(CodegenError(format!(
                "scoop_rt_alloc @{}: expected descriptor and size",
                self.function.symbol
            )));
        };
        let descriptor = self.value(*descriptor)?.into_pointer_value();
        let requested_size = self.value(*requested_size)?.into_int_value();
        let object = self.managed_alloc_value(descriptor, requested_size)?;
        self.temps.insert(out, object.into());
        Ok(())
    }

    fn managed_alloc_value(
        &mut self,
        descriptor: PointerValue<'ctx>,
        requested_size: IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let i64_ty = context.i64_type();

        let below_header = builder
            .build_int_compare(
                IntPredicate::ULT,
                requested_size,
                i64_ty.const_int(16, false),
                "alloc_below_header",
            )
            .map_err(|error| CodegenError(format!("allocation size check: {error}")))?;
        let at_least_header = builder
            .build_select(
                below_header,
                i64_ty.const_int(16, false),
                requested_size,
                "alloc_min_size",
            )
            .map_err(|error| CodegenError(format!("normalize allocation size: {error}")))?
            .into_int_value();
        let aligned_size = builder
            .build_and(
                builder
                    .build_int_add(
                        at_least_header,
                        i64_ty.const_int(7, false),
                        "alloc_size_plus_align",
                    )
                    .map_err(|error| CodegenError(format!("align allocation size: {error}")))?,
                i64_ty.const_int(!7_u64, false),
                "alloc_size",
            )
            .map_err(|error| CodegenError(format!("mask allocation size: {error}")))?;

        let allocation_global = self
            .llvm
            .get_global("scoop_rt_allocation_context")
            .unwrap_or_else(|| {
                let global = self
                    .llvm
                    .add_global(ptr, None, "scoop_rt_allocation_context");
                global.set_thread_local(true);
                global
            });
        let allocation_context = builder
            .build_load(
                ptr,
                allocation_global.as_pointer_value(),
                "allocation_context",
            )
            .map_err(|error| CodegenError(format!("load allocation context: {error}")))?
            .into_pointer_value();
        let allocation_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
        let cursor_slot = builder
            .build_struct_gep(allocation_ty, allocation_context, 0, "tlab_cursor_slot")
            .map_err(|error| CodegenError(format!("address TLAB cursor: {error}")))?;
        let limit_slot = builder
            .build_struct_gep(allocation_ty, allocation_context, 1, "tlab_limit_slot")
            .map_err(|error| CodegenError(format!("address TLAB limit: {error}")))?;
        let cursor = builder
            .build_load(ptr, cursor_slot, "tlab_cursor")
            .map_err(|error| CodegenError(format!("load TLAB cursor: {error}")))?
            .into_pointer_value();
        let limit = builder
            .build_load(ptr, limit_slot, "tlab_limit")
            .map_err(|error| CodegenError(format!("load TLAB limit: {error}")))?
            .into_pointer_value();
        let cursor_int = builder
            .build_ptr_to_int(cursor, i64_ty, "tlab_cursor_int")
            .map_err(|error| CodegenError(format!("convert TLAB cursor: {error}")))?;
        let limit_int = builder
            .build_ptr_to_int(limit, i64_ty, "tlab_limit_int")
            .map_err(|error| CodegenError(format!("convert TLAB limit: {error}")))?;
        let cursor_end = builder
            .build_int_add(cursor_int, aligned_size, "tlab_cursor_end")
            .map_err(|error| CodegenError(format!("advance TLAB cursor: {error}")))?;
        let line_base = builder
            .build_and(
                cursor_int,
                i64_ty.const_int(!127_u64, false),
                "tlab_line_base",
            )
            .map_err(|error| CodegenError(format!("align TLAB line: {error}")))?;
        let line_end = builder
            .build_int_add(line_base, i64_ty.const_int(128, false), "tlab_line_end")
            .map_err(|error| CodegenError(format!("compute TLAB line end: {error}")))?;
        let fits_current_line = builder
            .build_int_compare(IntPredicate::ULE, cursor_end, line_end, "alloc_fits_line")
            .map_err(|error| CodegenError(format!("check TLAB line: {error}")))?;
        let object_int = builder
            .build_select(fits_current_line, cursor_int, line_end, "tlab_object_int")
            .map_err(|error| CodegenError(format!("select TLAB object: {error}")))?
            .into_int_value();
        let next_int = builder
            .build_int_add(object_int, aligned_size, "tlab_next_int")
            .map_err(|error| CodegenError(format!("advance selected TLAB object: {error}")))?;
        let has_tlab = builder
            .build_int_compare(
                IntPredicate::NE,
                cursor_int,
                i64_ty.const_zero(),
                "tlab_present",
            )
            .map_err(|error| CodegenError(format!("check TLAB presence: {error}")))?;
        let is_small = builder
            .build_int_compare(
                IntPredicate::ULE,
                aligned_size,
                i64_ty.const_int(64, false),
                "alloc_is_small",
            )
            .map_err(|error| CodegenError(format!("check small allocation: {error}")))?;
        let within_limit = builder
            .build_int_compare(IntPredicate::ULE, next_int, limit_int, "alloc_within_tlab")
            .map_err(|error| CodegenError(format!("check TLAB limit: {error}")))?;
        let fast = builder
            .build_and(has_tlab, is_small, "alloc_has_small_tlab")
            .and_then(|condition| builder.build_and(condition, within_limit, "alloc_fast_path"))
            .map_err(|error| CodegenError(format!("combine TLAB checks: {error}")))?;

        let index = self.allocation_index;
        self.allocation_index += 1;
        let fast_block =
            context.append_basic_block(self.llvm_function, &format!("alloc.fast.{index}"));
        let slow_block =
            context.append_basic_block(self.llvm_function, &format!("alloc.slow.{index}"));
        let continue_block =
            context.append_basic_block(self.llvm_function, &format!("alloc.continue.{index}"));
        builder
            .build_conditional_branch(fast, fast_block, slow_block)
            .map_err(|error| CodegenError(format!("branch on TLAB fast path: {error}")))?;

        builder.position_at_end(fast_block);
        let object = builder
            .build_int_to_ptr(object_int, ptr, "tlab_object")
            .map_err(|error| CodegenError(format!("materialize TLAB object: {error}")))?;
        let next = builder
            .build_int_to_ptr(next_int, ptr, "tlab_next")
            .map_err(|error| CodegenError(format!("materialize TLAB cursor: {error}")))?;
        builder
            .build_store(cursor_slot, next)
            .map_err(|error| CodegenError(format!("publish TLAB cursor: {error}")))?;
        let finish = self.runtime_fn(
            "scoop_runtime_finish_tlab_alloc",
            context
                .void_type()
                .fn_type(&[ptr.into(), ptr.into(), i64_ty.into()], false),
        );
        finish.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        finish.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute("gc-leaf-function", ""),
        );
        builder
            .build_call(
                finish,
                &[object.into(), descriptor.into(), aligned_size.into()],
                "",
            )
            .map_err(|error| CodegenError(format!("finish TLAB allocation: {error}")))?;
        builder
            .build_unconditional_branch(continue_block)
            .map_err(|error| CodegenError(format!("leave TLAB fast path: {error}")))?;

        builder.position_at_end(slow_block);
        let slow = self.runtime_fn(
            "scoop_runtime_alloc_slow",
            ptr.fn_type(&[ptr.into(), i64_ty.into()], false),
        );
        slow.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        let slow_object = builder
            .build_call(
                slow,
                &[descriptor.into(), aligned_size.into()],
                "slow_object",
            )
            .map_err(|error| CodegenError(format!("slow allocation: {error}")))?
            .try_as_basic_value()
            .basic()
            .expect("allocation slow path returns an object")
            .into_pointer_value();
        builder
            .build_unconditional_branch(continue_block)
            .map_err(|error| CodegenError(format!("leave allocation slow path: {error}")))?;

        builder.position_at_end(continue_block);
        let phi = builder
            .build_phi(ptr, "managed_object")
            .map_err(|error| CodegenError(format!("merge allocation result: {error}")))?;
        phi.add_incoming(&[(&object, fast_block), (&slow_object, slow_block)]);
        Ok(phi.as_basic_value().into_pointer_value())
    }

    fn array_type(&self, id: ArrayTypeId) -> (&ArrayType, GlobalValue<'ctx>) {
        (&self.arrays[id], self.array_tds[arena_index(id)])
    }

    /// Byte-offset GEP from an opaque pointer (object field access).
    fn byte_gep(
        &self,
        ptr: PointerValue<'ctx>,
        offset: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: `ptr` addresses an object at least `offset` bytes
        // large (callers address fields of the runtime object model).
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                ptr,
                &[self.context.i32_type().const_int(offset, false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "gep {name} @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }

    /// M9 write-barrier instrumentation point (milestone9 DESIGN 3.1,
    /// runtime spec 3.6): after a heap store, mark the card covering the
    /// stored-to address with a monotonic atomic OR. Equal plain byte stores
    /// from multiple mutators would still be a data race.
    /// Emitted unconditionally (also for scalar stores); the v1
    /// collector ignores the table, and the generational remembered
    /// set consumes it once generations land.
    fn card_mark(&self, addr: PointerValue<'ctx>) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let error = |e: inkwell::builder::BuilderError| {
            CodegenError(format!(
                "card mark @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        };
        // The card table is a runtime POINTER VARIABLE (`extern
        // unsigned char *scoop_gc_card_table`), pre-biased by the
        // runtime with the arena base so `base + (addr >> 9)` lands
        // inside the backing table for every heap address (see
        // runtime/include/scoop_rt.h): load the pointer, then GEP.
        let card_table_global = self.llvm.get_global(CARD_TABLE_SYMBOL).unwrap_or_else(|| {
            self.llvm
                .add_global(ptr_ty(context), None, CARD_TABLE_SYMBOL)
        });
        let card_table = builder
            .build_load(
                ptr_ty(context),
                card_table_global.as_pointer_value(),
                "card_table",
            )
            .map_err(error)?
            .into_pointer_value();
        let addr = builder
            .build_ptr_to_int(addr, context.i64_type(), "card_addr")
            .map_err(error)?;
        // Logical shift: the card index of the address.
        let card = builder
            .build_right_shift(
                addr,
                context.i64_type().const_int(CARD_SHIFT, false),
                false,
                "card_index",
            )
            .map_err(error)?;
        // SAFETY: the loaded card table base is pre-biased so that
        // `base + card` addresses the card of any heap address (one
        // card per 512 bytes of the GC window).
        let card_ptr =
            unsafe { builder.build_gep(context.i8_type(), card_table, &[card], "card_ptr") }
                .map_err(error)?;
        builder
            .build_atomicrmw(
                AtomicRMWBinOp::Or,
                card_ptr,
                context.i8_type().const_int(1, false),
                AtomicOrdering::Monotonic,
            )
            .map_err(error)?;
        Ok(())
    }

    /// A safepoint poll (M9, milestone9 DESIGN 3.1): a plain call to
    /// the runtime's `scoop_rt_safepoint`, which the
    /// `rewrite-statepoints-for-gc` pass turns into a statepoint —
    /// exactly the spot where the GC can suspend the polling thread.
    fn safepoint_poll(&self) -> Result<(), CodegenError> {
        let safepoint = self.runtime_fn(
            SAFEPOINT_SYMBOL,
            self.context.void_type().fn_type(&[], false),
        );
        self.builder.build_call(safepoint, &[], "").map_err(|e| {
            CodegenError(format!(
                "safepoint poll @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })?;
        Ok(())
    }

    /// Address of element `index` of an array object: the element
    /// area starts after the 16-byte header (M9) + size field, rounded
    /// up to the element type's ABI alignment.
    fn element_ptr(
        &self,
        array: PointerValue<'ctx>,
        element_ty: BasicTypeEnum<'ctx>,
        index: IntValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let data_offset = array_data_offset(self.target_data.get_abi_alignment(&element_ty) as u64);
        let base = self.byte_gep(array, data_offset, "elements")?;
        // SAFETY: `base` addresses the element area of an array whose
        // elements have layout `element_ty`; `index` was bounds-checked
        // against the array size (or is a valid constant index).
        unsafe { self.builder.build_gep(element_ty, base, &[index], name) }.map_err(|e| {
            CodegenError(format!(
                "element gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }

    /// Emit the array bounds check: trap when `(u64)index >= (u64)size`
    /// (the unsigned comparison also rejects negative indexes, which
    /// wrap above every in-range size). On return the builder is
    /// positioned in the in-bounds continuation block; the rest of the
    /// current LIR block (including its terminator) is emitted there.
    fn bounds_check(
        &mut self,
        array: PointerValue<'ctx>,
        index: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let builder = self.builder;
        let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
        let size = builder
            .build_load(self.context.i64_type(), size_ptr, "size")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol
                ))
            })?
            .into_int_value();
        let out_of_bounds = builder
            .build_int_compare(IntPredicate::UGE, index, size, "out_of_bounds")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol
                ))
            })?;
        let ok_block = self
            .context
            .append_basic_block(self.llvm_function, "in_bounds");
        let trap_block = self.bounds_trap_block()?;
        builder
            .build_conditional_branch(out_of_bounds, trap_block, ok_block)
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol
                ))
            })?;
        builder.position_at_end(ok_block);
        Ok(())
    }

    /// The shared bounds-check trap block of this function, created on
    /// first use: `scoop_rt_trap("array index out of bounds")` followed
    /// by `unreachable` (the TRAP_SYMBOL contract: noreturn).
    fn bounds_trap_block(
        &mut self,
    ) -> Result<inkwell::basic_block::BasicBlock<'ctx>, CodegenError> {
        if let Some(block) = self.bounds_trap_block {
            return Ok(block);
        }
        let builder = self.builder;
        let current = builder
            .get_insert_block()
            .ok_or_else(|| CodegenError("builder has no insertion block".to_string()))?;

        let message = self
            .bounds_message
            .ok_or_else(|| {
                CodegenError("bounds check in a module without array types".to_string())
            })?
            .as_pointer_value();

        let trap = self.runtime_fn(
            scoop_lir::TRAP_SYMBOL,
            self.context
                .void_type()
                .fn_type(&[ptr_ty(self.context).into()], false),
        );
        let block = self
            .context
            .append_basic_block(self.llvm_function, "bounds_trap");
        builder.position_at_end(block);
        builder
            .build_call(trap, &[message.into()], "trap")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds trap @{symbol}: {e}",
                    symbol = self.function.symbol
                ))
            })?;
        builder.build_unreachable().map_err(|e| {
            CodegenError(format!(
                "bounds trap @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })?;
        builder.position_at_end(current);
        self.bounds_trap_block = Some(block);
        Ok(block)
    }

    /// Address of the tag field of a tagged enum value in memory.
    fn tag_ptr(
        &self,
        slot: PointerValue<'ctx>,
        ty: inkwell::types::StructType<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: constant indexes 0, 0 address the i64 tag of the
        // `{ i64, [M x i8] }` object `slot` points to.
        unsafe {
            self.builder.build_gep(
                ty,
                slot,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero(),
                ],
                "tag_ptr",
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "tag gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }

    /// Address of one tagged-enum field at its enum-relative byte
    /// offset. LIR owns slot assignment and natural field layout.
    fn enum_field_ptr(
        &self,
        slot: PointerValue<'ctx>,
        offset: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: LIR guarantees every field offset lies within the
        // complete tagged-enum storage represented by `slot`.
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                slot,
                &[self.context.i64_type().const_int(offset, false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "enum field gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }
}

/// Translate one LIR function. Signature (parameters and return type)
/// comes from LIR; parameters are SSA values (`Value::Param`).
fn fn_type_of<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    function: &Function,
) -> Result<inkwell::types::FunctionType<'ctx>, CodegenError> {
    let mut param_tys: Vec<BasicMetadataTypeEnum> = function
        .params
        .iter()
        .map(|ty| basic_ty(context, structs, enums, ty).map(Into::into))
        .collect::<Result<_, _>>()?;
    if uses_return_slot(enums, &function.return_ty) {
        param_tys.insert(0, ptr_ty(context).into());
        return Ok(context.void_type().fn_type(&param_tys, false));
    }
    Ok(match &function.return_ty {
        LirType::Void => context.void_type().fn_type(&param_tys, false),
        return_ty => basic_ty(context, structs, enums, return_ty)?.fn_type(&param_tys, false),
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
    function: &Function,
) -> Result<(), CodegenError> {
    let fn_ty = fn_type_of(context, structs, enums, function)?;
    let llvm_function = llvm.add_function(&function.symbol, fn_ty, None);
    if function.gc_effect == GcEffect::Managed {
        llvm_function.set_gc(GC_STRATEGY);
    }
    Ok(())
}

fn c_basic_ty<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    ty: &scoop_lir::CType,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    Ok(match ty {
        scoop_lir::CType::Int | scoop_lir::CType::UInt => context.i64_type().into(),
        scoop_lir::CType::Boolean => context.bool_type().into(),
        scoop_lir::CType::Pointer | scoop_lir::CType::FunctionPointer { .. } => {
            ptr_ty(context).into()
        }
        scoop_lir::CType::Struct(id) => struct_ty(context, structs, enums, *id)?.into(),
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
    callback: &scoop_lir::CallbackBridge,
) -> Result<(), CodegenError> {
    let params = callback
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = if callback.return_type == scoop_lir::CType::Unit {
        context.void_type().fn_type(&params, false)
    } else {
        c_basic_ty(context, structs, enums, &callback.return_type)?.fn_type(&params, false)
    };
    llvm.add_function(&callback.trampoline_symbol, fn_ty, None);
    Ok(())
}

fn declare_foreign_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    callback: &scoop_lir::ForeignCallbackBridge,
) -> Result<(), CodegenError> {
    let params = callback
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = if callback.return_type == scoop_lir::CType::Unit {
        context.void_type().fn_type(&params, false)
    } else {
        c_basic_ty(context, structs, enums, &callback.return_type)?.fn_type(&params, false)
    };
    llvm.add_function(&callback.trampoline_symbol, fn_ty, None);
    Ok(())
}

/// Module-level data function emission needs, bundled to keep
/// signatures small.
struct ModuleCtx<'a, 'ctx> {
    functions: &'a [Function],
    structs: &'a Arena<StructDef>,
    enums: &'a Arena<EnumDef>,
    extern_functions: &'a Arena<ExternFunction>,
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

/// The loop-header blocks of `function`: targets of back edges, where
/// an edge B -> T is a back edge when T dominates B. Computed over the
/// full LIR control-flow graph (branch terminators plus the unwind
/// edges of invokes) with the standard iterative dominator dataflow.
/// lir-lower's only loop shape is `while`, whose condition block the
/// body branches back to, so in practice the headers are exactly the
/// `while.cond` blocks; a landing pad never dominates its invoke
/// blocks, so no header is a pad (the extra check at the insertion
/// site only restates LLVM's landingpad-first rule).
fn loop_headers(function: &Function) -> Vec<bool> {
    let len = function.blocks.len();
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); len];
    for (id, block) in function.blocks.iter() {
        let from = arena_index(id);
        match &block.terminator {
            Terminator::Br(target) => successors[from].push(arena_index(*target)),
            Terminator::CondBr {
                then_block,
                else_block,
                ..
            } => {
                successors[from].push(arena_index(*then_block));
                successors[from].push(arena_index(*else_block));
            }
            Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {}
        }
        // Invoke is terminator-like: the block's own
        // terminator restates the normal successor (counted above), so
        // only the unwind edge is extra.
        if let Some(Instruction::Invoke { unwind, .. }) = block.instructions.last() {
            successors[from].push(arena_index(*unwind));
        }
    }

    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); len];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }
    // dom(entry) = {entry}; dom(b) = {b} ∪ ⋂ dom(p) over preds p,
    // iterated to a fixed point. Unreachable blocks keep just
    // themselves, the usual treatment.
    let entry = arena_index(function.entry);
    let mut dominators: Vec<HashSet<usize>> = vec![(0..len).collect(); len];
    dominators[entry] = [entry].into_iter().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..len {
            if block == entry {
                continue;
            }
            let mut dom: HashSet<usize> = match predecessors[block].as_slice() {
                [] => [block].into_iter().collect(),
                [first, rest @ ..] => {
                    let mut dom = dominators[*first].clone();
                    for pred in rest {
                        dom.retain(|b| dominators[*pred].contains(b));
                    }
                    dom
                }
            };
            dom.insert(block);
            if dom != dominators[block] {
                dominators[block] = dom;
                changed = true;
            }
        }
    }

    let mut headers = vec![false; len];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            if dominators[from].contains(&to) {
                headers[to] = true;
            }
        }
    }
    headers
}

fn emit_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    module_ctx: &ModuleCtx<'_, 'ctx>,
    function: &Function,
) -> Result<(), CodegenError> {
    // Pre-declared in the first pass (see `emit_object`).
    let llvm_function = llvm
        .get_function(&function.symbol)
        .expect("function declared in the first pass");

    // A function containing a landing pad needs a personality function
    // (M8, runtime spec 5): `scoop_eh_personality`, declared with the
    // same variadic prototype LLVM uses for `__gxx_personality_v0`.
    // Setting it also makes LLVM emit the unwind table entry.
    let has_landing_pad = function.blocks.iter().any(|(_, block)| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
            )
        })
    });
    if has_landing_pad {
        let personality = llvm
            .get_function("scoop_eh_personality")
            .unwrap_or_else(|| {
                llvm.add_function(
                    "scoop_eh_personality",
                    context.i32_type().fn_type(&[], true),
                    None,
                )
            });
        llvm_function.set_personality_function(personality);
    }

    // All blocks up front so terminators can reference them in any order.
    let mut blocks = vec![None; function.blocks.len()];
    let entry_index = arena_index(function.entry);
    blocks[entry_index] =
        Some(context.append_basic_block(llvm_function, &function.blocks[function.entry].name));
    for (id, block) in function.blocks.iter() {
        let index = arena_index(id);
        if index != entry_index {
            blocks[index] = Some(context.append_basic_block(llvm_function, &block.name));
        }
    }
    let blocks: Vec<_> = blocks
        .into_iter()
        .map(|block| block.expect("every LIR block is created"))
        .collect();

    let param_offset = u32::from(uses_return_slot(module_ctx.enums, &function.return_ty));
    let return_slot = (param_offset != 0).then(|| {
        llvm_function
            .get_nth_param(0)
            .expect("return-slot function has its hidden parameter")
            .into_pointer_value()
    });

    let mut emitter = FnEmitter {
        context,
        llvm,
        builder,
        function,
        llvm_function,
        entry_block: blocks[arena_index(function.entry)],
        llvm_blocks: &blocks,
        functions: module_ctx.functions,
        structs: module_ctx.structs,
        enums: module_ctx.enums,
        extern_functions: module_ctx.extern_functions,
        native_globals: module_ctx.native_globals,
        native_global_bridges: module_ctx.native_global_bridges,
        foreign_callback_bridges: module_ctx.foreign_callback_bridges,
        globals_arena: module_ctx.globals_arena,
        globals: module_ctx.globals,
        arrays: module_ctx.arrays,
        array_tds: module_ctx.array_tds,
        type_tds: module_ctx.type_tds,
        external_type_tds: module_ctx.external_type_tds,
        target_data: module_ctx.target_data,
        return_slot,
        param_offset,
        allocas: Vec::with_capacity(function.locals.len()),
        temps: HashMap::new(),
        param_reloads: HashMap::new(),
        native_call_index: 0,
        allocation_index: 0,
        bounds_trap_block: None,
        bounds_message: module_ctx.bounds_message,
    };

    // All locals are stack slots allocated at the top of the entry block;
    // LLVM's mem2reg promotes them. Entry is empty at this point, so
    // positioning at its end places the allocas before every instruction.
    builder.position_at_end(blocks[arena_index(function.entry)]);
    for (_, local) in function.locals.iter() {
        let ty = basic_ty(context, module_ctx.structs, module_ctx.enums, &local.ty)?;
        emitter.allocas.push(
            builder
                .build_alloca(ty, &local.name)
                .map_err(|e| CodegenError(format!("alloca %{}: {e}", local.name)))?,
        );
    }
    // M9 safepoint poll (milestone9 DESIGN 3.1): every managed function
    // polls at entry, right after the allocas. Verified `@NoGC` bodies
    // deliberately carry neither statepoints nor polls.
    if function.gc_effect == GcEffect::Managed {
        emitter.safepoint_poll()?;
    }

    let headers = loop_headers(function);
    for (block_id, block) in function.blocks.iter() {
        builder.position_at_end(blocks[arena_index(block_id)]);
        // ... and every loop header polls at its top. The header set
        // provably never contains a landing pad (see `loop_headers`);
        // the LandingPad check only restates LLVM's landingpad-first
        // rule at the insertion site.
        if function.gc_effect == GcEffect::Managed
            && headers[arena_index(block_id)]
            && !matches!(
                block.instructions.first(),
                Some(Instruction::LandingPad { .. } | Instruction::CleanupPad { .. })
            )
        {
            emitter.safepoint_poll()?;
        }
        // Invoke is an LLVM terminator even though it
        // are LIR instructions: one must be the last instruction of its
        // block, and the block's LIR terminator must be the redundant
        // `Br` to the invoke's normal target (kept so dumps stay
        // readable); it is not emitted. A LandingPad instruction must be
        // the first of its block (LLVM requires the landingpad first).
        let mut invoke_terminated = false;
        for (index, instruction) in block.instructions.iter().enumerate() {
            let is_invoke = matches!(instruction, Instruction::Invoke { .. });
            if is_invoke && index + 1 != block.instructions.len() {
                return Err(CodegenError(format!(
                    "invoke @{}: must be the last instruction of block {}",
                    function.symbol, block.name
                )));
            }
            if matches!(
                instruction,
                Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
            ) && index != 0
            {
                return Err(CodegenError(format!(
                    "landing pad @{}: must be the first instruction of block {}",
                    function.symbol, block.name
                )));
            }
            emitter.instruction(instruction)?;
            invoke_terminated = is_invoke;
        }
        if invoke_terminated {
            let normal = match block.instructions.last() {
                Some(Instruction::Invoke { normal, .. }) => *normal,
                _ => continue,
            };
            match &block.terminator {
                Terminator::Br(target) if *target == normal => {}
                _ => {
                    return Err(CodegenError(format!(
                        "invoke block @{}:{}: terminator must be `br` to the invoke's normal target",
                        function.symbol, block.name
                    )));
                }
            }
            continue;
        }
        match &block.terminator {
            Terminator::Br(target) => {
                builder
                    .build_unconditional_branch(blocks[arena_index(*target)])
                    .map_err(|e| CodegenError(format!("br @{}: {e}", block.name)))?;
            }
            Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } => {
                let cond = emitter.value(*cond)?.into_int_value();
                builder
                    .build_conditional_branch(
                        cond,
                        blocks[arena_index(*then_block)],
                        blocks[arena_index(*else_block)],
                    )
                    .map_err(|e| CodegenError(format!("cbr @{}: {e}", block.name)))?;
            }
            Terminator::Return { value } => {
                let value = value
                    .map(|value| emitter.value(value))
                    .transpose()
                    .map_err(|e: CodegenError| {
                        CodegenError(format!("ret @{}: {}", function.symbol, e.0))
                    })?;
                if let Some(slot) = emitter.return_slot {
                    let value = value.ok_or_else(|| {
                        CodegenError(format!(
                            "ret @{}: aggregate return has no value",
                            function.symbol
                        ))
                    })?;
                    builder
                        .build_store(slot, value)
                        .map_err(|e| CodegenError(format!("ret slot @{}: {e}", function.symbol)))?;
                    builder
                        .build_return(None)
                        .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
                } else {
                    builder
                        .build_return(
                            value
                                .as_ref()
                                .map(|v| v as &dyn inkwell::values::BasicValue),
                        )
                        .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
                }
            }
            Terminator::Resume { exception } => {
                let exception = emitter.value(*exception)?;
                builder
                    .build_resume(exception)
                    .map_err(|e| CodegenError(format!("resume @{}: {e}", function.symbol)))?;
            }
            Terminator::Unreachable => {
                builder
                    .build_unreachable()
                    .map_err(|e| CodegenError(format!("unreachable @{}: {e}", function.symbol)))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
