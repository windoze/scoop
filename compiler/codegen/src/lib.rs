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
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicValue, BasicValueEnum, GlobalValue, IntValue, PointerValue, ValueKind};
use inkwell::{AddressSpace, AtomicOrdering, AtomicRMWBinOp, IntPredicate, OptimizationLevel};
use la_arena::{Arena, Idx};
use scoop_lir::{
    ArrayType, ArrayTypeId, BinOp, ConstantValue, EnumDef, EnumRepr, ExternFunction,
    ExternFunctionKind, Function, GcEffect, Global, GlobalInit, Instruction, LirType, Module,
    NativeGlobal, RefScan, StructDef, TempId, Terminator, TypeDescriptor, UnOp, Value,
};

const SCAN_ARRAY: u64 = u64::MAX;
const SCAN_SEQUENCE: u64 = u64::MAX - 1;

/// Error produced while translating LIR or emitting the object file.
#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodegenError {}

/// Generate the C declarations and static assertions used by the M12 C
/// bridge. Synthetic names intentionally do not expose Scoop source field
/// names; the bridge ABI promises byte layout, not a C-facing typedef API.
pub fn c_layout_assertions(module: &Module) -> Result<String, CodegenError> {
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
        for field in &module.structs[id].fields {
            if let LirType::Struct(nested) = &field.ty {
                if module.structs[*nested].c_layout.is_none() {
                    return Err(CodegenError(format!(
                        "C layout `{}` contains ordinary struct `{}`",
                        module.structs[id].name, module.structs[*nested].name
                    )));
                }
                visit(module, *nested, visiting, visited, order)?;
            }
        }
        visiting.remove(&raw);
        visited.insert(raw);
        order.push(id);
        Ok(())
    }

    fn c_type(module: &Module, ty: &LirType) -> Result<String, CodegenError> {
        Ok(match ty {
            LirType::I1 => "_Bool".to_string(),
            LirType::I64 => "uint64_t".to_string(),
            LirType::Ptr(_) => "void *".to_string(),
            LirType::Struct(id) if module.structs[*id].c_layout.is_some() => {
                format!("scoop_c_layout_{}", arena_index(*id))
            }
            LirType::Enum(id) if matches!(module.enums[*id].repr, EnumRepr::Niche { .. }) => {
                "void *".to_string()
            }
            other => {
                return Err(CodegenError(format!(
                    "non-C type {} reached C bridge layout generation",
                    other.dump()
                )));
            }
        })
    }

    let mut order = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for (id, definition) in module.structs.iter() {
        if definition.c_layout.is_some() {
            visit(module, id, &mut visiting, &mut visited, &mut order)?;
        }
    }

    let mut out = String::from("#include <stddef.h>\n#include <stdint.h>\n\n");
    for id in order {
        let definition = &module.structs[id];
        let name = format!("scoop_c_layout_{}", arena_index(id));
        out.push_str(&format!(
            "typedef struct __attribute__((packed, aligned({}))) {} {{\n",
            definition.align, name
        ));
        let mut cursor = 0u64;
        let mut padding_index = 0usize;
        for (field_index, field) in definition.fields.iter().enumerate() {
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
            out.push_str(&format!(
                "  {} _field_{};\n",
                c_type(module, &field.ty)?,
                field_index
            ));
            cursor = field.layout.offset + c_field_size(&module.structs, &module.enums, &field.ty)?;
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
        out.push_str(&format!("}} {};\n", name));
        out.push_str(&format!(
            "_Static_assert(sizeof({}) == {}, \"{} size\");\n",
            name, definition.size, name
        ));
        out.push_str(&format!(
            "_Static_assert(_Alignof({}) == {}, \"{} alignment\");\n",
            name, definition.align, name
        ));
        for (field_index, field) in definition.fields.iter().enumerate() {
            out.push_str(&format!(
                "_Static_assert(offsetof({}, _field_{}) == {}, \"{} field {} offset\");\n",
                name, field_index, field.layout.offset, name, field_index
            ));
        }
        out.push('\n');
    }
    Ok(out)
}

