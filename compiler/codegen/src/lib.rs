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
    TypeDescriptor, TypeDescriptorRef, UnOp, Value,
};

const SCAN_ARRAY: u64 = u64::MAX;
const SCAN_SEQUENCE: u64 = u64::MAX - 1;

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
mod atom_boundaries;
mod c_bridge;
mod c_bridge_emission;
mod callable_atom_boundaries;
mod callable_runtime_scans;
mod dataflow;
mod declarations;
mod elf_llvm;
mod elf_object;
mod emission;
mod function;
mod generated_c_atom_boundaries;
mod llvm_types;
mod module_context;
mod object_materialization;
mod object_partition;
mod runtime_metadata_v1;
mod shape_definitions;
mod statepoint;
mod target;
mod type_descriptors;
mod validation;

pub use c_bridge::{
    GeneratedCBridgeSourceSetV1, GeneratedCBridgeSourceUnitV1, c_layout_assertions,
    render_c_bridge_source_set,
};
pub use c_bridge_emission::{
    EmittedGeneratedCBridgeObjectMemberV1, EmittedGeneratedCBridgeObjectSetV1,
    emit_c_bridge_object_set,
};
pub(crate) use declarations::*;
#[cfg(test)]
pub(crate) use emission::emit_llvm_module;
#[cfg(test)]
pub(crate) use emission::host_target_machine;
pub use emission::{
    EmittedConeObjectMemberKindV1, EmittedConeObjectMemberV1, EmittedConeObjectSetV1,
    EmittedConeObjectSetV2, RenderedConeObjectModuleV1, emit_object_set, emit_object_set_v2,
    render_llvm_ir_members,
};
use function::emit_function;
pub(crate) use llvm_types::*;
mod floating;
pub(crate) use floating::{float_constant, float_type};
pub(crate) use module_context::*;
pub use object_materialization::EmittedStrongDigestPatchMaterializationV1;
pub use object_partition::{
    ScoopLirObjectKindV1, ScoopLirObjectPartitionError, ScoopLirObjectPartitionV1,
    ScoopLirObjectUnitSetV1,
};
pub use runtime_metadata_v1::{
    CallableRegistrationPatchSiteV1, EmittedConeImageSupportAtomV1, EmittedConeImageSupportAtomsV1,
    EmittedConeImageV1, EmittedEntryProductionV1, EmittedRootEntryV1,
    EmittedStaticStorageInitialStateV1, EmittedStaticStorageRelocationTableV1,
    EmittedStrongCallableRegistrationSetV1, EmittedStrongCallableRegistrationV1,
    EmittedStrongImmortalObjectRegistrationSetV1, EmittedStrongImmortalObjectRegistrationV1,
    EmittedStrongInitializationUnitRegistrationSetV1,
    EmittedStrongInitializationUnitRegistrationV1, EmittedStrongRuntimeMetadataV1,
    EmittedStrongSafepointRegistrationSetV1, EmittedStrongSafepointRegistrationV1,
    EmittedStrongStaticStorageRegistrationSetV1, EmittedStrongStaticStorageRegistrationV1,
    EmittedStrongTypeRegistrationSetV1, EmittedStrongTypeRegistrationV1,
    InitializationRegistrationPatchSiteV1, ProvisionalStrongDigestPatchLocationV1,
    RootEntryPatchSiteV1, RuntimeImagePatchSiteV1, SafepointRegistrationPatchSiteV1,
    StaticStorageRegistrationPatchSiteV1, TypeRegistrationPatchSiteV1,
};
use target::ManagedAddressSpace;
pub use target::{LlvmVersion, TargetProfileId, ValidatedBackendProfile, linked_llvm_version};
pub(crate) use type_descriptors::{TypeDescriptorGlobals, type_descriptor_global};

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

mod metadata_sections;
