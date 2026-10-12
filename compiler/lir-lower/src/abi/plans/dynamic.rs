use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum InterfaceCallable {
    Local(mir::FunctionId),
    External(mir::ExternalCallableUseId),
}

impl InterfaceCallable {
    pub(crate) fn new(module: &mir::Module, callee: mir::DispatchCallee) -> Self {
        match callee {
            mir::DispatchCallee::User(id) => Self::Local(id),
            mir::DispatchCallee::Monomorphized(id) => {
                Self::Local(module.meta.instances[id].function)
            }
            mir::DispatchCallee::External(id) => Self::External(id),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FunctionDispatch {
    Closure,
    Bridge,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FunctionExecution {
    dispatch: FunctionDispatch,
    function_type: mir::FunctionTypeId,
    hidden_parameters: Vec<lir::LirType>,
    result: lir::LirType,
}

impl FunctionExecution {
    pub(crate) fn new(
        module: &mir::Module,
        dispatch: FunctionDispatch,
        function_type: mir::FunctionTypeId,
        arguments: &[mir::Expr],
        result: &mir::Type,
    ) -> Self {
        let signature = &module.function_types[function_type];
        Self {
            dispatch,
            function_type,
            hidden_parameters: arguments
                .iter()
                .skip(signature.arity())
                .map(|arg| lir_type(module, &arg.ty))
                .collect(),
            result: if !signature.is_suspend && signature.return_type == mir::Type::Unit {
                lir::LirType::Void
            } else {
                lir_type(module, result)
            },
        }
    }

    fn parameters(&self, module: &mir::Module) -> Vec<lir::LirType> {
        std::iter::once(lir::MANAGED_PTR)
            .chain(
                module.function_types[self.function_type]
                    .parameters()
                    .map(|ty| lir_type(module, ty)),
            )
            .chain(self.hidden_parameters.iter().cloned())
            .collect()
    }
}

type DynamicRequests = (
    HashMap<InterfaceCallable, lir::ScoopAbiRequestId>,
    HashMap<FunctionExecution, lir::ScoopAbiRequestId>,
);

pub(super) fn prepare(
    builder: &mut SignatureBuilder<'_>,
    module: &mir::Module,
    external: &la_arena::Arena<lir::ExternalCallable>,
    external_map: &HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
) -> StorageResult<DynamicRequests> {
    let mut interfaces = HashMap::new();
    let mut executions = HashMap::new();
    for function in module
        .functions
        .iter()
        .map(|(_, function)| function)
        .chain(module.release_hooks.iter().map(|(_, hook)| &hook.code))
    {
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                let mir::StatementKind::Call(effect) = &statement.kind else {
                    continue;
                };
                let (call, result) = match effect {
                    mir::CallEffect::Unit(call) => (call, &mir::Type::Unit),
                    mir::CallEffect::Value { destination, call } => {
                        (call, &function.body.locals[*destination].ty)
                    }
                };
                let (dispatch, function_type) = match call.target {
                    mir::CallTarget::Interface { callee, .. } => {
                        let key = InterfaceCallable::new(module, callee);
                        if let std::collections::hash_map::Entry::Vacant(entry) =
                            interfaces.entry(key)
                        {
                            let (parameters, result) = match key {
                                InterfaceCallable::Local(id) => {
                                    let function = &module.functions[id];
                                    (
                                        function
                                            .params
                                            .iter()
                                            .map(|p| lir_type(module, &p.ty))
                                            .collect::<Vec<_>>(),
                                        (function.return_ty != mir::Type::Unit)
                                            .then(|| lir_type(module, &function.return_ty)),
                                    )
                                }
                                InterfaceCallable::External(id) => {
                                    let signature = external[external_map[&id]].signature();
                                    (
                                        signature
                                            .arguments()
                                            .iter()
                                            .map(|arg| arg.logical_storage_type().clone())
                                            .collect(),
                                        signature.result().logical_storage_type().cloned(),
                                    )
                                }
                            };
                            entry.insert(
                                builder.scoop(
                                    std::iter::once(lir::MANAGED_PTR)
                                        .chain(parameters.into_iter().skip(1)),
                                    result,
                                )?,
                            );
                        }
                        continue;
                    }
                    mir::CallTarget::Closure { function_type, .. } => {
                        (FunctionDispatch::Closure, function_type)
                    }
                    mir::CallTarget::FunctionBridge { function_type, .. } => {
                        (FunctionDispatch::Bridge, function_type)
                    }
                    mir::CallTarget::Direct(_) | mir::CallTarget::Virtual { .. } => continue,
                };
                let key =
                    FunctionExecution::new(module, dispatch, function_type, &call.args, result);
                if let std::collections::hash_map::Entry::Vacant(entry) = executions.entry(key) {
                    let key = entry.key();
                    let signature = builder.scoop(
                        key.parameters(module),
                        (key.result != lir::LirType::Void).then(|| key.result.clone()),
                    )?;
                    entry.insert(signature);
                }
            }
        }
    }
    Ok((interfaces, executions))
}
