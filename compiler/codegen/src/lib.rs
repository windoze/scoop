//! Codegen stage: mechanically translate LIR to LLVM IR, emit
//! TypeDescriptors, expand codegen-stage intrinsics, produce `.o`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.5 and
//! `docs/milestone2/DESIGN.md` section 2.5.
//!
//! All locals become `alloca`s at the top of the entry block; SSA
//! construction is left to LLVM's mem2reg. Temps are SSA values kept in a
//! map. Function signatures come from LIR (`params` / `return_ty`);
//! runtime functions are declared at their call sites with the signature
//! implied by the operands.
//!
//! M9: GC support (milestone9 DESIGN section 3.1). Every heap object
//! carries the 16-byte header `{ td, gc_word }`: class fields start at
//! their natural byte offsets from 16, the boxed payload and array size
//! live at offset 16, array elements at 24, and string constants get a zeroed GC word
//! between the TD and the length. `scoop_rt_alloc` writes both header
//! words, so no allocation site stores the header. Every function is
//! declared with the `statepoint-example` GC strategy and the whole
//! module runs through `rewrite-statepoints-for-gc` before object
//! emission, so the `.o` carries the `__llvm_stackmaps` section the
//! runtime's stack scan reads (impl spec 2.4's statepoint insertion
//! happens here rather than in LIR: it is an instrumentation of the
//! emitted LLVM, and keeping it out of LIR keeps the LIR dumps
//! stable). Safepoint polls — plain calls to the runtime's
//! `scoop_rt_safepoint` — are emitted at every function entry and at
//! the loop-header blocks `loop_headers` finds. Every `HeapStore` /
//! `ArraySet` is followed by the write-barrier card mark
//! (`scoop_gc_card_table[addr >> 9] = 1`, runtime spec 3.6).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicValueEnum, GlobalValue, IntValue, PointerValue, ValueKind};
use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};
use la_arena::{Arena, Idx};
use scoop_lir::{
    BinOp, EnumDef, EnumRepr, Function, Global, GlobalInit, Instruction, LirType, Module, RefScan,
    TempId, Terminator, UnOp, Value,
};

const SCAN_ARRAY: u64 = u64::MAX;
const SCAN_ENUM: u64 = u64::MAX - 1;
const SCAN_SEQUENCE: u64 = u64::MAX - 2;

#[derive(Clone, PartialEq, Eq)]
struct ArrayDescriptor {
    element: LirType,
    element_scan: RefScan,
}

/// Error produced while translating LIR or emitting the object file.
#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodegenError {}

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

/// Translate `module` to LLVM IR and emit an object file at `output`
/// using the host target.
pub fn emit_object(module: &Module, output: &Path) -> Result<(), CodegenError> {
    let machine = host_target_machine()?;
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

/// The host target machine, created up front: array TypeDescriptors
/// take element size/align from the target's data layout (the same
/// layout GEP uses), keeping alloc size and element stride consistent.
fn host_target_machine() -> Result<TargetMachine, CodegenError> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError(format!("failed to initialize native target: {e}")))?;
    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple)
        .map_err(|e| CodegenError(format!("no target for host triple: {e}")))?;
    target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| CodegenError("failed to create host target machine".to_string()))
}

/// Translate `module` to an (unverified) LLVM module: globals,
/// TypeDescriptors, and every function.
fn emit_llvm_module<'ctx>(
    context: &'ctx Context,
    module: &Module,
    machine: &TargetMachine,
) -> Result<LlvmModule<'ctx>, CodegenError> {
    let string_layout = module
        .meta
        .layouts
        .iter()
        .find(|layout| layout.name == "String")
        .ok_or_else(|| CodegenError("LIR meta lacks a layout for String".to_string()))?;

    let llvm = context.create_module("scoop");
    let builder = context.create_builder();
    let target_data = machine.get_target_data();

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
    // @scoop_td_String: type_id 1, no parent. Its vtable carries the
    // Any default slots with `toString` (slot 2) bound to the runtime
    // String identity (M7): String is a reference type and is never
    // boxed, so `Any.toString()` on a String dispatches through this
    // table — core's `print` / `println` rely on it.
    let string_vtable = emit_fn_table(
        context,
        &llvm,
        "scoop_td_String.vtable",
        &[
            "scoop_rt_any_equals".to_string(),
            "scoop_rt_any_hashcode".to_string(),
            "scoop_rt_string_identity".to_string(),
        ],
    )?;
    let string_name = private_c_string(context, &llvm, "scoop_td_String.name", "String");
    let string_td = llvm.add_global(td_ty, None, scoop_lir::STRING_TD_SYMBOL);
    string_td.set_constant(true);
    string_td.set_initializer(&context.const_struct(
        &[
            i64_ty.const_int(1, false).into(),
            i64_ty.const_int(string_layout.size, false).into(),
            i64_ty.const_int(string_layout.align, false).into(),
            ptr_ty.const_null().into(),
            ptr_ty.const_null().into(),
            string_vtable,
            ptr_ty.const_null().into(),
            i64_ty.const_zero().into(),
            string_name.into(),
        ],
        false,
    ));

    // One array TypeDescriptor per distinct (element layout, scan)
    // 2.2): same struct as String's TD, size/align of the *element*
    // layout, type_ids from 100 (1 is String). `size` here is the
    // element size, which is also what `scoop_rt_array_clone` needs.
    // The array wrapper stores the stride and a recursive scan program
    // for one inline element, so aggregates and tagged enums are
    // traced without boxing.
    let array_descriptors = array_descriptors(module);
    let mut array_tds: Vec<GlobalValue> = Vec::with_capacity(array_descriptors.len());
    for (index, descriptor) in array_descriptors.iter().enumerate() {
        let element_ty = basic_ty(context, &module.enums, &descriptor.element)?;
        let stride = target_data.get_abi_size(&element_ty);
        let element_scan = emit_ref_scan(
            context,
            &llvm,
            &format!("scoop_td_array.{index}.element"),
            &descriptor.element_scan,
        );
        let ref_offsets: BasicValueEnum = if let Some(element_scan) = element_scan {
            let words = i64_ty.const_array(&[
                i64_ty.const_int(SCAN_ARRAY, false),
                i64_ty.const_int(stride, false),
                element_scan.const_to_int(i64_ty),
            ]);
            private_const_global(&llvm, &format!("scoop_td_array.{index}.refs"), words.into())
                .into()
        } else {
            ptr_ty.const_null().into()
        };
        let array_name = private_c_string(
            context,
            &llvm,
            &format!("scoop_td_array.{index}.name"),
            &format!("Array<{}>", descriptor.element.dump()),
        );
        let array_td = llvm.add_global(td_ty, None, &format!("scoop_td_array.{index}"));
        array_td.set_constant(true);
        array_td.set_initializer(
            &context.const_struct(
                &[
                    i64_ty.const_int(100 + index as u64, false).into(),
                    i64_ty.const_int(stride, false).into(),
                    i64_ty
                        .const_int(target_data.get_abi_alignment(&element_ty) as u64, false)
                        .into(),
                    ref_offsets,
                    ptr_ty.const_null().into(),
                    ptr_ty.const_null().into(),
                    ptr_ty.const_null().into(),
                    i64_ty.const_zero().into(),
                    array_name.into(),
                ],
                false,
            ),
        );
        array_tds.push(array_td);
    }

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

    // Globals. Indexed by GlobalId (arena iteration is in index order).
    // TypeDescriptor reference stubs (`scoop_td_*`, see the lir-lower
    // module docs) emit no data: the real TD comes from
    // `LirMeta::type_descriptors` (or the built-in String TD above) and
    // `Value::Global` resolves them by symbol at use time. A stub is
    // recognized by its symbol naming one of those TDs, never by its
    // init shape.
    let td_symbols: HashSet<&str> = module
        .meta
        .type_descriptors
        .iter()
        .map(|td| td.symbol.as_str())
        .chain([scoop_lir::STRING_TD_SYMBOL])
        .collect();
    let mut globals: Vec<Option<GlobalValue>> = Vec::with_capacity(module.globals.len());
    for (_, global) in module.globals.iter() {
        if td_symbols.contains(global.symbol.as_str()) {
            globals.push(None);
            continue;
        }
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
        }
    }

    // Two passes: declare every function first so call sites never
    // create shadow extern declarations (a forward call would
    // otherwise declare the symbol as extern, and the later definition
    // would be renamed with a `.N` suffix by LLVM, breaking the link).
    let module_ctx = ModuleCtx {
        enums: &module.enums,
        globals_arena: &module.globals,
        globals: &globals,
        array_descriptors: &array_descriptors,
        array_tds: &array_tds,
        target_data: &target_data,
        bounds_message,
    };
    for function in &module.functions {
        declare_function(context, &llvm, &module.enums, function)?;
    }
    // Meta TypeDescriptors reference module functions (vtable / itable
    // slots), so they are emitted after the declare pass.
    emit_type_descriptors(context, &llvm, td_ty, module)?;
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
        LirType::Ptr => context.ptr_type(AddressSpace::default()).into(),
        LirType::ExceptionRecord => context
            .struct_type(
                &[
                    context.ptr_type(AddressSpace::default()).into(),
                    context.i32_type().into(),
                ],
                false,
            )
            .into(),
        // An array value is a pointer to the array object.
        LirType::Array(_) => context.ptr_type(AddressSpace::default()).into(),
        LirType::Aggregate(elements) => {
            let fields: Vec<BasicTypeEnum> = elements
                .iter()
                .map(|element| basic_ty(context, enums, element))
                .collect::<Result<_, _>>()?;
            context.struct_type(&fields, false).into()
        }
        LirType::Enum(id) => match &enums[*id].repr {
            // Niche optimization: the value is a bare pointer.
            EnumRepr::Niche { .. } => context.ptr_type(AddressSpace::default()).into(),
            EnumRepr::Tagged {
                payload_size,
                payload_align,
                ..
            } => tagged_ty(context, *payload_size, *payload_align).into(),
        },
    })
}

/// Aggregate values cannot be returned directly from a statepoint call:
/// LLVM's statepoint rewrite lowers such results incompletely on the
/// supported native targets. Keep LIR's value-returning contract, but use
/// an explicit caller-provided result slot in the physical LLVM ABI.
fn uses_return_slot(enums: &Arena<EnumDef>, ty: &LirType) -> bool {
    match ty {
        LirType::Aggregate(_) | LirType::ExceptionRecord => true,
        LirType::Enum(id) => matches!(enums[*id].repr, EnumRepr::Tagged { .. }),
        LirType::Void | LirType::I1 | LirType::I64 | LirType::Ptr | LirType::Array(_) => false,
    }
}

/// Byte offset of the payload area inside a tagged enum value: right
/// after the i64 tag, rounded up to the payload alignment.
fn payload_offset(payload_align: u64) -> u64 {
    8u64.max(payload_align)
}

/// `{ i64 tag, [M x i8] payload }` where the payload area starts at
/// `payload_offset(payload_align)` and spans `payload_size` bytes
/// (M covers the alignment padding plus the payload).
fn tagged_ty(
    context: &Context,
    payload_size: u64,
    payload_align: u64,
) -> inkwell::types::StructType<'_> {
    let bytes = (payload_offset(payload_align) - 8) + payload_size;
    context.struct_type(
        &[
            context.i64_type().into(),
            context.i8_type().array_type(bytes as u32).into(),
        ],
        false,
    )
}

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

/// The element layout of an array type.
fn array_element(ty: &LirType) -> Result<&LirType, CodegenError> {
    match ty {
        LirType::Array(element) => Ok(element),
        other => Err(CodegenError(format!(
            "expected an array type, found {}",
            other.dump()
        ))),
    }
}

