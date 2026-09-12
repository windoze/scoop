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
    ArrayType, ArrayTypeId, BinOp, CallableRef, DispatchEntry, EnumDefs, EnumRepr,
    ExternFunctionKind, ExternFunctions, Function, Global, GlobalInit, Instruction,
    IntegerBinaryOperation, IntegerComparison, IntegerDivRemOperation, IntegerKind,
    IntegerShiftOperation, IntegerSignedness, IntegerUnaryOperation, IntegerWidth,
    LirConstantImage, LirStaticInitialState, LirType, MachineScalarKind, Module, NativeGlobal,
    PointerKind, RefScan, StructDef, StructDefs, StructRepresentation, TempId, Terminator,
    TypeDescriptor, TypeDescriptorRef, TypeDescriptorScan, UnOp, Value,
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

mod abi;
mod artifact;
mod c_bridge;
mod declarations;
mod emission;
mod function;
mod image_roots;
mod initialization;
mod llvm_types;
mod module_context;
mod statepoint;
mod target;
mod type_descriptors;
mod validation;

pub use c_bridge::{c_bridge_source, c_layout_assertions};
pub(crate) use declarations::*;
#[cfg(test)]
pub(crate) use emission::emit_llvm_module;
#[cfg(test)]
pub(crate) use emission::host_target_machine;
pub use emission::{emit_object, render_llvm_ir};
use function::emit_function;
pub(crate) use llvm_types::*;
pub(crate) use module_context::*;
use target::ManagedAddressSpace;
pub use target::{
    LlvmVersion, ResolvedTargetProfile, TargetProfileId, ValidatedBackendProfile,
    ValidatedCBridgeToolchainProfile, ValidatedFinalLinkProfile, ValidatedRuntimeBuildProfile,
    linked_llvm_version,
};
pub(crate) use type_descriptors::{emit_ref_scan, emit_type_descriptors, type_descriptor_global};

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

#[cfg(test)]
mod tests;
