//! LLVM statepoint lowering and physical root planning.
//!
//! Typed LIR supplies site identity and logical leaves. Ordinary LLVM passes
//! retain those identities before RS4GC. Its actual relocation pairs determine
//! physical registration counts for runtime metadata and object emission.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::llvm_sys::core::{
    LLVMBuildCall2, LLVMBuildInvoke2, LLVMConstIntGetZExtValue, LLVMGetIntrinsicDeclaration,
    LLVMGetNumArgOperands, LLVMGetOperand, LLVMGlobalGetValueType, LLVMIsAConstantInt,
    LLVMLookupIntrinsicID,
};
use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::types::{AnyType, AsTypeRef, BasicTypeEnum, FunctionType};
use inkwell::values::{
    AsValueRef, BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallSiteValue, FunctionValue,
    InstructionOpcode, InstructionValue, PointerValue,
};
use la_arena::RawIdx;
use scoop_lir::GcEffect;

use crate::CodegenError;

mod bounded_loops;
use crate::ValidatedBackendProfile;
use crate::target::ManagedAddressSpace;

const GC_STRATEGY: &str = "statepoint-example";
pub(crate) const STATEPOINT_ROOT_IDENTITY_METADATA: &str = "scoop.statepoint-root-identity";

mod aggregate_values;
mod builders;
mod constant_roots;
mod edges;
mod finalization;
mod lowered;
pub(crate) use lowered::LoweredSafepoints;
mod manifest;
mod root_identity;
pub(crate) use root_identity::mark_root_identity;
mod storage;
pub(crate) use storage::mark_access as mark_storage_access;
mod policy;

pub(crate) use builders::{
    ManagedInvoke, ZeroLiveCall, build_managed_invoke, build_zero_live_call,
};
#[cfg(test)]
use manifest::ExpectedSite;
use manifest::{ExpectedRoot, ExpectedStatepoint};
pub(crate) use manifest::{ExpectedSafepoints, expectations};
#[cfg(test)]
pub(crate) use policy::rewrite;
pub(crate) use policy::{configure_function, lower, optimize};

/// Each safepoint and typed leaf restoration denotes one physical IR site.
pub(crate) fn mark_nonduplicable(context: &Context, call: CallSiteValue<'_>) {
    call.add_attribute(
        AttributeLoc::Function,
        context.create_enum_attribute(Attribute::get_named_enum_kind_id("noduplicate"), 0),
    );
}

#[cfg(test)]
#[path = "../statepoint_tests.rs"]
mod tests;
