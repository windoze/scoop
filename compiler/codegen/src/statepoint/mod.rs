//! LLVM statepoint rewrite policy and defensive verification.
//!
//! Typed LIR supplies site identity and logical leaves. Ordinary LLVM passes
//! determine their final SSA equivalence classes before RS4GC. The same plan
//! drives relocation checks, runtime metadata and object verification.

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
    LLVMGlobalGetValueType, LLVMIsAConstantInt, LLVMLookupIntrinsicID,
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

mod builders;
mod finalization;
mod manifest;
mod root_identity;
pub(crate) use root_identity::mark_root_identity;
mod policy;
mod provenance;
mod verifier;

pub(crate) use builders::{
    ManagedInvoke, ZeroLiveCall, build_managed_invoke, build_zero_live_call,
};
#[cfg(test)]
use manifest::ExpectedSite;
use manifest::{ExpectedRoot, ExpectedRoots, ExpectedStatepoint};
pub(crate) use manifest::{ExpectedSafepoints, expectations};
#[cfg(test)]
pub(crate) use policy::rewrite;
pub(crate) use policy::{configure_function, lower, optimize};
use verifier::ObservedStatepoint;
pub(crate) use verifier::verify_rewritten;

#[cfg(test)]
#[path = "../statepoint_tests.rs"]
mod tests;