/// Every distinct array allocation descriptor in first-use order.
/// Array operations other than allocation use the object's own TD;
/// therefore only `ArrayAlloc` needs to select a generated TD.
fn array_descriptors(module: &Module) -> Vec<ArrayDescriptor> {
    let mut out = Vec::new();
    for function in &module.functions {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                if let Instruction::ArrayAlloc {
                    out: temp,
                    element_scan,
                    ..
                } = instruction
                {
                    let element = array_element(&function.temps[*temp].ty)
                        .expect("ArrayAlloc output is an array")
                        .clone();
                    let descriptor = ArrayDescriptor {
                        element,
                        element_scan: element_scan.clone(),
                    };
                    if !out.contains(&descriptor) {
                        out.push(descriptor);
                    }
                }
            }
        }
    }
    out
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

/// First `type_id` assigned to `LirMeta::type_descriptors` entries
/// (runtime spec 2.2): 1 is String, 100+ are the array TDs.
const FIRST_TD_TYPE_ID: u64 = 1000;

/// Emit one recursive GC scan program. Child pointers are stored as
/// u64 constants because the C runtime descriptor is a word stream.
/// `None` has no global and is represented by a null pointer.
fn emit_ref_scan<'ctx>(
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
        RefScan::TaggedEnum {
            tag_offset,
            variants,
        } => {
            let children: Vec<_> = variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    emit_ref_scan(context, llvm, &format!("{name}.variant.{index}"), variant)
                })
                .collect();
            if children.iter().all(Option::is_none) {
                return None;
            }
            let mut words = Vec::with_capacity(children.len() + 3);
            words.push(i64_ty.const_int(SCAN_ENUM, false));
            words.push(i64_ty.const_int(*tag_offset, false));
            words.push(i64_ty.const_int(children.len() as u64, false));
            words.extend(children.into_iter().map(pointer_word));
            words
        }
    };
    let array = i64_ty.const_array(&words);
    Some(private_const_global(llvm, name, array.into()))
}

/// Emit one `ScoopTypeDescriptor` global per `LirMeta::type_descriptors`
/// entry (runtime spec 2.2; milestone6 DESIGN 2.5). Emission order
/// follows the list: `parent` / interface symbols must name globals
/// emitted earlier (or `STRING_TD_SYMBOL`). Runs after the function
/// declare pass so vtable / itable slots resolve to real functions.
fn emit_type_descriptors<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    td_ty: StructType<'ctx>,
    module: &Module,
) -> Result<(), CodegenError> {
    let i64_ty = context.i64_type();
    let ptr = ptr_ty(context);
    // ScoopItableEntry: { ptr interface, ptr slots }.
    let entry_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
    for (index, td) in module.meta.type_descriptors.iter().enumerate() {
        let ref_offsets: BasicValueEnum =
            emit_ref_scan(context, llvm, &format!("{}.refs", td.symbol), &td.scan)
                .map_or_else(|| ptr.const_null().into(), Into::into);
        let parent: BasicValueEnum = match &td.parent {
            Some(symbol) => llvm
                .get_global(symbol)
                .ok_or_else(|| {
                    CodegenError(format!(
                        "TypeDescriptor `{}`: parent `@{}` not emitted yet",
                        td.symbol, symbol
                    ))
                })?
                .as_pointer_value()
                .into(),
            None => ptr.const_null().into(),
        };
        let vtable = emit_fn_table(context, llvm, &format!("{}.vtable", td.symbol), &td.vtable)?;
        let (itables, itable_count): (BasicValueEnum, u64) = if td.itables.is_empty() {
            (ptr.const_null().into(), 0)
        } else {
            let mut entries = Vec::with_capacity(td.itables.len());
            for (record_index, record) in td.itables.iter().enumerate() {
                let interface = llvm
                    .get_global(&record.interface_symbol)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "TypeDescriptor `{}`: interface `@{}` not emitted yet",
                            td.symbol, record.interface_symbol
                        ))
                    })?
                    .as_pointer_value();
                let slots = emit_fn_table(
                    context,
                    llvm,
                    &format!("{}.itables.{record_index}", td.symbol),
                    &record.slots,
                )?;
                entries.push(context.const_struct(&[interface.into(), slots], false));
            }
            let array = entry_ty.const_array(&entries);
            let global =
                private_const_global(llvm, &format!("{}.itables", td.symbol), array.into());
            (global.into(), td.itables.len() as u64)
        };
        let global = llvm.add_global(td_ty, None, &td.symbol);
        let name = private_c_string(context, llvm, &format!("{}.name", td.symbol), &td.name);
        global.set_constant(true);
        global.set_initializer(
            &context.const_struct(
                &[
                    i64_ty
                        .const_int(FIRST_TD_TYPE_ID + index as u64, false)
                        .into(),
                    i64_ty.const_int(td.size, false).into(),
                    i64_ty.const_int(td.align, false).into(),
                    ref_offsets,
                    parent,
                    vtable,
                    itables,
                    i64_ty.const_int(itable_count, false).into(),
                    name.into(),
                ],
                false,
            ),
        );
    }
    Ok(())
}

/// A private constant global holding `value`; returns its address.
fn private_const_global<'ctx>(
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

/// A private NUL-terminated UTF-8 string whose address is stable for
/// the lifetime of the generated module.
fn private_c_string<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    value: &str,
) -> PointerValue<'ctx> {
    private_const_global(
        llvm,
        name,
        context.const_string(value.as_bytes(), true).into(),
    )
}

/// A global `[N x ptr]` of function addresses (a vtable or one itable's
/// slots), or null when the table is empty.
fn emit_fn_table<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    slots: &[String],
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    let ptr = ptr_ty(context);
    if slots.is_empty() {
        return Ok(ptr.const_null().into());
    }
    let mut values = Vec::with_capacity(slots.len());
    for symbol in slots {
        values.push(slot_fn_ptr(context, llvm, symbol)?);
    }
    let array = ptr.const_array(&values);
    Ok(private_const_global(llvm, name, array.into()).into())
}

