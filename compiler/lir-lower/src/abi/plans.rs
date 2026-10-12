//! Typed associations between real MIR callables and batch-local ABI requests.

use std::collections::HashMap;

use super::prepare::SignatureBuilder;
use super::*;

mod dynamic;
mod runtime;
pub(crate) use dynamic::{FunctionDispatch, FunctionExecution, InterfaceCallable};
pub(crate) use runtime::runtime_signature;

pub(crate) struct SignatureRequests {
    pub(crate) functions: HashMap<mir::FunctionId, lir::ScoopAbiRequestId>,
    pub(crate) scoop_externs: HashMap<mir::ExternFunctionId, lir::ScoopAbiRequestId>,
    pub(crate) c_externs: HashMap<mir::ExternFunctionId, lir::CAbiRequestId>,
    pub(crate) bridges: HashMap<mir::ExternFunctionId, lir::ScoopAbiRequestId>,
    pub(crate) interfaces: HashMap<InterfaceCallable, lir::ScoopAbiRequestId>,
    pub(crate) function_values: HashMap<FunctionExecution, lir::ScoopAbiRequestId>,
    pub(crate) runtime: HashMap<lir::RuntimeFunction, lir::ScoopAbiRequestId>,
    pub(crate) release: lir::ScoopAbiRequestId,
    pub(crate) root: lir::ScoopAbiRequestId,
    pub(crate) startup: lir::ScoopAbiRequestId,
}

pub(crate) struct Signatures {
    pub(crate) requests: SignatureRequests,
    pub(crate) plans: lir::AbiPlanSet,
}

impl Signatures {
    pub(crate) fn runtime(&self, function: lir::RuntimeFunction) -> &lir::ScoopAbiSignature {
        self.plans.scoop(self.requests.runtime[&function])
    }
}

pub(crate) fn prepare(
    context: &LoweringContext,
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    external: &la_arena::Arena<lir::ExternalCallable>,
    external_map: &HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
) -> StorageResult<(lir::AbiPlanningInput, SignatureRequests)> {
    let mut builder = SignatureBuilder::new(context, structs, enums);
    let functions = module
        .functions
        .iter()
        .map(|(id, function)| {
            builder
                .mir_scoop(
                    module,
                    function.params.iter().map(|p| &p.ty),
                    &function.return_ty,
                )
                .map(|signature| (id, signature))
        })
        .collect::<StorageResult<_>>()?;
    let mut scoop_externs = HashMap::new();
    let mut c_externs = HashMap::new();
    let mut bridges = HashMap::new();
    for (id, function) in module.extern_functions.iter() {
        match function.abi {
            mir::ExternAbi::Scoop => {
                scoop_externs.insert(
                    id,
                    builder.mir_scoop(
                        module,
                        function.params.iter(),
                        function.result.native_type(),
                    )?,
                );
            }
            mir::ExternAbi::C(_) => {
                let signature =
                    crate::externs::c_extern_signature(module, structs, enums, function);
                c_externs.insert(id, builder.c(&signature)?);
                if function.result.adaptation() == scoop_identity::CResultAdaptation::CaptureErrno {
                    let count = function.params.len()
                        + usize::from(function.result.native_type() != &mir::Type::Unit);
                    bridges.insert(
                        id,
                        builder.scoop(vec![lir::RAW_PTR; count], Some(lir::LirType::I32))?,
                    );
                }
            }
        }
    }
    let mut runtime = HashMap::new();
    for symbol in lir::RuntimeAbiSymbolV1::ALL {
        if let lir::RuntimeAbiSymbolV1::LirCall(function) = symbol {
            let (parameters, result) = runtime_signature(function);
            let signature =
                builder.scoop(parameters, (result != lir::LirType::Void).then_some(result))?;
            runtime.insert(function, signature);
        }
    }
    let (interfaces, function_values) =
        dynamic::prepare(&mut builder, module, external, external_map)?;
    let release = builder.scoop([lir::RAW_PTR], None)?;
    let root = builder.scoop(
        [lir::LirType::I32, lir::RAW_PTR, lir::RAW_PTR],
        Some(lir::LirType::I32),
    )?;
    let startup = builder.scoop([], Some(lir::LirType::I32))?;
    Ok((
        builder.input,
        SignatureRequests {
            functions,
            scoop_externs,
            c_externs,
            bridges,
            interfaces,
            function_values,
            runtime,
            release,
            root,
            startup,
        },
    ))
}
