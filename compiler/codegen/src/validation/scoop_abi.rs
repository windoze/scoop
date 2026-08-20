//! Defensive validation of the fully classified Scoop call ABI.
//!
//! Classification belongs to MIR -> LIR lowering.  This module only checks
//! that definitions, declarations and call sites preserve the exact typed
//! conventions already present in LIR.

use super::super::*;
use scoop_lir::{EnumDefId, StructDefId};
use std::collections::HashSet;

mod metadata;
use metadata::*;
mod call_sites;
use call_sites::*;
mod destinations;
use destinations::*;
mod runtime_calls;
use runtime_calls::*;

mod boxing;
mod c_calls;
mod enums;
mod pointer_storage;
mod structures;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageFacts {
    size: u64,
    align: u64,
    scan: RefScan,
}

pub(super) fn canonical_storage_scan(
    module: &Module,
    ty: &LirType,
    owner: &str,
) -> Result<RefScan, CodegenError> {
    AbiMetadataValidator::new(module)
        .storage_facts(ty, owner)
        .map(|facts| facts.scan)
}

struct AbiMetadataValidator<'a> {
    module: &'a Module,
    visiting_structs: HashSet<StructDefId>,
    visiting_enums: HashSet<EnumDefId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallProtocol {
    Managed,
    NoGc,
    NativeSafe,
    NativeGcLeaf,
    ReleaseNativeLeaf,
    NativeBorrowed,
}

impl CallProtocol {
    const fn name(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::NoGc => "no-gc",
            Self::NativeSafe => "native-safe",
            Self::NativeGcLeaf => "native-gc-leaf",
            Self::ReleaseNativeLeaf => "release-native-leaf",
            Self::NativeBorrowed => "native-borrowed",
        }
    }
}

pub(super) fn validate_scoop_abi(module: &Module) -> Result<(), CodegenError> {
    AbiMetadataValidator::new(module).validate_all()?;
    validate_scoop_extern_declarations(module)?;

    for function in module.callable_bodies() {
        validate_target_signature_references(function)?;
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::ManagedPoll { site } => validate_managed_poll(function, site)?,
                    Instruction::Call { site } => validate_call_site(module, function, site)?,
                    Instruction::Invoke { site } => validate_invoke_site(module, function, site)?,
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn validate_managed_poll(
    function: &Function,
    site: &scoop_lir::ManagedPollSite,
) -> Result<(), CodegenError> {
    if function.gc_effect != scoop_lir::GcEffect::Managed {
        return Err(managed_poll_error(
            function,
            "is only valid in a managed function",
        ));
    }

    let targets = &function.call_targets;
    let target_index = arena_index(site.target);
    if target_index >= targets.managed_targets.void.len() {
        return Err(managed_poll_error(
            function,
            format!("references invalid managed-void target {target_index}"),
        ));
    }
    let target = &targets.managed_targets.void[site.target];
    if target.destination
        != scoop_lir::ManagedCallDestination::runtime(scoop_lir::ManagedRuntimeFunction::Safepoint)
    {
        return Err(managed_poll_error(
            function,
            "target is not the managed safepoint runtime function",
        ));
    }

    let signature_index = arena_index(target.signature);
    if signature_index >= targets.void_signatures.len() {
        return Err(managed_poll_error(
            function,
            format!("target references invalid void signature {signature_index}"),
        ));
    }
    let signature = &targets.void_signatures[target.signature];
    if !signature.arguments().is_empty()
        || signature.calling_convention() != scoop_lir::CallingConvention::Cdecl
    {
        return Err(managed_poll_error(
            function,
            "target must use the exact `cdecl () -> void` safepoint ABI",
        ));
    }
    Ok(())
}

fn validate_target_signature_references(function: &Function) -> Result<(), CodegenError> {
    let targets = &function.call_targets;
    validate_protocol_target_signature_references(function, &targets.managed_targets)?;
    validate_protocol_target_signature_references(function, &targets.no_gc_targets)?;
    validate_protocol_target_signature_references(function, &targets.c_targets)?;
    validate_protocol_target_signature_references(function, &targets.native_borrowed_targets)
}

fn validate_protocol_target_signature_references<Destination>(
    function: &Function,
    targets: &scoop_lir::ProtocolCallTargets<Destination>,
) -> Result<(), CodegenError> {
    let signatures = &function.call_targets;
    for (_, target) in targets.void.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.void_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid void signature {index}"),
            ));
        }
    }
    for (_, target) in targets.elided_zst.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.elided_zst_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid elided-ZST signature {index}"),
            ));
        }
    }
    for (_, target) in targets.direct.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.direct_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid direct-result signature {index}"),
            ));
        }
    }
    for (_, target) in targets.indirect_result.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.indirect_result_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid indirect-result signature {index}"),
            ));
        }
    }
    Ok(())
}

fn validate_scoop_extern_declarations(module: &Module) -> Result<(), CodegenError> {
    for (_, function) in module.extern_functions.iter() {
        let ExternFunctionKind::Scoop { signature, .. } = &function.kind else {
            continue;
        };
        if function.calling_convention != signature.calling_convention() {
            return Err(CodegenError(format!(
                "Scoop extern `{}` carries a calling convention that disagrees with its authoritative ABI signature",
                function.source_name
            )));
        }
    }
    Ok(())
}

fn call_calling_convention(call: &scoop_lir::TypedCallView<'_>) -> scoop_lir::CallingConvention {
    match call {
        scoop_lir::TypedCallView::Void { signature, .. } => signature.calling_convention(),
        scoop_lir::TypedCallView::ElidedZst { signature, .. } => signature.calling_convention(),
        scoop_lir::TypedCallView::Direct { signature, .. } => signature.calling_convention(),
        scoop_lir::TypedCallView::IndirectResult { signature, .. } => {
            signature.calling_convention()
        }
    }
}

fn indirect_result_convention(
    call: &scoop_lir::TypedCallView<'_>,
) -> Option<scoop_lir::IndirectResultConvention> {
    match call {
        scoop_lir::TypedCallView::IndirectResult { signature, .. } => Some(signature.convention()),
        scoop_lir::TypedCallView::Void { .. }
        | scoop_lir::TypedCallView::ElidedZst { .. }
        | scoop_lir::TypedCallView::Direct { .. } => None,
    }
}

const fn call_argument_convention(argument: scoop_lir::AbiCallArgument) -> &'static str {
    match argument {
        scoop_lir::AbiCallArgument::ElidedZst(_) => "elided-ZST",
        scoop_lir::AbiCallArgument::Direct(_) => "direct",
        scoop_lir::AbiCallArgument::Indirect(_) => "indirect",
    }
}

const fn argument_convention(argument: &scoop_lir::AbiArgument) -> &'static str {
    match argument {
        scoop_lir::AbiArgument::ElidedZst(_) => "elided-ZST",
        scoop_lir::AbiArgument::Direct(_) => "direct",
        scoop_lir::AbiArgument::Indirect(_) => "indirect",
    }
}

fn call_error(function: &Function, detail: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("typed call @{}: {detail}", function.symbol()))
}

fn managed_poll_error(function: &Function, detail: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("managed poll @{}: {detail}", function.symbol()))
}

#[cfg(test)]
mod tests;