/// Generate the host-C translation unit that owns outbound C ABI wrappers
/// and inbound callback trampolines. `None` means the module needs no second
/// object file.
pub fn c_bridge_source(module: &Module) -> Result<Option<String>, CodegenError> {
    let c_externs = module
        .extern_functions
        .iter()
        .filter(|(_, function)| matches!(function.kind, ExternFunctionKind::C { .. }))
        .collect::<Vec<_>>();
    if c_externs.is_empty()
        && module.native_globals.is_empty()
        && module.callback_bridges.is_empty()
        && module.foreign_callback_bridges.is_empty()
    {
        return Ok(None);
    }

    fn collect_function_pointers(ty: &scoop_lir::CType, found: &mut Vec<scoop_lir::CType>) {
        if let scoop_lir::CType::FunctionPointer {
            params,
            return_type,
        } = ty
        {
            for parameter in params {
                collect_function_pointers(parameter, found);
            }
            collect_function_pointers(return_type, found);
            if !found.contains(ty) {
                found.push(ty.clone());
            }
        }
    }

    fn type_name(ty: &scoop_lir::CType, function_pointers: &[scoop_lir::CType]) -> String {
        match ty {
            scoop_lir::CType::Unit => "void".to_string(),
            scoop_lir::CType::Int => "int64_t".to_string(),
            scoop_lir::CType::UInt => "uint64_t".to_string(),
            scoop_lir::CType::Boolean => "_Bool".to_string(),
            scoop_lir::CType::Pointer => "void *".to_string(),
            scoop_lir::CType::Struct(id) => {
                format!("scoop_c_layout_{}", arena_index(*id))
            }
            scoop_lir::CType::FunctionPointer { .. } => {
                let index = function_pointers
                    .iter()
                    .position(|candidate| candidate == ty)
                    .expect("function pointer type was collected");
                format!("scoop_c_funptr_{index}")
            }
        }
    }

    let mut function_pointers = Vec::new();
    for (_, function) in &c_externs {
        let ExternFunctionKind::C {
            params,
            return_type,
            ..
        } = &function.kind
        else {
            unreachable!()
        };
        for parameter in params {
            collect_function_pointers(parameter, &mut function_pointers);
        }
        collect_function_pointers(return_type, &mut function_pointers);
    }
    for (_, global) in module.native_globals.iter() {
        collect_function_pointers(&global.c_type, &mut function_pointers);
    }
    for (_, callback) in module.callback_bridges.iter() {
        for parameter in &callback.params {
            collect_function_pointers(parameter, &mut function_pointers);
        }
        collect_function_pointers(&callback.return_type, &mut function_pointers);
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        for parameter in &callback.params {
            collect_function_pointers(parameter, &mut function_pointers);
        }
        collect_function_pointers(&callback.return_type, &mut function_pointers);
    }

    let mut out = c_layout_assertions(module)?;
    out.push_str("#include <string.h>\n\n");
    for (index, ty) in function_pointers.iter().enumerate() {
        let scoop_lir::CType::FunctionPointer {
            params,
            return_type,
        } = ty
        else {
            unreachable!()
        };
        let params = if params.is_empty() {
            "void".to_string()
        } else {
            params
                .iter()
                .map(|parameter| type_name(parameter, &function_pointers))
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "typedef {} (*scoop_c_funptr_{index})({params});\n",
            type_name(return_type, &function_pointers)
        ));
    }
    if !function_pointers.is_empty() {
        out.push('\n');
    }

    let mut declared_symbols = HashSet::new();
    for (id, function) in c_externs {
        let ExternFunctionKind::C {
            bridge_symbol,
            params,
            return_type,
        } = &function.kind
        else {
            unreachable!()
        };
        let parameter_names = params
            .iter()
            .map(|parameter| type_name(parameter, &function_pointers))
            .collect::<Vec<_>>();
        if declared_symbols.insert(function.native_symbol.clone()) {
            let prototype_params = if parameter_names.is_empty() {
                "void".to_string()
            } else {
                parameter_names.join(", ")
            };
            out.push_str(&format!(
                "extern {} {}({});\n",
                type_name(return_type, &function_pointers),
                function.native_symbol,
                prototype_params
            ));
        }

        let has_result = *return_type != scoop_lir::CType::Unit;
        let mut wrapper_params = Vec::new();
        if has_result {
            wrapper_params.push("void *result".to_string());
        }
        wrapper_params.extend(
            params
                .iter()
                .enumerate()
                .map(|(index, _)| format!("const void *arg{index}")),
        );
        if wrapper_params.is_empty() {
            wrapper_params.push("void".to_string());
        }
        out.push_str(&format!(
            "void {}({}) {{\n",
            bridge_symbol,
            wrapper_params.join(", ")
        ));
        for (index, parameter) in params.iter().enumerate() {
            let name = type_name(parameter, &function_pointers);
            out.push_str(&format!(
                "  {name} value{index};\n  memcpy(&value{index}, arg{index}, sizeof(value{index}));\n"
            ));
        }
        let arguments = (0..params.len())
            .map(|index| format!("value{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        if has_result {
            let result_name = type_name(return_type, &function_pointers);
            out.push_str(&format!(
                "  {result_name} native_result = {}({arguments});\n  memcpy(result, &native_result, sizeof(native_result));\n",
                function.native_symbol
            ));
        } else {
            out.push_str(&format!("  {}({arguments});\n", function.native_symbol));
        }
        out.push_str("}\n\n");
        let _ = id;
    }
    for (_, global) in module.native_globals.iter() {
        let native_type = type_name(&global.c_type, &function_pointers);
        let thread_local = if global.thread_local {
            "_Thread_local "
        } else {
            ""
        };
        if declared_symbols.insert(global.native_symbol.clone()) {
            out.push_str(&format!(
                "extern {thread_local}{native_type} {};\n",
                global.native_symbol
            ));
        }
        out.push_str(&format!(
            "void {}(void *result) {{\n  memcpy(result, &{}, sizeof({}));\n}}\n\n",
            global.get_bridge_symbol, global.native_symbol, global.native_symbol
        ));
        if let Some(setter) = &global.set_bridge_symbol {
            out.push_str(&format!(
                "void {setter}(const void *value) {{\n  memcpy(&{}, value, sizeof({}));\n}}\n\n",
                global.native_symbol, global.native_symbol
            ));
        }
        out.push_str(&format!(
            "void {}(void *result) {{\n  void *native_address = (void *)&{};\n  memcpy(result, &native_address, sizeof(native_address));\n}}\n\n",
            global.address_bridge_symbol, global.native_symbol
        ));
    }
    for (_, callback) in module.callback_bridges.iter() {
        let has_result = callback.return_type != scoop_lir::CType::Unit;
        let mut storage_params = Vec::new();
        if has_result {
            storage_params.push("void *result".to_string());
        }
        storage_params.extend(
            callback
                .params
                .iter()
                .enumerate()
                .map(|(index, _)| format!("const void *arg{index}")),
        );
        if storage_params.is_empty() {
            storage_params.push("void".to_string());
        }
        out.push_str(&format!(
            "extern void {}({});\n",
            callback.bridge_symbol,
            storage_params.join(", ")
        ));

        let callback_params = if callback.params.is_empty() {
            "void".to_string()
        } else {
            callback
                .params
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    format!("{} arg{index}", type_name(parameter, &function_pointers))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "{} {}({callback_params}) {{\n",
            type_name(&callback.return_type, &function_pointers),
            callback.trampoline_symbol
        ));
        if has_result {
            out.push_str(&format!(
                "  {} result;\n",
                type_name(&callback.return_type, &function_pointers)
            ));
        }
        let mut storage_args = Vec::new();
        if has_result {
            storage_args.push("&result".to_string());
        }
        storage_args.extend((0..callback.params.len()).map(|index| format!("&arg{index}")));
        out.push_str(&format!(
            "  {}({});\n",
            callback.bridge_symbol,
            storage_args.join(", ")
        ));
        if has_result {
            out.push_str("  return result;\n");
        }
        out.push_str("}\n\n");
    }
    if !module.foreign_callback_bridges.is_empty() {
        out.push_str(
            "extern uint32_t scoop_runtime_callback_invoke(void *context, const void *signature, void *result, const void *const *arguments);\n\n",
        );
    }
    let mut emitted_foreign_trampolines = HashSet::new();
    for (_, callback) in module.foreign_callback_bridges.iter() {
        if !emitted_foreign_trampolines.insert(callback.trampoline_symbol.as_str()) {
            continue;
        }
        out.push_str(&format!(
            "const unsigned char {} = 0;\n",
            callback.signature_symbol
        ));
        let callback_params = callback
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                format!("{} arg{index}", type_name(parameter, &function_pointers))
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "{} {}({}) {{\n",
            type_name(&callback.return_type, &function_pointers),
            callback.trampoline_symbol,
            if callback_params.is_empty() {
                "void"
            } else {
                &callback_params
            }
        ));
        let has_result = callback.return_type != scoop_lir::CType::Unit;
        if has_result {
            out.push_str(&format!(
                "  {} result = {{0}};\n",
                type_name(&callback.return_type, &function_pointers)
            ));
        }
        let argument_indices = (0..callback.params.len())
            .filter(|index| *index != callback.context_index as usize)
            .collect::<Vec<_>>();
        if !argument_indices.is_empty() {
            out.push_str(&format!(
                "  const void *arguments[{}] = {{{}}};\n",
                argument_indices.len(),
                argument_indices
                    .iter()
                    .map(|index| format!("&arg{index}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push_str(&format!(
            "  (void)scoop_runtime_callback_invoke(arg{}, &{}, {}, {});\n",
            callback.context_index,
            callback.signature_symbol,
            if has_result { "&result" } else { "NULL" },
            if argument_indices.is_empty() {
                "NULL"
            } else {
                "arguments"
            }
        ));
        if has_result {
            out.push_str("  return result;\n");
        }
        out.push_str("}\n\n");
    }
    Ok(Some(out))
}

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

fn align_up(value: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

fn array_data_offset(element_align: u64) -> u64 {
    align_up(24, element_align)
}

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
    // String constants need the descriptor address before function tables can
    // be emitted. The complete initializer comes from typed LIR metadata after
    // all function declarations exist; codegen does not synthesize it.
    let string_td = llvm.add_global(td_ty, None, scoop_lir::STRING_TD_SYMBOL);

    // Declare every descriptor from complete LIR metadata before any
    // initializer is built. Parent/interface references therefore resolve by
    // construction; array descriptors are indexed exactly like their
    // `ArrayTypeId` arena and are never deduplicated from instruction shapes.
    let type_tds: Vec<GlobalValue> = module
        .meta
        .type_descriptors
        .iter()
        .map(|descriptor| llvm.add_global(td_ty, None, &descriptor.symbol))
        .collect();
    let array_tds: Vec<GlobalValue> = module
        .meta
        .arrays
        .iter()
        .map(|(_, array)| llvm.add_global(td_ty, None, &array.type_descriptor.symbol))
        .collect();

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
    // typed `LirMeta` descriptors and
    // `Value::Global` resolves them by symbol at use time. A stub is
    // recognized by its symbol naming one of those TDs, never by its
    // init shape.
    let td_symbols: HashSet<&str> = module
        .meta
        .type_descriptors
        .iter()
        .map(|td| td.symbol.as_str())
        .chain(
            module
                .meta
                .arrays
                .iter()
                .map(|(_, array)| array.type_descriptor.symbol.as_str()),
        )
        .chain([module.meta.string.type_descriptor.symbol.as_str()])
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
        structs: &module.structs,
        enums: &module.enums,
        extern_functions: &module.extern_functions,
        native_globals: &module.native_globals,
        foreign_callback_bridges: &module.foreign_callback_bridges,
        globals_arena: &module.globals,
        globals: &globals,
        arrays: &module.meta.arrays,
        array_tds: &array_tds,
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
    emit_type_descriptors(context, &llvm, string_td, &type_tds, &array_tds, module)?;
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
        ConstantValue::NullPtr => ptr_ty(context).const_null().into(),
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

/// First generated `type_id` after the runtime-owned String id 1. Ordinary
/// descriptors are followed immediately by concrete arrays, so uniqueness
/// does not depend on a reserved numeric range or an assumed entity count.
const FIRST_GENERATED_TD_TYPE_ID: u64 = 2;

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
    string_global: GlobalValue<'ctx>,
    type_globals: &[GlobalValue<'ctx>],
    array_globals: &[GlobalValue<'ctx>],
    module: &Module,
) -> Result<(), CodegenError> {
    let ptr = ptr_ty(context);
    // ScoopItableEntry: { ptr interface, ptr slots }.
    let entry_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
    for (index, (td, global)) in module
        .meta
        .type_descriptors
        .iter()
        .zip(type_globals)
        .enumerate()
    {
        emit_type_descriptor(
            context,
            llvm,
            entry_ty,
            *global,
            td,
            FIRST_GENERATED_TD_TYPE_ID + index as u64,
            DescriptorScan::Fixed(&td.scan),
        )?;
    }
    for (index, ((_, array), global)) in module.meta.arrays.iter().zip(array_globals).enumerate() {
        emit_type_descriptor(
            context,
            llvm,
            entry_ty,
            *global,
            &array.type_descriptor,
            FIRST_GENERATED_TD_TYPE_ID + module.meta.type_descriptors.len() as u64 + index as u64,
            DescriptorScan::ArrayElement {
                stride: array.element_size,
                scan: &array.type_descriptor.scan,
            },
        )?;
    }
    emit_type_descriptor(
        context,
        llvm,
        entry_ty,
        string_global,
        &module.meta.string.type_descriptor,
        1,
        DescriptorScan::Fixed(&module.meta.string.type_descriptor.scan),
    )?;
    Ok(())
}

enum DescriptorScan<'a> {
    Fixed(&'a RefScan),
    ArrayElement { stride: u64, scan: &'a RefScan },
}

fn emit_type_descriptor<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    entry_ty: StructType<'ctx>,
    global: GlobalValue<'ctx>,
    descriptor: &TypeDescriptor,
    type_id: u64,
    scan: DescriptorScan<'_>,
) -> Result<(), CodegenError> {
    let i64_ty = context.i64_type();
    let ptr = ptr_ty(context);
    let ref_offsets: BasicValueEnum = match scan {
        DescriptorScan::Fixed(scan) => {
            emit_ref_scan(context, llvm, &format!("{}.refs", descriptor.symbol), scan)
                .map_or_else(|| ptr.const_null().into(), Into::into)
        }
        DescriptorScan::ArrayElement { stride, scan } => emit_ref_scan(
            context,
            llvm,
            &format!("{}.element", descriptor.symbol),
            scan,
        )
        .map_or_else(
            || ptr.const_null().into(),
            |element_scan| {
                let words = i64_ty.const_array(&[
                    i64_ty.const_int(SCAN_ARRAY, false),
                    i64_ty.const_int(stride, false),
                    element_scan.const_to_int(i64_ty),
                ]);
                private_const_global(llvm, &format!("{}.refs", descriptor.symbol), words.into())
                    .into()
            },
        ),
    };
    let parent: BasicValueEnum = match &descriptor.parent {
        Some(symbol) => llvm
            .get_global(symbol)
            .ok_or_else(|| {
                CodegenError(format!(
                    "TypeDescriptor `{}`: parent `@{}` not emitted yet",
                    descriptor.symbol, symbol
                ))
            })?
            .as_pointer_value()
            .into(),
        None => ptr.const_null().into(),
    };
    let vtable = emit_fn_table(
        context,
        llvm,
        &format!("{}.vtable", descriptor.symbol),
        &descriptor.vtable,
    )?;
    let (itables, itable_count): (BasicValueEnum, u64) = if descriptor.itables.is_empty() {
        (ptr.const_null().into(), 0)
    } else {
        let mut entries = Vec::with_capacity(descriptor.itables.len());
        for (record_index, record) in descriptor.itables.iter().enumerate() {
            let interface = llvm
                .get_global(&record.interface_symbol)
                .ok_or_else(|| {
                    CodegenError(format!(
                        "TypeDescriptor `{}`: interface `@{}` not emitted yet",
                        descriptor.symbol, record.interface_symbol
                    ))
                })?
                .as_pointer_value();
            let slots = emit_fn_table(
                context,
                llvm,
                &format!("{}.itables.{record_index}", descriptor.symbol),
                &record.slots,
            )?;
            entries.push(context.const_struct(&[interface.into(), slots], false));
        }
        let array = entry_ty.const_array(&entries);
        let itable_global = private_const_global(
            llvm,
            &format!("{}.itables", descriptor.symbol),
            array.into(),
        );
        (itable_global.into(), descriptor.itables.len() as u64)
    };
    let name = private_c_string(
        context,
        llvm,
        &format!("{}.name", descriptor.symbol),
        &descriptor.name,
    );
    global.set_constant(true);
    global.set_initializer(&context.const_struct(
        &[
            i64_ty.const_int(type_id, false).into(),
            i64_ty.const_int(descriptor.size, false).into(),
            i64_ty.const_int(descriptor.align, false).into(),
            ref_offsets,
            parent,
            vtable,
            itables,
            i64_ty.const_int(itable_count, false).into(),
            name.into(),
        ],
        false,
    ));
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
        values.push(slot_fn_ptr(llvm, symbol)?);
    }
    let array = ptr.const_array(&values);
    Ok(private_const_global(llvm, name, array.into()).into())
}

/// Address of the exact module function named by a vtable / itable slot.
/// All dispatch entries are declared in the first pass; codegen never guesses
/// a missing function's signature from its symbol.
fn slot_fn_ptr<'ctx>(
    llvm: &LlvmModule<'ctx>,
    symbol: &str,
) -> Result<PointerValue<'ctx>, CodegenError> {
    llvm.get_function(symbol)
        .map(|function| function.as_global_value().as_pointer_value())
        .ok_or_else(|| {
            CodegenError(format!(
                "vtable/itable slot `@{symbol}` is not a declared function"
            ))
        })
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
    structs: &'a Arena<StructDef>,
    enums: &'a Arena<EnumDef>,
    extern_functions: &'a Arena<ExternFunction>,
    native_globals: &'a Arena<NativeGlobal>,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    /// Complete array metadata and descriptor globals, indexed directly by
    /// `ArrayTypeId`.
    arrays: &'a Arena<ArrayType>,
    array_tds: &'a [GlobalValue<'ctx>],
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
            Value::NullPtr => ptr_ty(context).const_null().into(),
            Value::Global(id) => match &self.globals[arena_index(id)] {
                Some(global) => global.as_pointer_value().into(),
                // A TypeDescriptor stub: the TD global (emitted from the
                // metadata, including the typed intrinsic String descriptor)
                // is resolved by symbol.
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
            Instruction::ForeignCallbackOperation {
                out,
                operation,
                callback,
            } => {
                let callback = self.value(*callback)?.into_struct_value();
                let function_pointer = builder
                    .build_extract_value(callback, 0, "callback_function")
                    .map_err(|error| CodegenError(format!("extract callback function: {error}")))?
                    .into_pointer_value();
                let callback_context = builder
                    .build_extract_value(callback, 1, "callback_context")
                    .map_err(|error| CodegenError(format!("extract callback context: {error}")))?
                    .into_pointer_value();
                match operation {
                    scoop_lir::ForeignCallbackOperation::Retain => {
                        let out = out.expect("retain produces a callback value");
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
                    scoop_lir::ForeignCallbackOperation::Release => {
                        debug_assert!(out.is_none());
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
                    scoop_lir::ForeignCallbackOperation::Failure => {
                        let out = out.expect("failure query produces Option<Throwable>");
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
                    scoop_lir::ForeignCallbackOperation::State => {
                        let out = out.expect("state query produces an enum value");
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
                let callee = self.native_global_bridge(&native.get_bridge_symbol);
                let transition = self.publish_native_roots(
                    roots,
                    None,
                    scoop_lir::NativeCallEffect::NativeSafe,
                )?;
                builder
                    .build_call(callee, &[slot.into()], "native_global_get")
                    .map_err(|error| CodegenError(format!("native global read: {error}")))?;
                self.finish_native_transition(transition, scoop_lir::NativeCallEffect::NativeSafe)?;
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
                let symbol = native
                    .set_bridge_symbol
                    .as_deref()
                    .expect("only mutable native globals are assigned");
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots,
                    None,
                    scoop_lir::NativeCallEffect::NativeSafe,
                )?;
                builder
                    .build_call(callee, &[slot.into()], "native_global_set")
                    .map_err(|error| CodegenError(format!("native global write: {error}")))?;
                self.finish_native_transition(transition, scoop_lir::NativeCallEffect::NativeSafe)?;
            }
            Instruction::NativeGlobalAddress { out, global, roots } => {
                let native = &self.native_globals[*global];
                let ty: BasicTypeEnum = ptr_ty(context).into();
                let slot = self.entry_alloca(ty, "native_global_address")?;
                let callee = self.native_global_bridge(&native.address_bridge_symbol);
                let transition = self.publish_native_roots(
                    roots,
                    None,
                    scoop_lir::NativeCallEffect::NativeSafe,
                )?;
                builder
                    .build_call(callee, &[slot.into()], "native_global_address")
                    .map_err(|error| CodegenError(format!("native global address: {error}")))?;
                self.finish_native_transition(transition, scoop_lir::NativeCallEffect::NativeSafe)?;
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
            Instruction::NativeCall {
                out,
                function: extern_id,
                effect,
                args,
                roots,
                result_scan,
            } => {
                let extern_ = &self.extern_functions[*extern_id];
                let expected_effect = match &extern_.kind {
                    ExternFunctionKind::C { .. } => scoop_lir::NativeCallEffect::NativeSafe,
                    ExternFunctionKind::Scoop { .. } => scoop_lir::NativeCallEffect::NativeBorrowed,
                };
                if *effect != expected_effect {
                    return Err(CodegenError(format!(
                        "native call effect does not match extern{} ABI",
                        extern_id.into_raw().into_u32()
                    )));
                }
                let (symbol, gc_effect, c_bridge) = match &extern_.kind {
                    ExternFunctionKind::C { bridge_symbol, .. } => {
                        (bridge_symbol.as_str(), GcEffect::NoGc, true)
                    }
                    ExternFunctionKind::Scoop { gc_effect } => {
                        (extern_.native_symbol.as_str(), *gc_effect, false)
                    }
                };
                let result_slot = out
                    .map(|temp| {
                        let lir_ty = &function.temps[temp].ty;
                        let indirect = !c_bridge && uses_return_slot(self.enums, lir_ty);
                        let needs_storage = c_bridge || indirect || *result_scan != RefScan::None;
                        needs_storage
                            .then(|| {
                                let ty = basic_ty(context, self.structs, self.enums, lir_ty)?;
                                let slot = self.entry_alloca(ty, "native_result")?;
                                if *result_scan != RefScan::None {
                                    builder.build_store(slot, ty.const_zero()).map_err(
                                        |error| {
                                            CodegenError(format!(
                                                "zero native result @{symbol}: {error}"
                                            ))
                                        },
                                    )?;
                                }
                                Ok((temp, slot, ty, indirect))
                            })
                            .transpose()
                    })
                    .transpose()?
                    .flatten();
                let mut param_tys: Vec<BasicMetadataTypeEnum> = if c_bridge {
                    vec![ptr_ty(context).into(); args.len()]
                } else {
                    args.iter()
                        .map(|arg| {
                            basic_ty(
                                context,
                                self.structs,
                                self.enums,
                                &function.value_ty(self.globals_arena, *arg),
                            )
                            .map(Into::into)
                        })
                        .collect::<Result<_, _>>()?
                };
                if result_slot
                    .as_ref()
                    .is_some_and(|(_, _, _, indirect)| c_bridge || *indirect)
                {
                    param_tys.insert(0, ptr_ty(context).into());
                }
                let fn_ty = match (out, &result_slot, c_bridge) {
                    (_, _, true) | (_, Some((_, _, _, true)), false) => {
                        context.void_type().fn_type(&param_tys, false)
                    }
                    (Some(temp), _, false) => {
                        basic_ty(context, self.structs, self.enums, &function.temps[*temp].ty)?
                            .fn_type(&param_tys, false)
                    }
                    (None, None, false) => context.void_type().fn_type(&param_tys, false),
                    (None, Some(_), false) => {
                        unreachable!("native result storage requires an output temp")
                    }
                };
                let callee = self
                    .llvm
                    .get_function(symbol)
                    .unwrap_or_else(|| self.llvm.add_function(symbol, fn_ty, None));
                callee.add_attribute(
                    AttributeLoc::Function,
                    context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
                );
                if gc_effect == GcEffect::NoGc {
                    callee.add_attribute(
                        AttributeLoc::Function,
                        context.create_string_attribute("gc-leaf-function", ""),
                    );
                }
                let mut call_args =
                    args.iter()
                        .map(|arg| self.value(*arg).map(Into::into))
                        .collect::<Result<Vec<inkwell::values::BasicMetadataValueEnum>, _>>()?;
                if let Some((_, slot, _, indirect)) = result_slot
                    && (c_bridge || indirect)
                {
                    call_args.insert(0, slot.into());
                }
                let result_root = result_slot
                    .as_ref()
                    .filter(|_| *result_scan != RefScan::None)
                    .map(|(_, slot, _, _)| (*slot, result_scan));
                let native = self.publish_native_roots(roots, result_root, *effect)?;
                let call = builder
                    .build_call(callee, &call_args, "native_call")
                    .map_err(|error| CodegenError(format!("native call @{symbol}: {error}")))?;
                let direct_result = if out.is_some()
                    && !c_bridge
                    && !result_slot
                        .as_ref()
                        .is_some_and(|(_, _, _, indirect)| *indirect)
                {
                    let ValueKind::Basic(result) = call.try_as_basic_value() else {
                        return Err(CodegenError(format!(
                            "native call @{symbol} produced no direct value"
                        )));
                    };
                    if let Some((_, slot, _, _)) = result_slot {
                        builder.build_store(slot, result).map_err(|error| {
                            CodegenError(format!("store native result @{symbol}: {error}"))
                        })?;
                    }
                    Some(result)
                } else {
                    None
                };
                self.finish_native_transition(native, *effect)?;
                if let Some((temp, slot, ty, _)) = result_slot {
                    let result =
                        builder
                            .build_load(ty, slot, "native_result")
                            .map_err(|error| {
                                CodegenError(format!("load native result @{symbol}: {error}"))
                            })?;
                    self.temps.insert(temp, result);
                } else if let (Some(temp), Some(result)) = (out, direct_result) {
                    self.temps.insert(*temp, result);
                }
            }
            Instruction::Call { out, symbol, args } => {
                if symbol == "scoop_rt_alloc" {
                    self.managed_alloc(out, args)?;
                    return Ok(());
                }
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
                            self.structs,
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
                            basic_ty(context, self.structs, self.enums, &function.temps[*temp].ty)?
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
                            self.structs,
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
                    (Some(temp), None) => {
                        basic_ty(context, self.structs, self.enums, &function.temps[*temp].ty)?
                            .fn_type(&param_tys, false)
                    }
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
                            self.structs,
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
                    (Some(temp), None) => {
                        basic_ty(context, self.structs, self.enums, &function.temps[*temp].ty)?
                            .fn_type(&param_tys, false)
                    }
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
                            self.structs,
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
                    (Some(temp), None) => {
                        basic_ty(context, self.structs, self.enums, &function.temps[*temp].ty)?
                            .fn_type(&param_tys, false)
                    }
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
                        for (field, offset) in fields.iter().zip(&variant_repr.field_offsets) {
                            let field_ptr = self.enum_field_ptr(slot, *offset, "field_ptr")?;
                            builder
                                .build_store(field_ptr, self.value(*field)?)
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
                        let field_ptr = self.enum_field_ptr(
                            slot,
                            variant_repr.field_offsets[*index as usize],
                            "field_ptr",
                        )?;
                        let field_ty = basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            &variant_repr.fields[*index as usize],
                        )?;
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
        let ty = basic_ty(self.context, self.structs, self.enums, ty)?;
        let slot = self.entry_alloca(ty, name)?;
        Ok(Some((temp, slot, ty)))
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
        effect: scoop_lir::NativeCallEffect,
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
                &root.scan,
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
            scoop_lir::NativeCallEffect::NativeSafe => "scoop_rt_enter_native_safe",
            scoop_lir::NativeCallEffect::NativeBorrowed => "scoop_rt_enter_native_borrowed",
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
        effect: scoop_lir::NativeCallEffect,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let ptr = ptr_ty(context);
        let leave_symbol = match effect {
            scoop_lir::NativeCallEffect::NativeSafe => "scoop_rt_leave_native_safe",
            scoop_lir::NativeCallEffect::NativeBorrowed => "scoop_rt_leave_native_borrowed",
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
    fn managed_alloc(&mut self, out: &Option<TempId>, args: &[Value]) -> Result<(), CodegenError> {
        let Some(out) = *out else {
            return Err(CodegenError(format!(
                "scoop_rt_alloc @{}: allocation has no result",
                self.function.symbol
            )));
        };
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
            self.structs,
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
    structs: &'a Arena<StructDef>,
    enums: &'a Arena<EnumDef>,
    extern_functions: &'a Arena<ExternFunction>,
    native_globals: &'a Arena<NativeGlobal>,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    arrays: &'a Arena<ArrayType>,
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
        structs: module_ctx.structs,
        enums: module_ctx.enums,
        extern_functions: module_ctx.extern_functions,
        native_globals: module_ctx.native_globals,
        foreign_callback_bridges: module_ctx.foreign_callback_bridges,
        globals_arena: module_ctx.globals_arena,
        globals: module_ctx.globals,
        arrays: module_ctx.arrays,
        array_tds: module_ctx.array_tds,
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
        BasicBlock, EnumDef, EnumRepr, EnumVariantRepr, Global, GlobalInit, ItableRecord, Layout,
        LayoutKind, LirMeta, Local, MANAGED_PTR, METADATA_PTR, PointerKind, RAW_PTR,
        StringMetadata, Temp, TypeDescriptor,
    };

    use super::*;

    fn string_metadata() -> StringMetadata {
        StringMetadata {
            layout: Layout {
                name: "String".to_string(),
                size: 24,
                align: 8,
                fields: Vec::new(),
                c_layout: None,
                interior_mutable: false,
                kind: LayoutKind::Intrinsic(scoop_lir::IntrinsicTypeRepresentation::String),
            },
            type_descriptor: TypeDescriptor {
                name: "String".to_string(),
                symbol: scoop_lir::STRING_TD_SYMBOL.to_string(),
                size: 24,
                align: 8,
                scan: RefScan::None,
                parent: None,
                vtable: Vec::new(),
                itables: Vec::new(),
            },
        }
    }

    fn array_type(
        arrays: &mut Arena<ArrayType>,
        name: &str,
        kind: scoop_lir::ArrayKind,
        element: LirType,
        element_size: u64,
        element_align: u64,
        scan: RefScan,
    ) -> ArrayTypeId {
        arrays.alloc(ArrayType {
            kind,
            element,
            element_size,
            element_align,
            type_descriptor: TypeDescriptor {
                name: name.to_string(),
                symbol: format!("scoop_td_{name}"),
                size: element_size,
                align: element_align,
                scan,
                parent: None,
                vtable: Vec::new(),
                itables: Vec::new(),
            },
        })
    }

    /// An M2-shaped module: string constants, a user function exercising
    /// alloca/store/load, arithmetic, branches, aggregates and
    /// extractvalue, plus calls into the new runtime functions.
    fn values_module() -> Module {
        let mut globals = Arena::default();
        let hello = globals.alloc(Global {
            symbol: "scoop.string.0".to_string(),
            address_kind: PointerKind::Managed,
            init: GlobalInit::StringConst("hello, ".to_string()),
        });
        let world = globals.alloc(Global {
            symbol: "scoop.string.1".to_string(),
            address_kind: PointerKind::Managed,
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
        let t6 = temp(&mut temps, MANAGED_PTR);
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
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
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
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
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
            address_kind: PointerKind::Raw,
            init: GlobalInit::CString("unwrap on None".to_string()),
        });
        let mut enums = Arena::default();
        // Dot/Circle share the pure-value slot at 8; Rect owns a
        // ref-bearing slot at 16, with its String at offset 24.
        let shape = enums.alloc(EnumDef {
            name: "Shape".to_string(),
            repr: EnumRepr::Tagged {
                variants: vec![
                    EnumVariantRepr {
                        fields: vec![],
                        field_offsets: vec![],
                        slot_offset: 8,
                        slot_size: 0,
                        slot_align: 1,
                        gc_free: true,
                    },
                    EnumVariantRepr {
                        fields: vec![LirType::I64],
                        field_offsets: vec![8],
                        slot_offset: 8,
                        slot_size: 8,
                        slot_align: 8,
                        gc_free: true,
                    },
                    EnumVariantRepr {
                        fields: vec![LirType::I64, MANAGED_PTR],
                        field_offsets: vec![16, 24],
                        slot_offset: 16,
                        slot_size: 16,
                        slot_align: 8,
                        gc_free: false,
                    },
                ],
                size: 32,
                align: 8,
            },
            scan: RefScan::References(vec![24]),
        });
        // enum Option<String> { None, Some(String) } — niche pointer.
        let option = enums.alloc(EnumDef {
            name: "Option<String>".to_string(),
            repr: EnumRepr::Niche { payload_variant: 1 },
            scan: RefScan::References(vec![0]),
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
        let t6 = tagged_temps.alloc(Temp { ty: MANAGED_PTR }); // enum_field v2 f1 s2 (local)
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
            gc_effect: GcEffect::Managed,
            symbol: "scoop.tagged".to_string(),
            params: vec![shape_ty.clone(), MANAGED_PTR],
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
        let n1 = niche_temps.alloc(Temp { ty: MANAGED_PTR }); // enum_field v1 f0 o
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
            gc_effect: GcEffect::Managed,
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
            gc_effect: GcEffect::Managed,
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
            gc_effect: GcEffect::Managed,
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
            gc_effect: GcEffect::Managed,
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
            gc_effect: GcEffect::Managed,
            symbol: "scoop.consume_shape_indirect".to_string(),
            params: vec![METADATA_PTR],
            return_ty: LirType::I64,
            locals: Arena::default(),
            temps: indirect_temps,
            blocks: indirect_blocks,
            entry: indirect_entry,
        };

        Module {
            globals,
            structs: Arena::default(),
            enums,
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
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
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![
                    Layout {
                        name: "String".to_string(),
                        size: 24,
                        align: 8,
                        fields: Vec::new(),
                        c_layout: None,
                        interior_mutable: false,
                        kind: LayoutKind::Plain {
                            scan: RefScan::None,
                        },
                    },
                    Layout {
                        name: "Shape".to_string(),
                        size: 32,
                        align: 8,
                        fields: Vec::new(),
                        c_layout: None,
                        interior_mutable: false,
                        kind: LayoutKind::Enum {
                            scan: RefScan::References(vec![24]),
                        },
                    },
                    Layout {
                        name: "Option<String>".to_string(),
                        size: 8,
                        align: 8,
                        fields: Vec::new(),
                        c_layout: None,
                        interior_mutable: false,
                        kind: LayoutKind::Enum {
                            scan: RefScan::References(vec![0]),
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
        let ir = ir_of(&module);
        assert!(
            ir.contains("store { i64, [24 x i8] } zeroinitializer"),
            "tagged enum construction must zero every inactive ref slot:\n{ir}"
        );
        assert!(
            ir.contains("getelementptr i8, ptr %enum_wrap") && ir.contains("i64 24"),
            "ref-bearing variant fields must use their assigned slot offsets:\n{ir}"
        );
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
        let mut arrays = Arena::new();
        let int_array = array_type(
            &mut arrays,
            "Array<Int>",
            scoop_lir::ArrayKind::Immutable,
            LirType::I64,
            8,
            8,
            RefScan::None,
        );
        let mutable_int_array = array_type(
            &mut arrays,
            "MutableArray<Int>",
            scoop_lir::ArrayKind::Mutable,
            LirType::I64,
            8,
            8,
            RefScan::None,
        );
        let point_array = array_type(
            &mut arrays,
            "Array<Point>",
            scoop_lir::ArrayKind::Immutable,
            point.clone(),
            16,
            8,
            RefScan::None,
        );
        let mutable_point_array = array_type(
            &mut arrays,
            "MutableArray<Point>",
            scoop_lir::ArrayKind::Mutable,
            point.clone(),
            16,
            8,
            RefScan::None,
        );

        let mut locals = Arena::default();
        let numbers = locals.alloc(Local {
            name: "numbers".to_string(),
            ty: MANAGED_PTR,
        });

        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_alloc (1, 2, 3)
        let t1 = temps.alloc(Temp { ty: LirType::I64 }); // array_len t0
        let t2 = temps.alloc(Temp { ty: LirType::I64 }); // array_get t0[1]
        let t3 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_clone t0
        let t4 = temps.alloc(Temp { ty: point.clone() }); // aggregate (t2, t1)
        let t5 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_alloc (t4, t4)
        let t6 = temps.alloc(Temp { ty: point.clone() }); // array_get t5[1]
        let t7 = temps.alloc(Temp { ty: LirType::I64 }); // extract t6.1
        let t8 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_clone t5
        let t9 = temps.alloc(Temp { ty: LirType::I64 }); // t1 + t7

        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::ArrayAlloc {
                    out: t0,
                    elements: vec![Value::IntConst(1), Value::IntConst(2), Value::IntConst(3)],
                    array_type: int_array,
                },
                Instruction::Store {
                    local: numbers,
                    value: Value::Temp(t0),
                },
                Instruction::ArrayLen {
                    out: t1,
                    operand: Value::Local(numbers),
                    array_type: int_array,
                },
                Instruction::ArrayGet {
                    out: t2,
                    array: Value::Local(numbers),
                    index: Value::IntConst(1),
                    array_type: int_array,
                },
                Instruction::ArraySet {
                    array: Value::Local(numbers),
                    index: Value::IntConst(0),
                    value: Value::Temp(t2),
                    array_type: int_array,
                },
                Instruction::ArrayClone {
                    out: t3,
                    operand: Value::Local(numbers),
                    array_type: mutable_int_array,
                },
                Instruction::MakeAggregate {
                    out: t4,
                    elements: vec![Value::Temp(t2), Value::Temp(t1)],
                },
                Instruction::ArrayAlloc {
                    out: t5,
                    elements: vec![Value::Temp(t4), Value::Temp(t4)],
                    array_type: point_array,
                },
                Instruction::ArrayGet {
                    out: t6,
                    array: Value::Temp(t5),
                    index: Value::IntConst(0),
                    array_type: point_array,
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
                    array_type: point_array,
                },
                Instruction::ArrayClone {
                    out: t8,
                    operand: Value::Temp(t5),
                    array_type: mutable_point_array,
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
                    array_type: mutable_int_array,
                },
                Instruction::ArraySet {
                    array: Value::Temp(t8),
                    index: Value::IntConst(0),
                    value: Value::Temp(t6),
                    array_type: mutable_point_array,
                },
            ],
            terminator: Terminator::Return { value: None },
        });

        Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
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
                string: string_metadata(),
                arrays,
                layouts: Vec::new(),
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn emits_m5_arrays() {
        let module = arrays_module();
        let ir = ir_of(&module);
        assert!(
            ir.lines().any(|line| {
                line.contains("call ptr @scoop_rt_array_clone")
                    && line.contains("scoop_td_MutableArray<Int>")
            }),
            "Int clone must receive the target nominal descriptor:\n{ir}"
        );
        assert!(
            ir.lines().any(|line| {
                line.contains("call ptr @scoop_rt_array_clone")
                    && line.contains("scoop_td_MutableArray<Point>")
            }),
            "Point clone must receive the target nominal descriptor:\n{ir}"
        );
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
                gc_effect: GcEffect::Managed,
                symbol: symbol.to_string(),
                params: vec![MANAGED_PTR],
                return_ty: MANAGED_PTR,
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
        let t0 = temps.alloc(Temp { ty: MANAGED_PTR });
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
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![METADATA_PTR, MANAGED_PTR],
            return_ty: MANAGED_PTR,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        };

        Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![describe("Shape.describe"), describe("Point.describe"), main],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
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
                    // open class Shape: its sole ordinary virtual method.
                    TypeDescriptor {
                        name: "Shape".to_string(),
                        symbol: "scoop_td_Shape".to_string(),
                        size: 24,
                        align: 8,
                        scan: RefScan::References(vec![16]),
                        parent: None,
                        vtable: vec!["Shape.describe".to_string()],
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
                        vtable: vec!["Point.describe".to_string()],
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
            address_kind: PointerKind::Metadata,
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
            gc_effect: GcEffect::Managed,
            symbol: "Point.describe".to_string(),
            params: vec![MANAGED_PTR],
            return_ty: MANAGED_PTR,
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
        //   call_indirect t4[0](t3); println_int t2; println_boolean t7
        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: MANAGED_PTR });
        let t1 = temps.alloc(Temp { ty: METADATA_PTR });
        let t2 = temps.alloc(Temp { ty: LirType::I64 });
        let t3 = temps.alloc(Temp { ty: MANAGED_PTR });
        let t4 = temps.alloc(Temp { ty: METADATA_PTR });
        let t5 = temps.alloc(Temp {
            ty: LirType::Aggregate(vec![LirType::I64]),
        });
        let t6 = temps.alloc(Temp { ty: MANAGED_PTR });
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
                    slot: 0,
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
            gc_effect: GcEffect::Managed,
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
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![describe, main],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
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
                    vtable: vec!["Point.describe".to_string()],
                    itables: vec![],
                }],
            },
        }
    }

    #[test]
    fn emits_m6_heap_access_and_td_stubs() {
        let module = heap_module();
        let ir = ir_of(&module);
        assert!(
            ir.contains("@scoop_rt_allocation_context = external thread_local global ptr")
                && ir.contains("alloc.fast.0")
                && ir.contains("alloc.slow.0")
                && ir.contains("@scoop_runtime_finish_tlab_alloc")
                && ir.contains("@scoop_runtime_alloc_slow"),
            "managed allocation must expose an inline TLAB fast path and collecting fallback:\n{ir}"
        );
        assert!(
            !ir.contains("call ptr @scoop_rt_alloc"),
            "generated code must not route every allocation through the compatibility entry:\n{ir}"
        );
        assert!(
            ir.contains("and i64 %tlab_cursor_int, -128")
                && ir.contains("add i64 %tlab_line_base, 128"),
            "the inline allocator must use runtime's 128-byte Immix line boundary:\n{ir}"
        );
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
            gc_effect: GcEffect::Managed,
            symbol: "scoop.thrower".to_string(),
            params: vec![MANAGED_PTR],
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
        let t4 = temps.alloc(Temp { ty: RAW_PTR });
        let t5 = temps.alloc(Temp { ty: MANAGED_PTR });
        let t6 = temps.alloc(Temp {
            ty: LirType::ExceptionRecord,
        });
        let t7 = temps.alloc(Temp { ty: RAW_PTR });
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
            gc_effect: GcEffect::Managed,
            symbol: "scoop.eh_test".to_string(),
            params: vec![METADATA_PTR],
            return_ty: LirType::I64,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        };

        Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![thrower, eh_test],
            entry_symbol: "scoop.eh_test".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
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

    #[test]
    fn typed_intrinsic_string_supplies_the_only_descriptor_definition() {
        let ir = ir_of(&values_module());
        assert_eq!(
            ir.match_indices("@scoop_td_String =").count(),
            1,
            "String must have exactly one descriptor definition"
        );
        assert!(ir.contains("@scoop_td_String ="));
    }

    #[test]
    fn native_calls_publish_roots_transition_and_reload() {
        let mut extern_functions = Arena::default();
        let c_call = extern_functions.alloc(ExternFunction {
            source_name: "wait".to_string(),
            native_symbol: "native_wait".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: Vec::new(),
            return_type: LirType::Void,
            kind: ExternFunctionKind::C {
                bridge_symbol: "scoop_c_bridge_wait".to_string(),
                params: Vec::new(),
                return_type: scoop_lir::CType::Unit,
            },
        });
        let borrowed = extern_functions.alloc(ExternFunction {
            source_name: "borrowed".to_string(),
            native_symbol: "native_borrowed".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: Vec::new(),
            return_type: MANAGED_PTR,
            kind: ExternFunctionKind::Scoop {
                gc_effect: GcEffect::Managed,
            },
        });

        let mut safe_blocks = Arena::default();
        let safe_entry = safe_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::NativeCall {
                out: None,
                function: c_call,
                effect: scoop_lir::NativeCallEffect::NativeSafe,
                args: Vec::new(),
                roots: vec![scoop_lir::CallerRoot {
                    source: scoop_lir::CallerRootSource::Param(0),
                    scan: RefScan::References(vec![0]),
                }],
                result_scan: RefScan::None,
            }],
            terminator: Terminator::Return {
                value: Some(Value::Param(0)),
            },
        });
        let safe = Function {
            gc_effect: GcEffect::Managed,
            symbol: "safe_root".to_string(),
            params: vec![MANAGED_PTR],
            return_ty: MANAGED_PTR,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks: safe_blocks,
            entry: safe_entry,
        };

        let mut borrowed_temps = Arena::default();
        let result = borrowed_temps.alloc(Temp { ty: MANAGED_PTR });
        let mut borrowed_blocks = Arena::default();
        let borrowed_entry = borrowed_blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::NativeCall {
                out: Some(result),
                function: borrowed,
                effect: scoop_lir::NativeCallEffect::NativeBorrowed,
                args: Vec::new(),
                roots: Vec::new(),
                result_scan: RefScan::References(vec![0]),
            }],
            terminator: Terminator::Return {
                value: Some(Value::Temp(result)),
            },
        });
        let borrowed_function = Function {
            gc_effect: GcEffect::Managed,
            symbol: "borrowed_result".to_string(),
            params: Vec::new(),
            return_ty: MANAGED_PTR,
            locals: Arena::default(),
            temps: borrowed_temps,
            blocks: borrowed_blocks,
            entry: borrowed_entry,
        };

        let module = Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions,
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![safe, borrowed_function],
            entry_symbol: "safe_root".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: Vec::new(),
            },
        };

        let ir = ir_of(&module);
        assert!(ir.contains("@scoop_rt_push_caller_roots"));
        assert!(ir.contains("@scoop_rt_enter_native_safe"));
        assert!(ir.contains("@scoop_rt_leave_native_safe"));
        assert!(ir.contains("@scoop_rt_enter_native_borrowed"));
        assert!(ir.contains("@scoop_rt_leave_native_borrowed"));
        assert!(ir.contains("@scoop_rt_pop_caller_roots"));
        assert!(
            ir.contains("store ptr null, ptr %native_result"),
            "managed native result storage must be zero before publication:\n{ir}"
        );
        assert!(
            ir.contains("%caller_root_reload = load ptr, ptr %caller_root_param"),
            "published parameter roots must be reloaded after leave-native:\n{ir}"
        );
    }

    #[test]
    fn continuation_state_atomics_keep_their_llvm_orderings() {
        let mut temps = Arena::default();
        let loaded = temps.alloc(Temp { ty: LirType::I64 });
        let observed = temps.alloc(Temp { ty: LirType::I64 });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::AtomicLoad {
                    out: loaded,
                    object: Value::Param(0),
                    offset: 16,
                },
                Instruction::AtomicStore {
                    object: Value::Param(0),
                    offset: 16,
                    value: Value::IntConst(2),
                },
                Instruction::AtomicCompareExchange {
                    out: observed,
                    object: Value::Param(0),
                    offset: 16,
                    expected: Value::Temp(loaded),
                    replacement: Value::IntConst(6),
                },
            ],
            terminator: Terminator::Return {
                value: Some(Value::Temp(observed)),
            },
        });
        let module = Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
                symbol: "continuation_atomics".to_string(),
                params: vec![MANAGED_PTR],
                return_ty: LirType::I64,
                locals: Arena::default(),
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "continuation_atomics".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: Vec::new(),
            },
        };

        let ir = ir_of(&module);
        assert!(
            ir.contains("load atomic i64, ptr %atomic_field_ptr acquire"),
            "continuation state reads must be acquire loads:\n{ir}"
        );
        assert!(
            ir.contains("store atomic i64 2, ptr %atomic_field_ptr1 release"),
            "continuation state publication must be a release store:\n{ir}"
        );
        assert!(
            ir.contains("cmpxchg ptr %atomic_field_ptr2") && ir.contains("acq_rel acquire"),
            "continuation state claims must be acq_rel/acquire compare-exchange:\n{ir}"
        );
    }

    /// An M11-shaped module with both ordinary and suspend closure calls.
    /// Both return aggregates so the machine ABI has a leading result slot;
    /// the closure remains the first source-level argument and the suspend
    /// call carries its continuation immediately after it.
    fn closure_abi_module() -> Module {
        let ordinary_result_ty = LirType::Aggregate(vec![LirType::I64, MANAGED_PTR]);
        let suspend_result_ty = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
        let mut temps = Arena::default();
        let ordinary_result = temps.alloc(Temp {
            ty: ordinary_result_ty,
        });
        let suspend_result = temps.alloc(Temp {
            ty: suspend_result_ty,
        });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::CallIndirect {
                    out: Some(ordinary_result),
                    table: Value::Param(0),
                    slot: 2,
                    args: vec![Value::Param(0), Value::Param(1)],
                },
                Instruction::CallIndirect {
                    out: Some(suspend_result),
                    table: Value::Param(0),
                    slot: 2,
                    args: vec![Value::Param(0), Value::Param(2)],
                },
            ],
            terminator: Terminator::Return { value: None },
        });

        Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
                symbol: "scoop.closure_abi".to_string(),
                params: vec![MANAGED_PTR, LirType::I64, MANAGED_PTR],
                return_ty: LirType::Void,
                locals: Arena::default(),
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop.closure_abi".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays: Arena::new(),
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![],
            },
        }
    }

    #[test]
    fn closure_calls_preserve_hidden_abi_and_indirect_statepoints() {
        let module = closure_abi_module();
        let ir = ir_of(&module);
        assert_eq!(
            ir.matches("getelementptr ptr, ptr %0, i32 2").count(),
            2,
            "closure calls must load invoke from slot 2:\n{ir}"
        );
        assert!(
            ir.lines().any(|line| {
                line.contains("call void %fn_ptr")
                    && line.contains("(ptr %indirect_result, ptr %0, i64 %1)")
            }),
            "ordinary closure ABI must be (result slot, closure, arguments):\n{ir}"
        );
        assert!(
            ir.lines().any(|line| {
                line.contains("call void %fn_ptr") && line.contains("ptr %0, ptr %2)")
            }),
            "suspend closure ABI must keep continuation after the closure:\n{ir}"
        );
        assert!(
            ir.contains("load { i64, ptr }, ptr %indirect_result")
                && ir.contains("load { i64, i64 }, ptr %indirect_result"),
            "aggregate closure results must use typed return storage:\n{ir}"
        );

        let machine = host_target_machine().expect("target machine");
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine).expect("emit module");
        llvm.verify().expect("valid LLVM module");
        llvm.run_passes(
            "rewrite-statepoints-for-gc",
            &machine,
            PassBuilderOptions::create(),
        )
        .expect("rewrite-statepoints-for-gc pass");
        let rewritten = llvm.print_to_string().to_string();
        assert_eq!(
            rewritten
                .lines()
                .filter(|line| {
                    line.contains("call token")
                        && line.contains("gc.statepoint")
                        && line.contains("%fn_ptr")
                })
                .count(),
            2,
            "both managed indirect closure calls must become statepoints:\n{rewritten}"
        );
    }

    /// An M9-shaped module: a HeapStore and an ArraySet (both carry
    /// the write-barrier card mark) in the entry block, then a
    /// `while`-shaped loop (header ← body back edge) for the loop
    /// safepoint poll.
    fn barrier_module() -> Module {
        let mut arrays = Arena::new();
        let int_array = array_type(
            &mut arrays,
            "Array<Int>",
            scoop_lir::ArrayKind::Immutable,
            LirType::I64,
            8,
            8,
            RefScan::None,
        );
        let mut temps = Arena::default();
        let t0 = temps.alloc(Temp { ty: MANAGED_PTR }); // alloc result
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
                    array_type: int_array,
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
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
                symbol: "scoop_main".to_string(),
                params: vec![METADATA_PTR, MANAGED_PTR],
                return_ty: LirType::Void,
                locals: Arena::default(),
                temps,
                blocks,
                entry,
            }],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                string: string_metadata(),
                arrays,
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
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
    fn no_gc_functions_carry_neither_gc_strategy_nor_safepoint_polls() {
        let mut module = barrier_module();
        module.functions[0].gc_effect = GcEffect::NoGc;
        let ir = ir_of(&module);
        assert!(
            !ir.contains("gc \"statepoint-example\""),
            "NoGC function unexpectedly carries the GC strategy:\n{ir}"
        );
        assert_eq!(
            ir.matches("call void @scoop_rt_safepoint()").count(),
            0,
            "NoGC function unexpectedly polls safepoints:\n{ir}"
        );
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
        // One monotonic atomic card mark per heap store: the HeapStore and
        // the ArraySet element store.
        let marks = ir.matches(" = atomicrmw or ptr ").count();
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
        let nested_element_scan = RefScan::Sequence(vec![
            RefScan::References(vec![16]),
            RefScan::References(vec![8]),
        ]);
        let mut arrays = Arena::new();
        let ref_array_type = array_type(
            &mut arrays,
            "ArrayRef",
            scoop_lir::ArrayKind::Immutable,
            MANAGED_PTR,
            8,
            8,
            RefScan::References(vec![0]),
        );
        let nested_array_type = array_type(
            &mut arrays,
            "ArrayNested",
            scoop_lir::ArrayKind::Immutable,
            LirType::Aggregate(vec![LirType::I64, MANAGED_PTR, MANAGED_PTR]),
            24,
            8,
            nested_element_scan,
        );
        let mut temps = Arena::default();
        let array = temps.alloc(Temp { ty: MANAGED_PTR });
        let nested_array = temps.alloc(Temp { ty: MANAGED_PTR });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::ArrayAlloc {
                    out: array,
                    elements: vec![],
                    array_type: ref_array_type,
                },
                Instruction::ArrayAlloc {
                    out: nested_array,
                    elements: vec![],
                    array_type: nested_array_type,
                },
            ],
            terminator: Terminator::Return { value: None },
        });
        let module = Module {
            globals: Arena::default(),
            structs: Arena::default(),
            enums: Arena::default(),
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
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
                string: string_metadata(),
                arrays,
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
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
                        RefScan::References(vec![40, 48]),
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
                "@scoop_td_ArrayRef.element = private constant [2 x i64] [i64 1, i64 0]"
            ) && ir.contains(
                "@scoop_td_ArrayRef.refs = private constant [3 x i64] [i64 -1, i64 8, i64 ptrtoint (ptr @scoop_td_ArrayRef.element to i64)]"
            ),
            "reference-element array TD must carry SCOOP_REFS_ARRAY:\n{ir}"
        );
        assert!(
            ir.contains(
                "@scoop_td_ArrayNested.element.part.1 = private constant [2 x i64] [i64 1, i64 8]"
            ) && ir.contains(
                "@scoop_td_ArrayNested.element = private constant [4 x i64] [i64 -2, i64 2"
            ) && ir.contains(
                "@scoop_td_ArrayNested.refs = private constant [3 x i64] [i64 -1, i64 24, i64 ptrtoint (ptr @scoop_td_ArrayNested.element to i64)]"
            ),
            "aggregate array TD must wrap the recursive element scan:\n{ir}"
        );
        assert!(
            ir.contains(
                "@scoop_td_Holder.refs.part.1 = private constant [3 x i64] [i64 2, i64 40, i64 48]"
            ),
            "nested tagged enum scan must use fixed ref offsets:\n{ir}"
        );
        assert!(
            ir.contains("@scoop_td_Holder.refs = private constant [4 x i64] [i64 -2, i64 2"),
            "aggregate scan must compose fixed scans:\n{ir}"
        );
    }

    #[test]
    fn c_layout_matches_llvm_and_generated_c_assertions() {
        let mut structs = Arena::default();
        let inner = structs.alloc(StructDef {
            name: "Inner".to_string(),
            fields: vec![
                scoop_lir::StructField {
                    ty: LirType::I1,
                    layout: scoop_lir::FieldLayout {
                        offset: 0,
                        access_align: 1,
                    },
                },
                scoop_lir::StructField {
                    ty: LirType::I64,
                    layout: scoop_lir::FieldLayout {
                        offset: 1,
                        access_align: 1,
                    },
                },
            ],
            size: 16,
            align: 8,
            c_layout: Some(scoop_lir::CLayout {
                aligned: 8,
                packed: 1,
            }),
            interior_mutable: false,
        });
        let outer = structs.alloc(StructDef {
            name: "Outer".to_string(),
            fields: vec![
                scoop_lir::StructField {
                    ty: LirType::I1,
                    layout: scoop_lir::FieldLayout {
                        offset: 0,
                        access_align: 1,
                    },
                },
                scoop_lir::StructField {
                    ty: LirType::Struct(inner),
                    layout: scoop_lir::FieldLayout {
                        offset: 2,
                        access_align: 2,
                    },
                },
                scoop_lir::StructField {
                    ty: LirType::I64,
                    layout: scoop_lir::FieldLayout {
                        offset: 18,
                        access_align: 2,
                    },
                },
            ],
            size: 32,
            align: 16,
            c_layout: Some(scoop_lir::CLayout {
                aligned: 16,
                packed: 2,
            }),
            interior_mutable: true,
        });
        let mut enums = Arena::default();
        let wrapped = enums.alloc(EnumDef {
            name: "Wrapped".to_string(),
            repr: EnumRepr::Tagged {
                variants: vec![
                    EnumVariantRepr {
                        fields: vec![LirType::Struct(outer)],
                        field_offsets: vec![16],
                        slot_offset: 16,
                        slot_size: 32,
                        slot_align: 16,
                        gc_free: true,
                    },
                    EnumVariantRepr {
                        fields: Vec::new(),
                        field_offsets: Vec::new(),
                        slot_offset: 16,
                        slot_size: 0,
                        slot_align: 1,
                        gc_free: true,
                    },
                ],
                size: 48,
                align: 16,
            },
            scan: RefScan::None,
        });

        let mut arrays = Arena::new();
        let outer_array = array_type(
            &mut arrays,
            "ArrayOuter",
            scoop_lir::ArrayKind::Immutable,
            LirType::Struct(outer),
            32,
            16,
            RefScan::None,
        );

        let mut temps = Arena::default();
        let inner_value = temps.alloc(Temp {
            ty: LirType::Struct(inner),
        });
        let inner_field = temps.alloc(Temp { ty: LirType::I64 });
        let outer_value = temps.alloc(Temp {
            ty: LirType::Struct(outer),
        });
        let outer_field = temps.alloc(Temp {
            ty: LirType::Struct(inner),
        });
        let array = temps.alloc(Temp { ty: MANAGED_PTR });
        let loaded = temps.alloc(Temp {
            ty: LirType::Struct(outer),
        });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![
                Instruction::MakeAggregate {
                    out: inner_value,
                    elements: vec![Value::BoolConst(true), Value::IntConst(7)],
                },
                Instruction::ExtractValue {
                    out: inner_field,
                    aggregate: Value::Temp(inner_value),
                    index: 1,
                },
                Instruction::MakeAggregate {
                    out: outer_value,
                    elements: vec![
                        Value::BoolConst(false),
                        Value::Temp(inner_value),
                        Value::IntConst(9),
                    ],
                },
                Instruction::ExtractValue {
                    out: outer_field,
                    aggregate: Value::Temp(outer_value),
                    index: 1,
                },
                Instruction::ArrayAlloc {
                    out: array,
                    elements: vec![Value::Temp(outer_value)],
                    array_type: outer_array,
                },
                Instruction::ArrayGet {
                    out: loaded,
                    array: Value::Temp(array),
                    index: Value::IntConst(0),
                    array_type: outer_array,
                },
            ],
            terminator: Terminator::Return { value: None },
        });
        let mut module = Module {
            globals: Arena::default(),
            structs,
            enums,
            extern_functions: Arena::default(),
            native_globals: Arena::default(),
            callback_bridges: Arena::default(),
            foreign_callback_bridges: Arena::default(),
            functions: vec![Function {
                gc_effect: GcEffect::Managed,
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
                string: string_metadata(),
                arrays,
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 24,
                    align: 8,
                    fields: Vec::new(),
                    c_layout: None,
                    interior_mutable: false,
                    kind: LayoutKind::Plain {
                        scan: RefScan::None,
                    },
                }],
                type_descriptors: vec![],
            },
        };

        let machine = host_target_machine().expect("target machine");
        let target_data = machine.get_target_data();
        let context = Context::create();
        let outer_ty = basic_ty(
            &context,
            &module.structs,
            &module.enums,
            &LirType::Struct(outer),
        )
        .expect("outer LLVM type");
        assert_eq!(target_data.get_abi_size(&outer_ty), 32);
        assert_eq!(target_data.get_abi_alignment(&outer_ty), 16);
        let wrapped_ty = basic_ty(
            &context,
            &module.structs,
            &module.enums,
            &LirType::Enum(wrapped),
        )
        .expect("wrapped LLVM type");
        assert_eq!(target_data.get_abi_size(&wrapped_ty), 48);
        assert_eq!(target_data.get_abi_alignment(&wrapped_ty), 16);

        let assertions = c_layout_assertions(&module).expect("C assertions");
        assert!(
            assertions.find("scoop_c_layout_0").unwrap()
                < assertions.find("scoop_c_layout_1").unwrap(),
            "nested declaration must precede its user:\n{assertions}"
        );
        assert!(assertions.contains("offsetof(scoop_c_layout_1, _field_1) == 2"));
        assert!(assertions.contains("_Alignof(scoop_c_layout_1) == 16"));
        let source = std::env::temp_dir().join(format!(
            "scoop_c_layout_assertions_{}.c",
            std::process::id()
        ));
        std::fs::write(&source, &assertions).expect("write generated C");
        let status = std::process::Command::new("cc")
            .args(["-std=c11", "-fsyntax-only"])
            .arg(&source)
            .status()
            .expect("run C compiler");
        std::fs::remove_file(&source).ok();
        assert!(status.success(), "generated C assertions must compile");

        module.extern_functions.alloc(ExternFunction {
            source_name: "swap".to_string(),
            native_symbol: "native_swap".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
            params: vec![LirType::Struct(outer)],
            return_type: LirType::Struct(outer),
            kind: ExternFunctionKind::C {
                bridge_symbol: "scoop_c_bridge_0".to_string(),
                params: vec![scoop_lir::CType::Struct(outer)],
                return_type: scoop_lir::CType::Struct(outer),
            },
        });
        module.callback_bridges.alloc(scoop_lir::CallbackBridge {
            source_name: "swapCallback".to_string(),
            bridge_symbol: "scoop_callback_bridge_0".to_string(),
            trampoline_symbol: "scoop_c_callback_0".to_string(),
            params: vec![scoop_lir::CType::Struct(outer)],
            return_type: scoop_lir::CType::Struct(outer),
        });
        for (adapter, mode) in [
            (
                "scoop_foreign_callback_adapter_0",
                scoop_lir::ForeignCallbackMode::Reusable,
            ),
            (
                "scoop_foreign_callback_adapter_1",
                scoop_lir::ForeignCallbackMode::OneShot,
            ),
        ] {
            module
                .foreign_callback_bridges
                .alloc(scoop_lir::ForeignCallbackBridge {
                    adapter_symbol: adapter.to_string(),
                    trampoline_symbol: "scoop_foreign_callback_0".to_string(),
                    signature_symbol: "scoop_foreign_callback_signature_0".to_string(),
                    params: vec![scoop_lir::CType::Int, scoop_lir::CType::Pointer],
                    return_type: scoop_lir::CType::Int,
                    context_index: 1,
                    mode,
                });
        }
        let bridge = c_bridge_source(&module)
            .expect("C bridge")
            .expect("C extern needs a bridge");
        assert!(bridge.contains("extern scoop_c_layout_1 native_swap(scoop_c_layout_1);"));
        assert!(bridge.contains("void scoop_c_bridge_0(void *result, const void *arg0)"));
        assert!(bridge.contains("memcpy(result, &native_result, sizeof(native_result));"));
        assert!(
            bridge.contains("extern void scoop_callback_bridge_0(void *result, const void *arg0);")
        );
        assert!(bridge.contains("scoop_c_layout_1 scoop_c_callback_0(scoop_c_layout_1 arg0)"));
        assert!(bridge.contains("scoop_callback_bridge_0(&result, &arg0);"));
        assert_eq!(
            bridge
                .matches("const unsigned char scoop_foreign_callback_signature_0 = 0;")
                .count(),
            1,
            "one signature/context shape must emit one descriptor:\n{bridge}"
        );
        assert_eq!(
            bridge
                .matches("int64_t scoop_foreign_callback_0(int64_t arg0, void * arg1)")
                .count(),
            1,
            "registrations sharing a signature/context shape must share one trampoline:\n{bridge}"
        );
        assert!(bridge.contains("int64_t result = {0};"));
        assert!(bridge.contains("const void *arguments[1] = {&arg0};"));
        assert!(bridge.contains(
            "scoop_runtime_callback_invoke(arg1, &scoop_foreign_callback_signature_0, &result, arguments)"
        ));
        let bridge_source = std::env::temp_dir().join(format!(
            "scoop_c_foreign_callback_bridge_{}.c",
            std::process::id()
        ));
        std::fs::write(&bridge_source, &bridge).expect("write generated callback C");
        let status = std::process::Command::new("cc")
            .args(["-std=c11", "-fsyntax-only"])
            .arg(&bridge_source)
            .status()
            .expect("run C compiler");
        std::fs::remove_file(&bridge_source).ok();
        assert!(status.success(), "generated callback C must compile");

        let ir = ir_of(&module);
        assert!(
            ir.contains("getelementptr i8, ptr %managed_object, i32 32"),
            "over-aligned array data must start at offset 32:\n{ir}"
        );
        assert!(
            ir.contains(
                "@scoop_runtime_finish_tlab_alloc(ptr %tlab_object, ptr @scoop_td_ArrayOuter, i64 64)"
            ) && ir.contains("@scoop_runtime_alloc_slow(ptr @scoop_td_ArrayOuter, i64 64)"),
            "one 32-byte element plus the aligned 32-byte header must flow through the 64-byte TLAB check:\n{ir}"
        );
    }
}
