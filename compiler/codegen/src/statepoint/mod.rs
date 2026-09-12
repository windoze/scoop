//! LLVM statepoint rewrite policy and defensive verification.
//!
//! Typed LIR is the sole source of safepoint identity and live-root shape.
//! This module records that complete manifest, configures LLVM 22.1, and
//! rejects rewritten IR that does not match it exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::slice;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::llvm_sys::core::{
    LLVMBuildCall2, LLVMBuildInvoke2, LLVMConstIntGetZExtValue, LLVMGetCalledValue, LLVMGetGC,
    LLVMGetIntrinsicDeclaration, LLVMGetNumArgOperands, LLVMGetOperand, LLVMGetValueName2,
    LLVMGlobalGetValueType, LLVMIsAAllocaInst, LLVMIsAConstantInt, LLVMLookupIntrinsicID,
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
use crate::ValidatedBackendProfile;
use crate::target::ManagedAddressSpace;

const GC_STRATEGY: &str = "statepoint-example";
pub(crate) const TYPED_MANAGED_POINTER_BOUNDARY_METADATA: &str =
    "scoop.typed-managed-pointer-boundary";
pub(crate) const STATEPOINT_ROOT_IDENTITY_METADATA: &str = "scoop.statepoint-root-identity";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypedManagedPointerBoundary {
    AllocationResult,
    CardAddress,
}

impl TypedManagedPointerBoundary {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::AllocationResult => "allocation-result",
            Self::CardAddress => "card-address",
        }
    }
}

pub(crate) fn mark_root_identity(
    context: &Context,
    instruction: InstructionValue<'_>,
    safepoint: scoop_lir::SafepointId,
    source: scoop_lir::CallerRootSource,
    byte_offset: u64,
) -> Result<(), CodegenError> {
    let (source_kind, source_index) = match source {
        scoop_lir::CallerRootSource::Param(index) => (0, index),
        scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
        scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
    };
    let i64_type = context.i64_type();
    instruction
        .set_metadata(
            context.metadata_node(&[
                i64_type.const_int(safepoint.get(), false).into(),
                i64_type.const_int(source_kind, false).into(),
                i64_type.const_int(source_index.into(), false).into(),
                i64_type.const_int(byte_offset, false).into(),
            ]),
            context.get_kind_id(STATEPOINT_ROOT_IDENTITY_METADATA),
        )
        .map_err(|error| {
            CodegenError(format!(
                "mark statepoint {} root identity: {error}",
                safepoint.get()
            ))
        })
}

mod builders;
mod manifest;
mod policy;
mod provenance;
mod verifier;

pub(crate) use builders::{
    ManagedInvoke, ZeroLiveCall, build_managed_invoke, build_zero_live_call,
};
#[cfg(test)]
use manifest::ExpectedSite;
use manifest::{ExpectedRoot, ExpectedStatepoint};
pub(crate) use manifest::{ExpectedSafepoints, expectations};
pub(crate) use policy::{configure_function, rewrite};
use verifier::ObservedStatepoint;
pub(crate) use verifier::verify_rewritten;

#[cfg(test)]
#[path = "../statepoint_tests.rs"]
mod tests;