/// Address of the function a vtable / itable slot points at: a module
/// function (declared in the first pass — user methods and adjust
/// thunks) or one of the Any default methods / the String identity,
/// declared extern here with its runtime signature
/// (runtime/include/scoop_rt.h).
fn slot_fn_ptr<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    symbol: &str,
) -> Result<PointerValue<'ctx>, CodegenError> {
    if let Some(function) = llvm.get_function(symbol) {
        return Ok(function.as_global_value().as_pointer_value());
    }
    let ptr = ptr_ty(context);
    let fn_ty = match symbol {
        "scoop_rt_any_equals" => context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        "scoop_rt_any_hashcode" => context.i64_type().fn_type(&[ptr.into()], false),
        "scoop_rt_any_tostring" | "scoop_rt_string_identity" => ptr.fn_type(&[ptr.into()], false),
        _ => {
            return Err(CodegenError(format!(
                "vtable/itable slot `@{symbol}` is not a function in the module"
            )));
        }
    };
    Ok(llvm
        .add_function(symbol, fn_ty, None)
        .as_global_value()
        .as_pointer_value())
}

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
    enums: &'a Arena<EnumDef>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    /// Distinct array allocation descriptors and their TypeDescriptor
    /// globals (indexed in parallel).
    array_descriptors: &'a [ArrayDescriptor],
    array_tds: &'a [GlobalValue<'ctx>],
    target_data: &'a inkwell::targets::TargetData,
    /// Hidden result pointer for a physically indirect aggregate return.
    return_slot: Option<PointerValue<'ctx>>,
    /// LIR parameters start after the hidden result pointer when present.
    param_offset: u32,
    allocas: Vec<PointerValue<'ctx>>,
    temps: HashMap<TempId, BasicValueEnum<'ctx>>,
    /// Lazily-created shared bounds-check trap block of this function
    /// (one per function, reused by every ArrayGet / ArraySet) and the
    /// module-level "array index out of bounds" message global it
    /// references (`Some` whenever the module uses arrays).
    bounds_trap_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    bounds_message: Option<GlobalValue<'ctx>>,
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
                let ty = basic_ty(context, self.enums, &function.locals[id].ty)?;
                self.builder
                    .build_load(ty, self.allocas[arena_index(id)], &function.locals[id].name)
                    .map_err(|e| CodegenError(format!("load %{}: {e}", function.locals[id].name)))?
            }
            Value::Param(index) => self
                .llvm_function
                .get_nth_param(index + self.param_offset)
                .ok_or_else(|| CodegenError(format!("param {index} out of range")))?,
            Value::Temp(id) => *self.temps.get(&id).ok_or_else(|| {
                CodegenError(format!(
                    "temp t{} used before definition",
                    id.into_raw().into_u32()
                ))
            })?,
            Value::IntConst(value) => context.i64_type().const_int(value as u64, true).into(),
            Value::BoolConst(value) => context.bool_type().const_int(value as u64, false).into(),
            Value::Global(id) => match &self.globals[arena_index(id)] {
                Some(global) => global.as_pointer_value().into(),
                // A TypeDescriptor stub: the TD global (emitted from the
                // meta, or the built-in String TD) is resolved by symbol.
                None => self
                    .llvm
                    .get_global(&self.globals_arena[id].symbol)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "TypeDescriptor global `@{}` was not emitted",
                            self.globals_arena[id].symbol
                        ))
                    })?
                    .as_pointer_value()
                    .into(),
            },
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
                let ty =
                    basic_ty(context, self.enums, &function.temps[*out].ty)?.into_struct_type();
                let name = format!("t{}", out.into_raw().into_u32());
                let mut aggregate = ty.get_undef();
                for (index, element) in elements.iter().enumerate() {
                    aggregate = builder
                        .build_insert_value(aggregate, self.value(*element)?, index as u32, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "insertvalue @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                        .into_struct_value();
                }
                self.temps.insert(*out, aggregate.into());
            }
            Instruction::ExtractValue {
                out,
                aggregate,
                index,
            } => {
                let name = format!("t{}", out.into_raw().into_u32());
                let aggregate = self.value(*aggregate)?.into_struct_value();
                let element = builder
                    .build_extract_value(aggregate, *index, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "extractvalue @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
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
                let field_ty = basic_ty(context, self.enums, &function.temps[*out].ty)?;
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
            Instruction::Store { local, value: v } => {
                let operand = self.value(*v)?;
                builder
                    .build_store(self.allocas[arena_index(*local)], operand)
                    .map_err(|e| {
                        CodegenError(format!("store %{}: {e}", function.locals[*local].name))
                    })?;
            }
            Instruction::Call { out, symbol, args } => {
                // `scoop_rt_box` has a fixed runtime contract and a
                // by-value aggregate payload argument (see `box_call`).
                if symbol == "scoop_rt_box" {
                    self.box_call(out, args)?;
                    return Ok(());
                }
                // Signature from the call site: parameter types from the
                // operands, return type from the result temp (void when
                // there is none). Undefined callees are declared extern.
                let result_slot = self.result_slot(*out, "call_result")?;
                let mut param_tys: Vec<BasicMetadataTypeEnum> = args
                    .iter()
                    .map(|arg| {
                        basic_ty(
                            context,
                            self.enums,
                            &function.value_ty(self.globals_arena, *arg),
                        )
                        .map(Into::into)
                    })
                    .collect::<Result<_, _>>()?;
                if result_slot.is_some() {
                    param_tys.insert(0, ptr_ty(context).into());
                }
                let fn_ty = if symbol == scoop_lir::TRAP_SYMBOL {
                    // Fixed runtime contract: void scoop_rt_trap(ptr).
                    context
                        .void_type()
                        .fn_type(&[ptr_ty(context).into()], false)
                } else {
                    match (out, &result_slot) {
                        (_, Some(_)) => context.void_type().fn_type(&param_tys, false),
                        (Some(temp), None) => {
                            basic_ty(context, self.enums, &function.temps[*temp].ty)?
                                .fn_type(&param_tys, false)
                        }
                        (None, None) => context.void_type().fn_type(&param_tys, false),
                    }
                };
                let callee = self
                    .llvm
                    .get_function(symbol)
                    .unwrap_or_else(|| self.llvm.add_function(symbol, fn_ty, None));
                let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum> = args
                    .iter()
                    .map(|arg| self.value(*arg).map(Into::into))
                    .collect::<Result<_, _>>()?;
                if let Some((_, slot, _)) = result_slot {
                    call_args.insert(0, slot.into());
                }
                let call = builder
                    .build_call(callee, &call_args, "call")
                    .map_err(|e| CodegenError(format!("call @{symbol}: {e}")))?;
                if let Some((temp, slot, ty)) = result_slot {
                    let result = builder
                        .build_load(ty, slot, "call_result")
                        .map_err(|e| CodegenError(format!("load call result @{symbol}: {e}")))?;
                    self.temps.insert(temp, result);
                } else if let Some(temp) = out {
                    match call.try_as_basic_value() {
                        ValueKind::Basic(result) => {
                            self.temps.insert(*temp, result);
                        }
                        ValueKind::Instruction(_) => {
                            return Err(CodegenError(format!(
                                "call @{symbol} produced no value for t{}",
                                temp.into_raw().into_u32()
                            )));
                        }
                    }
                }
            }
            Instruction::CallIndirect {
                out,
                table,
                slot,
                args,
            } => {
                // `table[slot]` (ptr GEP + load), called with the
                // signature implied by the call site (impl spec 2.9).
                let table = self.value(*table)?.into_pointer_value();
                // SAFETY: `table` addresses a function table with at
                // least `slot + 1` slots (LIR contract of CallIndirect).
                let slot_ptr = unsafe {
                    builder.build_gep(
                        ptr_ty(context),
                        table,
                        &[context.i32_type().const_int((*slot).into(), false)],
                        "slot_ptr",
                    )
                }
                .map_err(|e| {
                    CodegenError(format!(
                        "call_indirect @{symbol}: {e}",
                        symbol = function.symbol
                    ))
                })?;
                let fn_ptr = builder
                    .build_load(ptr_ty(context), slot_ptr, "fn_ptr")
                    .map_err(|e| {
                        CodegenError(format!(
                            "call_indirect @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?
                    .into_pointer_value();
                let result_slot = self.result_slot(*out, "indirect_result")?;
                let mut param_tys: Vec<BasicMetadataTypeEnum> = args
                    .iter()
                    .map(|arg| {
                        basic_ty(
                            context,
                            self.enums,
                            &function.value_ty(self.globals_arena, *arg),
                        )
                        .map(Into::into)
                    })
                    .collect::<Result<_, _>>()?;
                if result_slot.is_some() {
                    param_tys.insert(0, ptr_ty(context).into());
                }
                let fn_ty = match (out, &result_slot) {
                    (_, Some(_)) => context.void_type().fn_type(&param_tys, false),
                    (Some(temp), None) => basic_ty(context, self.enums, &function.temps[*temp].ty)?
                        .fn_type(&param_tys, false),
                    (None, None) => context.void_type().fn_type(&param_tys, false),
                };
                let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum> = args
                    .iter()
                    .map(|arg| self.value(*arg).map(Into::into))
                    .collect::<Result<_, _>>()?;
                if let Some((_, slot, _)) = result_slot {
                    call_args.insert(0, slot.into());
                }
                let call = builder
                    .build_indirect_call(fn_ty, fn_ptr, &call_args, "call_indirect")
                    .map_err(|e| {
                        CodegenError(format!(
                            "call_indirect @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                if let Some((temp, slot, ty)) = result_slot {
                    let result = builder
                        .build_load(ty, slot, "indirect_result")
                        .map_err(|e| {
                            CodegenError(format!(
                                "load indirect result @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                    self.temps.insert(temp, result);
                } else if let Some(temp) = out {
                    match call.try_as_basic_value() {
                        ValueKind::Basic(result) => {
                            self.temps.insert(*temp, result);
                        }
                        ValueKind::Instruction(_) => {
                            return Err(CodegenError(format!(
                                "call_indirect produced no value for t{}",
                                temp.into_raw().into_u32()
                            )));
                        }
                    }
                }
            }
            Instruction::Invoke {
                out,
                symbol,
                args,
                normal,
                unwind,
            } => {
                // LLVM `invoke` (M8, runtime spec 5): the call may throw;
                // control continues in `normal` or unwinds into the
                // `unwind` landing pad. Signature from the call site, as
                // for Call; undefined callees are declared extern.
                let result_slot = self.result_slot(*out, "invoke_result")?;
                let mut param_tys: Vec<BasicMetadataTypeEnum> = args
                    .iter()
                    .map(|arg| {
                        basic_ty(
                            context,
                            self.enums,
                            &function.value_ty(self.globals_arena, *arg),
                        )
                        .map(Into::into)
                    })
                    .collect::<Result<_, _>>()?;
                if result_slot.is_some() {
                    param_tys.insert(0, ptr_ty(context).into());
                }
                let fn_ty = match (out, &result_slot) {
                    (_, Some(_)) => context.void_type().fn_type(&param_tys, false),
                    (Some(temp), None) => basic_ty(context, self.enums, &function.temps[*temp].ty)?
                        .fn_type(&param_tys, false),
                    (None, None) => context.void_type().fn_type(&param_tys, false),
                };
                let callee = self
                    .llvm
                    .get_function(symbol)
                    .unwrap_or_else(|| self.llvm.add_function(symbol, fn_ty, None));
                let mut call_args: Vec<BasicValueEnum> = args
                    .iter()
                    .map(|arg| self.value(*arg))
                    .collect::<Result<_, _>>()?;
                if let Some((_, slot, _)) = result_slot {
                    call_args.insert(0, slot.into());
                }
                let invoke = builder
                    .build_invoke(
                        callee,
                        &call_args,
                        self.llvm_blocks[arena_index(*normal)],
                        self.llvm_blocks[arena_index(*unwind)],
                        "invoke",
                    )
                    .map_err(|e| CodegenError(format!("invoke @{symbol}: {e}")))?;
                // The result is defined on the normal edge; it enters the
                // temp map like a call result.
                if let Some((temp, slot, ty)) = result_slot {
                    builder.position_at_end(self.llvm_blocks[arena_index(*normal)]);
                    let result = builder
                        .build_load(ty, slot, "invoke_result")
                        .map_err(|e| CodegenError(format!("load invoke result @{symbol}: {e}")))?;
                    self.temps.insert(temp, result);
                } else if let Some(temp) = out {
                    match invoke.try_as_basic_value() {
                        ValueKind::Basic(result) => {
                            self.temps.insert(*temp, result);
                        }
                        ValueKind::Instruction(_) => {
                            return Err(CodegenError(format!(
                                "invoke @{symbol} produced no value for t{}",
                                temp.into_raw().into_u32()
                            )));
                        }
                    }
                }
            }
            Instruction::InvokeIndirect {
                out,
                table,
                slot,
                args,
                normal,
                unwind,
            } => {
                // Indirect variant: `table[slot]` loaded like
                // CallIndirect, invoked with the call-site signature.
                let table = self.value(*table)?.into_pointer_value();
                // SAFETY: `table` addresses a function table with at
                // least `slot + 1` slots (LIR contract of InvokeIndirect).
                let slot_ptr = unsafe {
                    builder.build_gep(
                        ptr_ty(context),
                        table,
                        &[context.i32_type().const_int((*slot).into(), false)],
                        "slot_ptr",
                    )
                }
                .map_err(|e| {
                    CodegenError(format!(
                        "invoke_indirect @{symbol}: {e}",
                        symbol = function.symbol
                    ))
                })?;
                let fn_ptr = builder
                    .build_load(ptr_ty(context), slot_ptr, "fn_ptr")
                    .map_err(|e| {
                        CodegenError(format!(
                            "invoke_indirect @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?
                    .into_pointer_value();
                let result_slot = self.result_slot(*out, "indirect_invoke_result")?;
                let mut param_tys: Vec<BasicMetadataTypeEnum> = args
                    .iter()
                    .map(|arg| {
                        basic_ty(
                            context,
                            self.enums,
                            &function.value_ty(self.globals_arena, *arg),
                        )
                        .map(Into::into)
                    })
                    .collect::<Result<_, _>>()?;
                if result_slot.is_some() {
                    param_tys.insert(0, ptr_ty(context).into());
                }
                let fn_ty = match (out, &result_slot) {
                    (_, Some(_)) => context.void_type().fn_type(&param_tys, false),
                    (Some(temp), None) => basic_ty(context, self.enums, &function.temps[*temp].ty)?
                        .fn_type(&param_tys, false),
                    (None, None) => context.void_type().fn_type(&param_tys, false),
                };
                let mut call_args: Vec<BasicValueEnum> = args
                    .iter()
                    .map(|arg| self.value(*arg))
                    .collect::<Result<_, _>>()?;
                if let Some((_, slot, _)) = result_slot {
                    call_args.insert(0, slot.into());
                }
                let invoke = builder
                    .build_indirect_invoke(
                        fn_ty,
                        fn_ptr,
                        &call_args,
                        self.llvm_blocks[arena_index(*normal)],
                        self.llvm_blocks[arena_index(*unwind)],
                        "invoke_indirect",
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "invoke_indirect @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                if let Some((temp, slot, ty)) = result_slot {
                    builder.position_at_end(self.llvm_blocks[arena_index(*normal)]);
                    let result = builder
                        .build_load(ty, slot, "indirect_invoke_result")
                        .map_err(|e| {
                            CodegenError(format!(
                                "load indirect invoke result @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                    self.temps.insert(temp, result);
                } else if let Some(temp) = out {
                    match invoke.try_as_basic_value() {
                        ValueKind::Basic(result) => {
                            self.temps.insert(*temp, result);
                        }
                        ValueKind::Instruction(_) => {
                            return Err(CodegenError(format!(
                                "invoke_indirect produced no value for t{}",
                                temp.into_raw().into_u32()
                            )));
                        }
                    }
                }
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
                    basic_ty(context, self.enums, &LirType::ExceptionRecord)?.into_struct_type();
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
                element_scan,
            } => {
                // `{ ptr td, i64 gc_word, i64 size, [n x elem] }`
                // (runtime spec 2.5; the 16-byte header is M9):
                // allocate 24 + n * stride bytes, store the size at
                // offset 16, then store each element in order.
                let element = array_element(&function.temps[*out].ty)?;
                let element_ty = basic_ty(context, self.enums, element)?;
                let stride = self.target_data.get_abi_size(&element_ty);
                let td = self.array_td(element, element_scan)?.as_pointer_value();
                let total = 24 + elements.len() as u64 * stride;
                let alloc = self.runtime_fn(
                    "scoop_rt_alloc",
                    ptr_ty(context)
                        .fn_type(&[ptr_ty(context).into(), context.i64_type().into()], false),
                );
                let array = builder
                    .build_call(
                        alloc,
                        &[td.into(), context.i64_type().const_int(total, false).into()],
                        "array",
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_alloc @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| CodegenError("scoop_rt_alloc returned void".to_string()))?
                    .into_pointer_value();
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
            Instruction::ArrayLen { out, operand } => {
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
            Instruction::ArrayGet { out, array, index } => {
                let array_ty = function.value_ty(self.globals_arena, *array);
                let element = array_element(&array_ty)?;
                let element_ty = basic_ty(context, self.enums, element)?;
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
            } => {
                let array_ty = function.value_ty(self.globals_arena, *array);
                let element = array_element(&array_ty)?;
                let element_ty = basic_ty(context, self.enums, element)?;
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
            Instruction::ArrayClone { out, operand } => {
                // `ptr scoop_rt_array_clone(ptr obj, i64 elem_size)`.
                let element = array_element(&function.temps[*out].ty)?;
                let stride = self
                    .target_data
                    .get_abi_size(&basic_ty(context, self.enums, element)?);
                let clone = self.runtime_fn(
                    scoop_lir::ARRAY_CLONE_SYMBOL,
                    ptr_ty(context)
                        .fn_type(&[ptr_ty(context).into(), context.i64_type().into()], false),
                );
                let name = format!("t{}", out.into_raw().into_u32());
                let result = builder
                    .build_call(
                        clone,
                        &[
                            self.value(*operand)?.into(),
                            context.i64_type().const_int(stride, false).into(),
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
                        payload_size,
                        payload_align,
                    } => {
                        // Tagged values travel through memory: build the
                        // `{ i64 tag, [M x i8] payload }` aggregate in an
                        // entry-block alloca, then load it as a whole.
                        let ty = tagged_ty(context, *payload_size, *payload_align);
                        let slot = self.entry_alloca(ty.into(), "enum_wrap")?;
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
                        let field_tys = &variants[*variant as usize];
                        if !field_tys.is_empty() {
                            let payload_ptr =
                                self.payload_ptr(slot, *payload_align, "payload_ptr")?;
                            for (index, field) in fields.iter().enumerate() {
                                let field_ptr = self.variant_field_ptr(
                                    payload_ptr,
                                    field_tys,
                                    index as u32,
                                    "field_ptr",
                                )?;
                                builder
                                    .build_store(field_ptr, self.value(*field)?)
                                    .map_err(|e| {
                                        CodegenError(format!(
                                            "enum_wrap field @{symbol}: {e}",
                                            symbol = function.symbol
                                        ))
                                    })?;
                            }
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
                        payload_size,
                        payload_align,
                    } => {
                        // Reverse of EnumWrap: spill the aggregate into an
                        // entry-block alloca, then load the field out of
                        // the payload area.
                        let ty = tagged_ty(context, *payload_size, *payload_align);
                        let slot = self.entry_alloca(ty.into(), "enum_field")?;
                        builder.build_store(slot, operand).map_err(|e| {
                            CodegenError(format!(
                                "enum_field @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let payload_ptr = self.payload_ptr(slot, *payload_align, "payload_ptr")?;
                        let field_tys = &variants[*variant as usize];
                        let field_ptr =
                            self.variant_field_ptr(payload_ptr, field_tys, *index, "field_ptr")?;
                        let field_ty = basic_ty(context, self.enums, &field_tys[*index as usize])?;
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

    /// Allocate the hidden result slot for an aggregate-producing call.
    fn result_slot(
        &self,
        out: Option<TempId>,
        name: &str,
    ) -> Result<Option<(TempId, PointerValue<'ctx>, BasicTypeEnum<'ctx>)>, CodegenError> {
        let Some(temp) = out else {
            return Ok(None);
        };
        let ty = &self.function.temps[temp].ty;
        if !uses_return_slot(self.enums, ty) {
            return Ok(None);
        }
        let ty = basic_ty(self.context, self.enums, ty)?;
        let slot = self.entry_alloca(ty, name)?;
        Ok(Some((temp, slot, ty)))
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

    /// `ptr scoop_rt_box(ptr td, ptr payload, i64 size)` (runtime spec
    /// 2.3). lir-lower passes the payload by value (an aggregate for a
    /// value type); it is materialized behind a stack pointer here
    /// (lir-lower module docs, the "临时 alloca 取地址" contract).
    fn box_call(&mut self, out: &Option<TempId>, args: &[Value]) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        let [td, payload, size] = args else {
            return Err(CodegenError(format!(
                "scoop_rt_box @{symbol}: expected 3 arguments, got {count}",
                symbol = function.symbol,
                count = args.len()
            )));
        };
        let payload_ty = basic_ty(
            context,
            self.enums,
            &function.value_ty(self.globals_arena, *payload),
        )?;
        let slot = self.entry_alloca(payload_ty, "box_payload")?;
        builder
            .build_store(slot, self.value(*payload)?)
            .map_err(|e| {
                CodegenError(format!(
                    "box payload @{symbol}: {e}",
                    symbol = function.symbol
                ))
            })?;
        let box_fn = self.runtime_fn(
            "scoop_rt_box",
            ptr_ty(context).fn_type(
                &[
                    ptr_ty(context).into(),
                    ptr_ty(context).into(),
                    context.i64_type().into(),
                ],
                false,
            ),
        );
        let call = builder
            .build_call(
                box_fn,
                &[
                    self.value(*td)?.into(),
                    slot.into(),
                    self.value(*size)?.into(),
                ],
                "box",
            )
            .map_err(|e| CodegenError(format!("box @{symbol}: {e}", symbol = function.symbol)))?;
        if let Some(temp) = out {
            match call.try_as_basic_value() {
                ValueKind::Basic(result) => {
                    self.temps.insert(*temp, result);
                }
                ValueKind::Instruction(_) => {
                    return Err(CodegenError(format!(
                        "scoop_rt_box produced no value for t{}",
                        temp.into_raw().into_u32()
                    )));
                }
            }
        }
        Ok(())
    }

    /// The array TypeDescriptor global for one allocation's element
    /// layout and scan program.
    fn array_td(
        &self,
        element: &LirType,
        element_scan: &RefScan,
    ) -> Result<GlobalValue<'ctx>, CodegenError> {
        self.array_descriptors
            .iter()
            .position(|candidate| {
                candidate.element == *element && candidate.element_scan == *element_scan
            })
            .map(|index| self.array_tds[index])
            .ok_or_else(|| {
                CodegenError(format!(
                    "no array TypeDescriptor for element type {}",
                    element.dump()
                ))
            })
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
    /// runtime spec 3.6): after a heap store, mark the card covering
    /// the stored-to address — `scoop_gc_card_table[addr >> 9] = 1`.
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
            .build_store(card_ptr, context.i8_type().const_int(1, false))
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
    /// area starts right after the 16-byte header (M9) + size field
    /// (24 bytes).
    fn element_ptr(
        &self,
        array: PointerValue<'ctx>,
        element_ty: BasicTypeEnum<'ctx>,
        index: IntValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let base = self.byte_gep(array, 24, "elements")?;
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

    /// Address of the payload area of a tagged enum value in memory.
    /// Pointers are opaque, so a byte-wise i8 GEP needs no bitcast.
    fn payload_ptr(
        &self,
        slot: PointerValue<'ctx>,
        payload_align: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: the payload area starts at `payload_offset` bytes into
        // the `{ i64, [M x i8] }` object `slot` points to.
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                slot,
                &[self
                    .context
                    .i32_type()
                    .const_int(payload_offset(payload_align), false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "payload gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }

    /// Address of field `index` of a variant payload, viewing the
    /// payload area as the variant's field struct.
    fn variant_field_ptr(
        &self,
        payload_ptr: PointerValue<'ctx>,
        field_tys: &[LirType],
        index: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let fields: Vec<BasicTypeEnum> = field_tys
            .iter()
            .map(|ty| basic_ty(self.context, self.enums, ty))
            .collect::<Result<_, _>>()?;
        let variant_ty = self.context.struct_type(&fields, false);
        // SAFETY: `payload_ptr` addresses a payload area at least as
        // large as the variant's field struct; indexes 0, `index`
        // address the field within it.
        unsafe {
            self.builder.build_gep(
                variant_ty,
                payload_ptr,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_int(index as u64, false),
                ],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "field gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }
}

/// Translate one LIR function. Signature (parameters and return type)
/// comes from LIR; parameters are SSA values (`Value::Param`).
fn fn_type_of<'ctx>(
    context: &'ctx Context,
    enums: &Arena<EnumDef>,
    function: &Function,
) -> Result<inkwell::types::FunctionType<'ctx>, CodegenError> {
    let mut param_tys: Vec<BasicMetadataTypeEnum> = function
        .params
        .iter()
        .map(|ty| basic_ty(context, enums, ty).map(Into::into))
        .collect::<Result<_, _>>()?;
    if uses_return_slot(enums, &function.return_ty) {
        param_tys.insert(0, ptr_ty(context).into());
        return Ok(context.void_type().fn_type(&param_tys, false));
    }
    Ok(match &function.return_ty {
        LirType::Void => context.void_type().fn_type(&param_tys, false),
        return_ty => basic_ty(context, enums, return_ty)?.fn_type(&param_tys, false),
    })
}

/// Declare a function with its final symbol and signature. Every
/// function carries the GC strategy (M9, milestone9 DESIGN 5.5):
/// `rewrite-statepoints-for-gc` rewrites the body's call sites into
/// statepoints and LLVM emits their stackmaps. Only module functions
/// get it — the runtime declarations created at call sites are not
/// managed code.
fn declare_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    enums: &Arena<EnumDef>,
    function: &Function,
) -> Result<(), CodegenError> {
    let fn_ty = fn_type_of(context, enums, function)?;
    llvm.add_function(&function.symbol, fn_ty, None)
        .set_gc(GC_STRATEGY);
    Ok(())
}

/// Module-level data function emission needs, bundled to keep
/// signatures small.
struct ModuleCtx<'a, 'ctx> {
    enums: &'a Arena<EnumDef>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    array_descriptors: &'a [ArrayDescriptor],
    array_tds: &'a [GlobalValue<'ctx>],
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
        // Invoke / InvokeIndirect are terminator-like: the block's own
        // terminator restates the normal successor (counted above), so
        // only the unwind edge is extra.
        if let Some(
            Instruction::Invoke { unwind, .. } | Instruction::InvokeIndirect { unwind, .. },
        ) = block.instructions.last()
        {
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
        enums: module_ctx.enums,
        globals_arena: module_ctx.globals_arena,
        globals: module_ctx.globals,
        array_descriptors: module_ctx.array_descriptors,
        array_tds: module_ctx.array_tds,
        target_data: module_ctx.target_data,
        return_slot,
        param_offset,
        allocas: Vec::with_capacity(function.locals.len()),
        temps: HashMap::new(),
        bounds_trap_block: None,
        bounds_message: module_ctx.bounds_message,
    };

    // All locals are stack slots allocated at the top of the entry block;
    // LLVM's mem2reg promotes them. Entry is empty at this point, so
    // positioning at its end places the allocas before every instruction.
    builder.position_at_end(blocks[arena_index(function.entry)]);
    for (_, local) in function.locals.iter() {
        let ty = basic_ty(context, module_ctx.enums, &local.ty)?;
        emitter.allocas.push(
            builder
                .build_alloca(ty, &local.name)
                .map_err(|e| CodegenError(format!("alloca %{}: {e}", local.name)))?,
        );
    }
    // M9 safepoint poll (milestone9 DESIGN 3.1): every function polls
    // at entry, right after the allocas.
    emitter.safepoint_poll()?;

    let headers = loop_headers(function);
    for (block_id, block) in function.blocks.iter() {
        builder.position_at_end(blocks[arena_index(block_id)]);
        // ... and every loop header polls at its top. The header set
        // provably never contains a landing pad (see `loop_headers`);
        // the LandingPad check only restates LLVM's landingpad-first
        // rule at the insertion site.
        if headers[arena_index(block_id)]
            && !matches!(
                block.instructions.first(),
                Some(Instruction::LandingPad { .. } | Instruction::CleanupPad { .. })
            )
        {
            emitter.safepoint_poll()?;
        }
        // Invoke / InvokeIndirect are LLVM terminators even though they
        // are LIR instructions: one must be the last instruction of its
        // block, and the block's LIR terminator must be the redundant
        // `Br` to the invoke's normal target (kept so dumps stay
        // readable); it is not emitted. A LandingPad instruction must be
        // the first of its block (LLVM requires the landingpad first).
        let mut invoke_terminated = false;
        for (index, instruction) in block.instructions.iter().enumerate() {
            let is_invoke = matches!(
                instruction,
                Instruction::Invoke { .. } | Instruction::InvokeIndirect { .. }
            );
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
                Some(
                    Instruction::Invoke { normal, .. } | Instruction::InvokeIndirect { normal, .. },
                ) => *normal,
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
mod tests {
    use la_arena::Arena;
    use scoop_lir::{
        BasicBlock, EnumDef, EnumRepr, Global, GlobalInit, ItableRecord, Layout, LayoutKind,
        LirMeta, Local, Temp, TypeDescriptor, VariantLayout,
    };

    use super::*;

    /// An M2-shaped module: string constants, a user function exercising
    /// alloca/store/load, arithmetic, branches, aggregates and
    /// extractvalue, plus calls into the new runtime functions.
    fn values_module() -> Module {
        let mut globals = Arena::default();
        let hello = globals.alloc(Global {
            symbol: "scoop.string.0".to_string(),
            init: GlobalInit::StringConst("hello, ".to_string()),
        });
        let world = globals.alloc(Global {
            symbol: "scoop.string.1".to_string(),
            init: GlobalInit::StringConst("world".to_string()),
        });

        let mut locals = Arena::default();
        let n = locals.alloc(Local {
            name: "n".to_string(),
            ty: LirType::I64,
        });
        let point = locals.alloc(Local {
            name: "p".to_string(),
            ty: LirType::Aggregate(vec![LirType::I64, LirType::I64]),
        });
        let unit = locals.alloc(Local {
            name: "u".to_string(),
            ty: LirType::Aggregate(vec![]),
        });

        let mut temps = Arena::default();
        let temp = |temps: &mut Arena<Temp>, ty: LirType| temps.alloc(Temp { ty });
        // t0 = 1 + 2
        let t0 = temp(&mut temps, LirType::I64);
        // t1 = {40, 2} (Point)
        let t1 = temp(
            &mut temps,
            LirType::Aggregate(vec![LirType::I64, LirType::I64]),
        );
        // t2 = p.x
        let t2 = temp(&mut temps, LirType::I64);
        // t3 = t2 < 100
        let t3 = temp(&mut temps, LirType::I1);
        // t4 = -t2
        let t4 = temp(&mut temps, LirType::I64);
        // t5 = !true
        let t5 = temp(&mut temps, LirType::I1);
        // t6 = concat(hello, world)
        let t6 = temp(&mut temps, LirType::Ptr);
        // t7 = ()
        let t7 = temp(&mut temps, LirType::Aggregate(vec![]));

        // Allocate the four blocks first so terminators can reference
        // them, then fill in their bodies.
        let mut blocks = Arena::default();
        let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
            blocks.alloc(BasicBlock {
                name: name.to_string(),
                instructions: vec![],
                terminator: Terminator::Return { value: None },
            })
        };
        let entry = placeholder(&mut blocks, "entry");
        let then_block = placeholder(&mut blocks, "then");
        let else_block = placeholder(&mut blocks, "else");
        let end = placeholder(&mut blocks, "end");

        blocks[entry] = BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::BinOp {
                    out: t0,
                    op: BinOp::Add,
                    lhs: Value::IntConst(1),
                    rhs: Value::IntConst(2),
                },
                Instruction::Store {
                    local: n,
                    value: Value::Temp(t0),
                },
                Instruction::MakeAggregate {
                    out: t1,
                    elements: vec![Value::IntConst(40), Value::IntConst(2)],
                },
                Instruction::Store {
                    local: point,
                    value: Value::Temp(t1),
                },
                Instruction::ExtractValue {
                    out: t2,
                    aggregate: Value::Local(point),
                    index: 0,
                },
                Instruction::BinOp {
                    out: t3,
                    op: BinOp::Lt,
                    lhs: Value::Temp(t2),
                    rhs: Value::IntConst(100),
                },
                Instruction::Call {
                    out: Some(t6),
                    symbol: "scoop_rt_string_concat".to_string(),
                    args: vec![Value::Global(hello), Value::Global(world)],
                },
            ],
            terminator: Terminator::CondBr {
                cond: Value::Temp(t3),
                then_block,
                else_block,
            },
        };
        blocks[then_block] = BasicBlock {
            name: "then".to_string(),
            instructions: vec![
                Instruction::UnaryOp {
                    out: t4,
                    op: UnOp::Neg,
                    operand: Value::Temp(t2),
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_int".to_string(),
                    args: vec![Value::Temp(t4)],
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println".to_string(),
                    args: vec![Value::Temp(t6)],
                },
            ],
            terminator: Terminator::Br(end),
        };
        blocks[else_block] = BasicBlock {
            name: "else".to_string(),
            instructions: vec![
                Instruction::UnaryOp {
                    out: t5,
                    op: UnOp::Not,
                    operand: Value::BoolConst(true),
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_boolean".to_string(),
                    args: vec![Value::Temp(t5)],
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_int".to_string(),
                    args: vec![Value::Local(n)],
                },
            ],
            terminator: Terminator::Br(end),
        };
        blocks[end] = BasicBlock {
            name: "end".to_string(),
            instructions: vec![
                Instruction::MakeAggregate {
                    out: t7,
                    elements: vec![],
                },
                Instruction::Store {
                    local: unit,
                    value: Value::Temp(t7),
                },
            ],
            terminator: Terminator::Return { value: None },
        };

        Module {
            globals,
            enums: Arena::default(),
            functions: vec![Function {
                symbol: "scoop_main".to_string(),
                params: vec![],
                return_ty: LirType::Void,
                locals,
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn emits_non_empty_object_file() {
        let module = values_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }

    /// An M4-shaped module: a tagged enum with three variants (0/1/2
    /// fields of different types) and a niche enum (two variants,
    /// `Option<String>`-style), both exercised through EnumWrap /
    /// EnumTag / EnumField.
    fn enum_module() -> Module {
        let mut globals = Arena::default();
        let trap_message = globals.alloc(Global {
            symbol: "scoop.trap.0".to_string(),
            init: GlobalInit::CString("unwrap on None".to_string()),
        });
        let mut enums = Arena::default();
        // enum Shape { Dot, Circle(Int), Rect(Int, String) } — tagged
        // `{ i64, [16 x i8] }` (payload `{ i64, ptr }` = 16 bytes).
        let shape = enums.alloc(EnumDef {
            name: "Shape".to_string(),
            repr: EnumRepr::Tagged {
                variants: vec![vec![], vec![LirType::I64], vec![LirType::I64, LirType::Ptr]],
                payload_size: 16,
                payload_align: 8,
            },
        });
        // enum Option<String> { None, Some(String) } — niche pointer.
        let option = enums.alloc(EnumDef {
            name: "Option<String>".to_string(),
            repr: EnumRepr::Niche { payload_variant: 1 },
        });
        let shape_ty = LirType::Enum(shape);
        let option_ty = LirType::Enum(option);

        // fun @scoop.tagged(s: Shape, p: ptr) -> i64: all three enum
        // instructions on the tagged representation, including a local
        // of enum type (alloca + store + load).
        let mut tagged_locals = Arena::default();
        let s2 = tagged_locals.alloc(Local {
            name: "s2".to_string(),
            ty: shape_ty.clone(),
        });
        let mut tagged_temps = Arena::default();
        let t0 = tagged_temps.alloc(Temp {
            ty: shape_ty.clone(),
        }); // enum_wrap v0 ()
        let t1 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag t0
        let t2 = tagged_temps.alloc(Temp {
            ty: shape_ty.clone(),
        }); // enum_wrap v1 (7)
        let t3 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_field v1 f0 t2
        let t4 = tagged_temps.alloc(Temp {
            ty: shape_ty.clone(),
        }); // enum_wrap v2 (t3, p)
        let t5 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag s (param)
        let t6 = tagged_temps.alloc(Temp { ty: LirType::Ptr }); // enum_field v2 f1 s2 (local)
        let t7 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_field v2 f0 t4
        let t8 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t1 + t3
        let t9 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t5 + t7
        let t10 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t8 + t9
        let mut tagged_blocks = Arena::default();
        let tagged_entry = tagged_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::EnumWrap {
                    out: t0,
                    enum_id: shape,
                    variant: 0,
                    fields: vec![],
                },
                Instruction::EnumTag {
                    out: t1,
                    enum_id: shape,
                    operand: Value::Temp(t0),
                },
                Instruction::EnumWrap {
                    out: t2,
                    enum_id: shape,
                    variant: 1,
                    fields: vec![Value::IntConst(7)],
                },
                Instruction::EnumField {
                    out: t3,
                    enum_id: shape,
                    variant: 1,
                    index: 0,
                    operand: Value::Temp(t2),
                },
                Instruction::EnumWrap {
                    out: t4,
                    enum_id: shape,
                    variant: 2,
                    fields: vec![Value::Temp(t3), Value::Param(1)],
                },
                Instruction::EnumTag {
                    out: t5,
                    enum_id: shape,
                    operand: Value::Param(0),
                },
                Instruction::Store {
                    local: s2,
                    value: Value::Temp(t4),
                },
                Instruction::EnumField {
                    out: t6,
                    enum_id: shape,
                    variant: 2,
                    index: 1,
                    operand: Value::Local(s2),
                },
                Instruction::EnumField {
                    out: t7,
                    enum_id: shape,
                    variant: 2,
                    index: 0,
                    operand: Value::Temp(t4),
                },
                Instruction::BinOp {
                    out: t8,
                    op: BinOp::Add,
                    lhs: Value::Temp(t1),
                    rhs: Value::Temp(t3),
                },
                Instruction::BinOp {
                    out: t9,
                    op: BinOp::Add,
                    lhs: Value::Temp(t5),
                    rhs: Value::Temp(t7),
                },
                Instruction::BinOp {
                    out: t10,
                    op: BinOp::Add,
                    lhs: Value::Temp(t8),
                    rhs: Value::Temp(t9),
                },
                // Use the extracted pointer so nothing is dead.
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println".to_string(),
                    args: vec![Value::Temp(t6)],
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(t10)),
            },
        });
        let tagged = Function {
            symbol: "scoop.tagged".to_string(),
            params: vec![shape_ty.clone(), LirType::Ptr],
            return_ty: LirType::I64,
            locals: tagged_locals,
            temps: tagged_temps,
            blocks: tagged_blocks,
            entry: tagged_entry,
        };

        // fun @scoop.niche(o: Option<String>) -> i64: all three enum
        // instructions on the niche representation (null ↔ variant 0).
        let mut niche_locals = Arena::default();
        let o2 = niche_locals.alloc(Local {
            name: "o2".to_string(),
            ty: option_ty.clone(),
        });
        let mut niche_temps = Arena::default();
        let n0 = niche_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag o (param)
        let n1 = niche_temps.alloc(Temp { ty: LirType::Ptr }); // enum_field v1 f0 o
        let n2 = niche_temps.alloc(Temp {
            ty: option_ty.clone(),
        }); // enum_wrap v1 (n1)
        let n3 = niche_temps.alloc(Temp {
            ty: option_ty.clone(),
        }); // enum_wrap v0 () → null
        let n4 = niche_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag n3
        let n5 = niche_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag o2 (local)
        let n6 = niche_temps.alloc(Temp { ty: LirType::I64 }); // n0 + n4
        let n7 = niche_temps.alloc(Temp { ty: LirType::I64 }); // n6 + n5
        let mut niche_blocks = Arena::default();
        let niche_entry = niche_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::EnumTag {
                    out: n0,
                    enum_id: option,
                    operand: Value::Param(0),
                },
                Instruction::EnumField {
                    out: n1,
                    enum_id: option,
                    variant: 1,
                    index: 0,
                    operand: Value::Param(0),
                },
                Instruction::EnumWrap {
                    out: n2,
                    enum_id: option,
                    variant: 1,
                    fields: vec![Value::Temp(n1)],
                },
                Instruction::Store {
                    local: o2,
                    value: Value::Temp(n2),
                },
                Instruction::EnumWrap {
                    out: n3,
                    enum_id: option,
                    variant: 0,
                    fields: vec![],
                },
                Instruction::EnumTag {
                    out: n4,
                    enum_id: option,
                    operand: Value::Temp(n3),
                },
                Instruction::EnumTag {
                    out: n5,
                    enum_id: option,
                    operand: Value::Local(o2),
                },
                Instruction::BinOp {
                    out: n6,
                    op: BinOp::Add,
                    lhs: Value::Temp(n0),
                    rhs: Value::Temp(n4),
                },
                Instruction::BinOp {
                    out: n7,
                    op: BinOp::Add,
                    lhs: Value::Temp(n6),
                    rhs: Value::Temp(n5),
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(n7)),
            },
        });
        let niche = Function {
            symbol: "scoop.niche".to_string(),
            params: vec![option_ty.clone()],
            return_ty: LirType::I64,
            locals: niche_locals,
            temps: niche_temps,
            blocks: niche_blocks,
            entry: niche_entry,
        };

        // fun @scoop.trap_on_none(): the `!!`-on-None path — trap call
        // (noreturn) followed by unreachable.
        let mut trap_blocks = Arena::default();
        let trap_entry = trap_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::Call {
                out: None,
                symbol: scoop_lir::TRAP_SYMBOL.to_string(),
                args: vec![Value::Global(trap_message)],
            }],
            terminator: Terminator::Unreachable,
        });
        let trap_on_none = Function {
            symbol: "scoop.trap_on_none".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks: trap_blocks,
            entry: trap_entry,
        };

        // A tagged enum crossing a managed call boundary. The LLVM
        // statepoint pass cannot lower aggregate returns directly, so
        // codegen must use its hidden result-slot ABI here.
        let mut produce_temps = Arena::default();
        let produced = produce_temps.alloc(Temp {
            ty: shape_ty.clone(),
        });
        let mut produce_blocks = Arena::default();
        let produce_entry = produce_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::EnumWrap {
                out: produced,
                enum_id: shape,
                variant: 1,
                fields: vec![Value::IntConst(9)],
            }],
            terminator: Terminator::Return {
                value: Some(Value::Temp(produced)),
            },
        });
        let produce = Function {
            symbol: "scoop.produce_shape".to_string(),
            params: vec![],
            return_ty: shape_ty.clone(),
            locals: Arena::default(),
            temps: produce_temps,
            blocks: produce_blocks,
            entry: produce_entry,
        };
        let mut consume_temps = Arena::default();
        let received = consume_temps.alloc(Temp {
            ty: shape_ty.clone(),
        });
        let tag = consume_temps.alloc(Temp { ty: LirType::I64 });
        let mut consume_blocks = Arena::default();
        let consume_entry = consume_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::Call {
                    out: Some(received),
                    symbol: "scoop.produce_shape".to_string(),
                    args: vec![],
                },
                Instruction::EnumTag {
                    out: tag,
                    enum_id: shape,
                    operand: Value::Temp(received),
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(tag)),
            },
        });
        let consume = Function {
            symbol: "scoop.consume_shape".to_string(),
            params: vec![],
            return_ty: LirType::I64,
            locals: Arena::default(),
            temps: consume_temps,
            blocks: consume_blocks,
            entry: consume_entry,
        };
        let mut indirect_temps = Arena::default();
        let indirect_received = indirect_temps.alloc(Temp {
            ty: shape_ty.clone(),
        });
        let indirect_tag = indirect_temps.alloc(Temp { ty: LirType::I64 });
        let mut indirect_blocks = Arena::default();
        let indirect_entry = indirect_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::CallIndirect {
                    out: Some(indirect_received),
                    table: Value::Param(0),
                    slot: 0,
                    args: vec![],
                },
                Instruction::EnumTag {
                    out: indirect_tag,
                    enum_id: shape,
                    operand: Value::Temp(indirect_received),
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(indirect_tag)),
            },
        });
        let consume_indirect = Function {
            symbol: "scoop.consume_shape_indirect".to_string(),
            params: vec![LirType::Ptr],
            return_ty: LirType::I64,
            locals: Arena::default(),
            temps: indirect_temps,
            blocks: indirect_blocks,
            entry: indirect_entry,
        };

        Module {
            globals,
            enums,
            functions: vec![
                tagged,
                niche,
                trap_on_none,
                produce,
                consume,
                consume_indirect,
            ],
            entry_symbol: "scoop.tagged".to_string(),
            meta: LirMeta {
                layouts: vec![
                    Layout {
                        name: "String".to_string(),
                        size: 24,
                        align: 8,
                        kind: LayoutKind::Plain {
                            scan: RefScan::None,
                        },
                    },
                    Layout {
                        name: "Shape".to_string(),
                        size: 24,
                        align: 8,
                        kind: LayoutKind::Enum {
                            variants: vec![
                                VariantLayout {
                                    scan: RefScan::None,
                                },
                                VariantLayout {
                                    scan: RefScan::None,
                                },
                                VariantLayout {
                                    scan: RefScan::References(vec![8]),
                                },
                            ],
                        },
                    },
                    Layout {
                        name: "Option<String>".to_string(),
                        size: 8,
                        align: 8,
                        kind: LayoutKind::Enum {
                            variants: vec![
                                VariantLayout {
                                    scan: RefScan::None,
                                },
                                VariantLayout {
                                    scan: RefScan::References(vec![0]),
                                },
                            ],
                        },
                    },
                ],
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn emits_m4_enums() {
        let module = enum_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_m4_test_{}.o", std::process::id()));
        // `emit_object` verifies the LLVM module before writing, so a
        // successful return means `module.verify()` passed.
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }

    /// An M5-shaped module: ArrayAlloc with i64 and aggregate (Point)
    /// elements, ArrayLen, bounds-checked ArrayGet / ArraySet, and
    /// ArrayClone on both element shapes.
    fn arrays_module() -> Module {
        let point = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
        let int_array = LirType::Array(Box::new(LirType::I64));
        let point_array = LirType::Array(Box::new(point.clone()));

        let mut locals = Arena::default();
        let numbers = locals.alloc(Local {
            name: "numbers".to_string(),
            ty: int_array.clone(),
        });

        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp {
            ty: int_array.clone(),
        }); // array_alloc (1, 2, 3)
        let t1 = temps.alloc(Temp { ty: LirType::I64 }); // array_len t0
        let t2 = temps.alloc(Temp { ty: LirType::I64 }); // array_get t0[1]
        let t3 = temps.alloc(Temp {
            ty: int_array.clone(),
        }); // array_clone t0
        let t4 = temps.alloc(Temp { ty: point.clone() }); // aggregate (t2, t1)
        let t5 = temps.alloc(Temp {
            ty: point_array.clone(),
        }); // array_alloc (t4, t4)
        let t6 = temps.alloc(Temp { ty: point.clone() }); // array_get t5[1]
        let t7 = temps.alloc(Temp { ty: LirType::I64 }); // extract t6.1
        let t8 = temps.alloc(Temp {
            ty: point_array.clone(),
        }); // array_clone t5
        let t9 = temps.alloc(Temp { ty: LirType::I64 }); // t1 + t7

        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::ArrayAlloc {
                    out: t0,
                    elements: vec![Value::IntConst(1), Value::IntConst(2), Value::IntConst(3)],
                    element_scan: RefScan::None,
                },
                Instruction::Store {
                    local: numbers,
                    value: Value::Temp(t0),
                },
                Instruction::ArrayLen {
                    out: t1,
                    operand: Value::Local(numbers),
                },
                Instruction::ArrayGet {
                    out: t2,
                    array: Value::Local(numbers),
                    index: Value::IntConst(1),
                },
                Instruction::ArraySet {
                    array: Value::Local(numbers),
                    index: Value::IntConst(0),
                    value: Value::Temp(t2),
                },
                Instruction::ArrayClone {
                    out: t3,
                    operand: Value::Local(numbers),
                },
                Instruction::MakeAggregate {
                    out: t4,
                    elements: vec![Value::Temp(t2), Value::Temp(t1)],
                },
                Instruction::ArrayAlloc {
                    out: t5,
                    elements: vec![Value::Temp(t4), Value::Temp(t4)],
                    element_scan: RefScan::None,
                },
                Instruction::ArrayGet {
                    out: t6,
                    array: Value::Temp(t5),
                    index: Value::IntConst(0),
                },
                Instruction::ExtractValue {
                    out: t7,
                    aggregate: Value::Temp(t6),
                    index: 1,
                },
                Instruction::ArraySet {
                    array: Value::Temp(t5),
                    index: Value::Temp(t1),
                    value: Value::Temp(t6),
                },
                Instruction::ArrayClone {
                    out: t8,
                    operand: Value::Temp(t5),
                },
                Instruction::BinOp {
                    out: t9,
                    op: BinOp::Add,
                    lhs: Value::Temp(t1),
                    rhs: Value::Temp(t7),
                },
                // Use the clones and the sum so nothing is dead.
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_int".to_string(),
                    args: vec![Value::Temp(t9)],
                },
                Instruction::ArraySet {
                    array: Value::Temp(t3),
                    index: Value::IntConst(0),
                    value: Value::IntConst(0),
                },
                Instruction::ArraySet {
                    array: Value::Temp(t8),
                    index: Value::IntConst(0),
                    value: Value::Temp(t6),
                },
            ],
            terminator: Terminator::Return { value: None },
        });

        Module {
            globals: Arena::default(),
            enums: Arena::default(),
            functions: vec![Function {
                symbol: "scoop_main".to_string(),
                params: vec![],
                return_ty: LirType::Void,
                locals,
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![
                    Layout {
                        name: "String".to_string(),
                        size: 24,
                        align: 8,
                        kind: LayoutKind::Plain {
                            scan: RefScan::None,
                        },
                    },
                    Layout {
                        name: "[i64]".to_string(),
                        size: 8,
                        align: 8,
                        kind: LayoutKind::Array {
                            element_scan: RefScan::None,
                        },
                    },
                    Layout {
                        name: "[{i64, i64}]".to_string(),
                        size: 16,
                        align: 8,
                        kind: LayoutKind::Array {
                            element_scan: RefScan::None,
                        },
                    },
                ],
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn emits_m5_arrays() {
        let module = arrays_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_m5_test_{}.o", std::process::id()));
        // `emit_object` verifies the LLVM module before writing, so a
        // successful return means `module.verify()` passed.
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }

    /// An M6-shaped module: class TypeDescriptors (parent chain, vtable
    /// with the three Any default slots plus a user method, one itable)
    /// and indirect calls through a table pointer (vtable / itable
    /// dispatch shape, impl spec 2.9).
    fn classes_module() -> Module {
        // `fn describe(this: ptr) -> ptr` shared shape: returns `this`.
        let describe = |symbol: &str| {
            let mut blocks = Arena::default();
            let entry = blocks.alloc(BasicBlock {
                name: "entry".to_string(),
                instructions: vec![],
                terminator: Terminator::Return {
                    value: Some(Value::Param(0)),
                },
            });
            Function {
                symbol: symbol.to_string(),
                params: vec![LirType::Ptr],
                return_ty: LirType::Ptr,
                locals: Arena::default(),
                temps: Arena::default(),
                blocks,
                entry,
            }
        };

        // fun @scoop_main(table: ptr, obj: ptr) -> ptr:
        //   t0 = call_indirect table[0](obj) : ptr   (result)
        //   call_indirect table[1](obj)              (void)
        //   ret t0
        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: LirType::Ptr });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::CallIndirect {
                    out: Some(t0),
                    table: Value::Param(0),
                    slot: 0,
                    args: vec![Value::Param(1)],
                },
                Instruction::CallIndirect {
                    out: None,
                    table: Value::Param(0),
                    slot: 1,
                    args: vec![Value::Param(1)],
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(t0)),
            },
        });
        let main = Function {
            symbol: "scoop_main".to_string(),
            params: vec![LirType::Ptr, LirType::Ptr],
            return_ty: LirType::Ptr,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        };

        let any_slots = || {
            vec![
                "scoop_rt_any_equals".to_string(),
                "scoop_rt_any_hashcode".to_string(),
                "scoop_rt_any_tostring".to_string(),
            ]
        };
        Module {
            globals: Arena::default(),
            enums: Arena::default(),
            functions: vec![describe("Shape.describe"), describe("Point.describe"), main],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![
                    // interface Describable: itable key only.
                    TypeDescriptor {
                        name: "Describable".to_string(),
                        symbol: "scoop_td_Describable".to_string(),
                        size: 0,
                        align: 8,
                        scan: RefScan::None,
                        parent: None,
                        vtable: vec![],
                        itables: vec![],
                    },
                    // open class Shape: vtable = Any slots + describe.
                    TypeDescriptor {
                        name: "Shape".to_string(),
                        symbol: "scoop_td_Shape".to_string(),
                        size: 24,
                        align: 8,
                        scan: RefScan::References(vec![16]),
                        parent: None,
                        vtable: any_slots()
                            .into_iter()
                            .chain(["Shape.describe".to_string()])
                            .collect(),
                        itables: vec![],
                    },
                    // class Point : Shape, Describable.
                    TypeDescriptor {
                        name: "Point".to_string(),
                        symbol: "scoop_td_Point".to_string(),
                        size: 32,
                        align: 8,
                        scan: RefScan::References(vec![16]),
                        parent: Some("scoop_td_Shape".to_string()),
                        vtable: any_slots()
                            .into_iter()
                            .chain(["Point.describe".to_string()])
                            .collect(),
                        itables: vec![ItableRecord {
                            interface_symbol: "scoop_td_Describable".to_string(),
                            slots: vec!["Point.describe".to_string()],
                        }],
                    },
                ],
            },
        }
    }

    #[test]
    fn emits_m6_type_descriptors_and_call_indirect() {
        let module = classes_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_m6_test_{}.o", std::process::id()));
        // `emit_object` verifies the LLVM module before writing, so a
        // successful return means `module.verify()` passed.
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }

    /// An M6 heap-access module: a TypeDescriptor reference stub in the
    /// globals arena (skipped at data emission, resolved by symbol),
    /// HeapStore field writes, HeapLoad reads (header, i64
    /// field, ptr field, TD vtable pointer), and a `scoop_rt_box` call
    /// with a by-value aggregate payload.
    fn heap_module() -> Module {
        let mut globals = Arena::default();
        // The TD stub lir-lower appends (CString("") placeholder init);
        // the real TD comes from the meta below.
        let point_td_stub = globals.alloc(Global {
            symbol: "scoop_td_Point".to_string(),
            init: GlobalInit::CString(String::new()),
        });

        // fun @Point.describe(this: ptr) -> ptr: returns `this` (vtable
        // slot material).
        let mut describe_blocks = Arena::default();
        let describe_entry = describe_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![],
            terminator: Terminator::Return {
                value: Some(Value::Param(0)),
            },
        });
        let describe = Function {
            symbol: "Point.describe".to_string(),
            params: vec![LirType::Ptr],
            return_ty: LirType::Ptr,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks: describe_blocks,
            entry: describe_entry,
        };

        // fun @scoop_main() -> void (M9 16-byte header, fields at byte
        // offsets 16 and 24):
        //   t0 = scoop_rt_alloc(@scoop_td_Point, 32)  (stub operand)
        //   heap_store t0 +16, 42     (i64 field)
        //   heap_store t0 +24, t0     (ptr field)
        //   t1 = heap_load t0 +0 : ptr   (object header: the TD)
        //   t2 = heap_load t0 +16 : i64  (field 1)
        //   t3 = heap_load t0 +24 : ptr  (field 2)
        //   t4 = heap_load t1 +40 : ptr  (TD field 5: the vtable pointer)
        //   t5 = aggregate (t2) : {i64}
        //   t6 = scoop_rt_box(@scoop_td_Point, t5, 8)  (by-value payload)
        //   t7 = scoop_rt_is_instance(t6, @scoop_td_Point) : i1
        //   call_indirect t4[3](t3); println_int t2; println_boolean t7
        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: LirType::Ptr });
        let t1 = temps.alloc(Temp { ty: LirType::Ptr });
        let t2 = temps.alloc(Temp { ty: LirType::I64 });
        let t3 = temps.alloc(Temp { ty: LirType::Ptr });
        let t4 = temps.alloc(Temp { ty: LirType::Ptr });
        let t5 = temps.alloc(Temp {
            ty: LirType::Aggregate(vec![LirType::I64]),
        });
        let t6 = temps.alloc(Temp { ty: LirType::Ptr });
        let t7 = temps.alloc(Temp { ty: LirType::I1 });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::Call {
                    out: Some(t0),
                    symbol: "scoop_rt_alloc".to_string(),
                    args: vec![Value::Global(point_td_stub), Value::IntConst(32)],
                },
                Instruction::HeapStore {
                    object: Value::Temp(t0),
                    offset: 16,
                    value: Value::IntConst(42),
                },
                Instruction::HeapStore {
                    object: Value::Temp(t0),
                    offset: 24,
                    value: Value::Temp(t0),
                },
                Instruction::HeapLoad {
                    out: t1,
                    object: Value::Temp(t0),
                    offset: 0,
                },
                Instruction::HeapLoad {
                    out: t2,
                    object: Value::Temp(t0),
                    offset: 16,
                },
                Instruction::HeapLoad {
                    out: t3,
                    object: Value::Temp(t0),
                    offset: 24,
                },
                Instruction::HeapLoad {
                    out: t4,
                    object: Value::Temp(t1),
                    offset: 40,
                },
                Instruction::MakeAggregate {
                    out: t5,
                    elements: vec![Value::Temp(t2)],
                },
                Instruction::Call {
                    out: Some(t6),
                    symbol: "scoop_rt_box".to_string(),
                    args: vec![
                        Value::Global(point_td_stub),
                        Value::Temp(t5),
                        Value::IntConst(8),
                    ],
                },
                Instruction::Call {
                    out: Some(t7),
                    symbol: "scoop_rt_is_instance".to_string(),
                    args: vec![Value::Temp(t6), Value::Global(point_td_stub)],
                },
                Instruction::CallIndirect {
                    out: None,
                    table: Value::Temp(t4),
                    slot: 3,
                    args: vec![Value::Temp(t3)],
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_int".to_string(),
                    args: vec![Value::Temp(t2)],
                },
                Instruction::Call {
                    out: None,
                    symbol: "scoop_rt_println_boolean".to_string(),
                    args: vec![Value::Temp(t7)],
                },
            ],
            terminator: Terminator::Return { value: None },
        });
        let main = Function {
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        };

        Module {
            globals,
            enums: Arena::default(),
            functions: vec![describe, main],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![TypeDescriptor {
                    name: "Point".to_string(),
                    symbol: "scoop_td_Point".to_string(),
                    size: 32,
                    align: 8,
                    scan: RefScan::References(vec![24]),
                    parent: None,
                    vtable: vec![
                        "scoop_rt_any_equals".to_string(),
                        "scoop_rt_any_hashcode".to_string(),
                        "scoop_rt_any_tostring".to_string(),
                        "Point.describe".to_string(),
                    ],
                    itables: vec![],
                }],
            },
        }
    }

    #[test]
    fn emits_m6_heap_access_and_td_stubs() {
        let module = heap_module();
        let output = std::env::temp_dir().join(format!(
            "scoop_codegen_m6_heap_test_{}.o",
            std::process::id()
        ));
        // `emit_object` verifies the LLVM module before writing, so a
        // successful return means `module.verify()` passed.
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }

    /// An M8-shaped module (runtime spec 5): Invoke / InvokeIndirect
    /// sharing one catch-all landing pad, plus the cleanup-pad /
    /// EndCatch / Resume shape used for exceptional handler exits.
    fn exceptions_module() -> Module {
        // fun @scoop.thrower(e: ptr) -> void: the rethrow shape — Throw
        // as the last instruction; the Unreachable terminator emits the
        // LLVM `unreachable` after the noreturn runtime call.
        let mut thrower_blocks = Arena::default();
        let thrower_entry = thrower_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::Throw {
                exception: Value::Param(0),
            }],
            terminator: Terminator::Unreachable,
        });
        let thrower = Function {
            symbol: "scoop.thrower".to_string(),
            params: vec![LirType::Ptr],
            return_ty: LirType::Void,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks: thrower_blocks,
            entry: thrower_entry,
        };

        // fun @scoop.eh_test(table: ptr) -> i64:
        //   entry:  t0 = invoke_indirect table[0]() normal @normal unwind @lpad
        //   normal: t1 = invoke @scoop.may_throw() normal @done unwind @lpad
        //   done:   t2 = t0 + t1; ret t2
        //   lpad:   t3 = landingpad : ptr
        //           end_catch
        //           ret 0
        //   cleanup: t4 = cleanup_pad; end_catch; resume t4
        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: LirType::I64 });
        let t1 = temps.alloc(Temp { ty: LirType::I64 });
        let t2 = temps.alloc(Temp { ty: LirType::I64 });
        let t3 = temps.alloc(Temp {
            ty: LirType::ExceptionRecord,
        });
        let t4 = temps.alloc(Temp { ty: LirType::Ptr });
        let t5 = temps.alloc(Temp { ty: LirType::Ptr });
        let t6 = temps.alloc(Temp {
            ty: LirType::ExceptionRecord,
        });
        let t7 = temps.alloc(Temp { ty: LirType::Ptr });
        let mut blocks = Arena::default();
        let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
            blocks.alloc(BasicBlock {
                name: name.to_string(),
                instructions: vec![],
                terminator: Terminator::Unreachable,
            })
        };
        let entry = placeholder(&mut blocks, "entry");
        let normal = placeholder(&mut blocks, "normal");
        let done = placeholder(&mut blocks, "done");
        let lpad = placeholder(&mut blocks, "lpad");
        let cleanup = placeholder(&mut blocks, "cleanup");
        blocks[entry] = BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::InvokeIndirect {
                out: Some(t0),
                table: Value::Param(0),
                slot: 0,
                args: vec![],
                normal,
                unwind: lpad,
            }],
            terminator: Terminator::Br(normal),
        };
        blocks[normal] = BasicBlock {
            name: "normal".to_string(),
            instructions: vec![Instruction::Invoke {
                out: Some(t1),
                symbol: "scoop.may_throw".to_string(),
                args: vec![],
                normal: done,
                unwind: lpad,
            }],
            terminator: Terminator::Br(done),
        };
        blocks[done] = BasicBlock {
            name: "done".to_string(),
            instructions: vec![Instruction::BinOp {
                out: t2,
                op: BinOp::Add,
                lhs: Value::Temp(t0),
                rhs: Value::Temp(t1),
            }],
            terminator: Terminator::Return {
                value: Some(Value::Temp(t2)),
            },
        };
        blocks[lpad] = BasicBlock {
            name: "lpad".to_string(),
            instructions: vec![
                Instruction::LandingPad {
                    record: t3,
                    raw: t4,
                },
                Instruction::BeginCatch {
                    out: t5,
                    raw: Value::Temp(t4),
                },
                Instruction::EndCatch,
            ],
            terminator: Terminator::Return {
                value: Some(Value::IntConst(0)),
            },
        };
        blocks[cleanup] = BasicBlock {
            name: "cleanup".to_string(),
            instructions: vec![
                Instruction::CleanupPad {
                    record: t6,
                    raw: t7,
                },
                Instruction::EndCatch,
            ],
            terminator: Terminator::Resume {
                exception: Value::Temp(t6),
            },
        };
        let eh_test = Function {
            symbol: "scoop.eh_test".to_string(),
            params: vec![LirType::Ptr],
            return_ty: LirType::I64,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        };

        Module {
            globals: Arena::default(),
            enums: Arena::default(),
            functions: vec![thrower, eh_test],
            entry_symbol: "scoop.eh_test".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn emits_m8_exceptions() {
        let module = exceptions_module();
        let ir = ir_of(&module);
        assert!(ir.contains("@__cxa_begin_catch"));
        assert!(ir.contains("@__cxa_end_catch"));
        assert!(ir.contains("resume { ptr, i32 }"));
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_m8_test_{}.o", std::process::id()));
        // `emit_object` verifies the LLVM module before writing, so a
        // successful return means `module.verify()` passed. M9: this
        // also proves invoke / landingpad and the GC strategy coexist
        // — every function carries `gc "statepoint-example"` and the
        // module goes through `rewrite-statepoints-for-gc`.
        emit_object(&module, &output).expect("emit object");
        let bytes = std::fs::read(&output).expect("read object");
        assert!(!bytes.is_empty(), "object file is empty");
        // The landing pad function must carry an exception table.
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("gcc_except_tab"),
            "object file lacks exception tables"
        );
        assert!(
            text.contains("__llvm_stackmaps"),
            "object file lacks the __llvm_stackmaps section"
        );
        std::fs::remove_file(&output).ok();
    }

    // ---- M9: GC support ----

    /// The LLVM IR text of a module, verified, before the statepoint
    /// rewrite.
    fn ir_of(module: &Module) -> String {
        let machine = host_target_machine().expect("target machine");
        let context = Context::create();
        let llvm = emit_llvm_module(&context, module, &machine).expect("emit module");
        llvm.verify().expect("valid LLVM module");
        llvm.print_to_string().to_string()
    }

    /// An M9-shaped module: a HeapStore and an ArraySet (both carry
    /// the write-barrier card mark) in the entry block, then a
    /// `while`-shaped loop (header ← body back edge) for the loop
    /// safepoint poll.
    fn barrier_module() -> Module {
        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: LirType::Ptr }); // alloc result
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![],
            terminator: Terminator::Unreachable, // placeholder, filled below
        });
        let header = blocks.alloc(BasicBlock {
            name: "while.cond".to_string(),
            instructions: vec![],
            terminator: Terminator::Unreachable,
        });
        let body = blocks.alloc(BasicBlock {
            name: "while.body".to_string(),
            instructions: vec![],
            terminator: Terminator::Unreachable,
        });
        let exit = blocks.alloc(BasicBlock {
            name: "while.exit".to_string(),
            instructions: vec![],
            terminator: Terminator::Return { value: None },
        });
        blocks[entry] = BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::Call {
                    out: Some(t0),
                    symbol: "scoop_rt_alloc".to_string(),
                    args: vec![Value::Param(0), Value::IntConst(24)],
                },
                Instruction::HeapStore {
                    object: Value::Temp(t0),
                    offset: 16,
                    value: Value::IntConst(42),
                },
                Instruction::ArraySet {
                    array: Value::Param(1),
                    index: Value::IntConst(0),
                    value: Value::IntConst(7),
                },
            ],
            terminator: Terminator::Br(header),
        };
        blocks[header] = BasicBlock {
            name: "while.cond".to_string(),
            instructions: vec![],
            terminator: Terminator::CondBr {
                cond: Value::BoolConst(true),
                then_block: body,
                else_block: exit,
            },
        };
        blocks[body] = BasicBlock {
            name: "while.body".to_string(),
            instructions: vec![],
            terminator: Terminator::Br(header),
        };
        Module {
            globals: Arena::default(),
            enums: Arena::default(),
            functions: vec![Function {
                symbol: "scoop_main".to_string(),
                params: vec![LirType::Ptr, LirType::Array(Box::new(LirType::I64))],
                return_ty: LirType::Void,
                locals: Arena::default(),
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn functions_carry_the_gc_strategy_and_poll_safepoints() {
        let ir = ir_of(&barrier_module());
        assert!(
            ir.contains("gc \"statepoint-example\""),
            "function lacks the GC strategy:\n{ir}"
        );
        // One poll at the function entry, one at the loop header.
        let polls = ir.matches("call void @scoop_rt_safepoint()").count();
        assert_eq!(polls, 2, "entry + loop-header safepoint polls:\n{ir}");
    }

    #[test]
    fn heap_stores_mark_the_write_barrier_card() {
        let ir = ir_of(&barrier_module());
        // The card table is a pointer variable: load the (pre-biased)
        // base, then GEP by the card index.
        assert!(
            ir.contains("@scoop_gc_card_table = external global ptr"),
            "card table pointer global missing:\n{ir}"
        );
        assert!(
            ir.contains("load ptr, ptr @scoop_gc_card_table"),
            "card table base load missing:\n{ir}"
        );
        assert!(ir.contains("lshr i64"), "card index shift missing:\n{ir}");
        // One card mark (`store i8 1`) per heap store: the HeapStore
        // and the ArraySet element store.
        let marks = ir.matches("store i8 1").count();
        assert_eq!(marks, 2, "one card mark per heap store:\n{ir}");
    }

    #[test]
    fn heap_store_inside_the_object_header_is_rejected() {
        let mut module = barrier_module();
        let function = &mut module.functions[0];
        let entry = function.entry;
        function.blocks[entry].instructions[1] = Instruction::HeapStore {
            object: Value::IntConst(0),
            offset: 8,
            value: Value::IntConst(42),
        };
        let machine = host_target_machine().expect("target machine");
        let context = Context::create();
        let error = emit_llvm_module(&context, &module, &machine)
            .expect_err("offset 8 is inside the object header, not a field");
        assert!(
            error.0.contains("object header"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn statepoints_and_stackmaps_are_emitted() {
        let module = values_module();
        let machine = host_target_machine().expect("target machine");
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine).expect("emit module");
        llvm.verify().expect("valid LLVM module");
        // The same pass `emit_object` runs before writing the object
        // (the M0 spike's shape).
        llvm.run_passes(
            "rewrite-statepoints-for-gc",
            &machine,
            PassBuilderOptions::create(),
        )
        .expect("rewrite-statepoints-for-gc pass");
        let ir = llvm.print_to_string().to_string();
        assert!(
            ir.contains("gc.statepoint"),
            "statepoint intrinsics missing after rewrite-statepoints-for-gc:\n{ir}"
        );

        let output =
            std::env::temp_dir().join(format!("scoop_codegen_m9_test_{}.o", std::process::id()));
        machine
            .write_to_file(&llvm, FileType::Object, &output)
            .expect("write object");
        let bytes = std::fs::read(&output).expect("read object");
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("__llvm_stackmaps"),
            "object file lacks the __llvm_stackmaps section"
        );
        std::fs::remove_file(&output).ok();
    }

    #[test]
    fn type_descriptors_carry_the_gc_scan_descriptors() {
        // A class's plain table is count-prefixed (`[N, off0, ..]`,
        // runtime/include/scoop_rt.h's M9 scan-descriptor contract).
        let ir = ir_of(&heap_module());
        assert!(
            ir.contains("@scoop_td_Point.refs = private constant [2 x i64] [i64 1, i64 24]"),
            "plain scan table must be count-prefixed:\n{ir}"
        );

        // A reference-element array's TD carries the SCOOP_REFS_ARRAY
        // sentinel (u64::MAX, printed -1), its stride, and a pointer
        // to the recursive scan for one inline element.
        let mut temps = Arena::default();
        let array = temps.alloc(Temp {
            ty: LirType::Array(Box::new(LirType::Ptr)),
        });
        let nested_array = temps.alloc(Temp {
            ty: LirType::Array(Box::new(LirType::Aggregate(vec![
                LirType::I64,
                LirType::Ptr,
                LirType::Ptr,
            ]))),
        });
        let nested_element_scan = RefScan::Sequence(vec![
            RefScan::References(vec![16]),
            RefScan::TaggedEnum {
                tag_offset: 0,
                variants: vec![RefScan::References(vec![8]), RefScan::None, RefScan::None],
            },
        ]);
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::ArrayAlloc {
                    out: array,
                    elements: vec![],
                    element_scan: RefScan::References(vec![0]),
                },
                Instruction::ArrayAlloc {
                    out: nested_array,
                    elements: vec![],
                    element_scan: nested_element_scan,
                },
            ],
            terminator: Terminator::Return { value: None },
        });
        let module = Module {
            globals: Arena::default(),
            enums: Arena::default(),
            functions: vec![Function {
                symbol: "scoop_main".to_string(),
                params: vec![],
                return_ty: LirType::Void,
                locals: Arena::default(),
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![TypeDescriptor {
                    name: "Holder".to_string(),
                    symbol: "scoop_td_Holder".to_string(),
                    size: 56,
                    align: 8,
                    scan: RefScan::Sequence(vec![
                        RefScan::References(vec![16]),
                        RefScan::TaggedEnum {
                            tag_offset: 32,
                            variants: vec![
                                RefScan::References(vec![40]),
                                RefScan::References(vec![48]),
                                RefScan::None,
                            ],
                        },
                    ]),
                    parent: None,
                    vtable: vec![],
                    itables: vec![],
                }],
            },
        };
        let ir = ir_of(&module);
        assert!(
            ir.contains(
                "@scoop_td_array.0.element = private constant [2 x i64] [i64 1, i64 0]"
            ) && ir.contains(
                "@scoop_td_array.0.refs = private constant [3 x i64] [i64 -1, i64 8, i64 ptrtoint (ptr @scoop_td_array.0.element to i64)]"
            ),
            "reference-element array TD must carry SCOOP_REFS_ARRAY:\n{ir}"
        );
        assert!(
            ir.contains(
                "@scoop_td_array.1.element.part.1 = private constant [6 x i64] [i64 -2, i64 0, i64 3"
            ) && ir.contains(
                "@scoop_td_array.1.element = private constant [4 x i64] [i64 -3, i64 2"
            ) && ir.contains(
                "@scoop_td_array.1.refs = private constant [3 x i64] [i64 -1, i64 24, i64 ptrtoint (ptr @scoop_td_array.1.element to i64)]"
            ),
            "aggregate array TD must wrap the recursive element scan:\n{ir}"
        );
        assert!(
            ir.contains(
                "@scoop_td_Holder.refs.part.1 = private constant [6 x i64] [i64 -2, i64 32, i64 3"
            ),
            "nested tagged enum scan must retain the tag and variants:\n{ir}"
        );
        assert!(
            ir.contains("@scoop_td_Holder.refs = private constant [4 x i64] [i64 -3, i64 2"),
            "aggregate scan must compose plain and conditional scans:\n{ir}"
        );
    }
}
