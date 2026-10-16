//! LLVM statepoint lowering and physical root planning.
//!
//! Typed LIR supplies site identity and logical leaves. Ordinary LLVM passes
//! determine their final SSA equivalence classes before RS4GC. The same plan
//! drives runtime metadata and object emission.

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
use crate::ValidatedBackendProfile;
use crate::target::ManagedAddressSpace;

const GC_STRATEGY: &str = "statepoint-example";
pub(crate) const STATEPOINT_ROOT_IDENTITY_METADATA: &str = "scoop.statepoint-root-identity";

mod builders;
mod constant_roots;
mod finalization;
mod manifest;
mod root_identity;
pub(crate) use root_identity::mark_root_identity;
mod policy;

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

#[cfg(test)]
#[path = "../statepoint_tests.rs"]
mod tests;
