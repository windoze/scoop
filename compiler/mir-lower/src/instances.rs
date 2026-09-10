//! Persistent callable materialization metadata for concrete HIR functions.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct FunctionInstance {
    materialization: hir::CallableMaterialization,
    arguments: hir::NonEmptyVec<hir::TypeId>,
    symbol: hir::InstanceSymbol,
}

impl FunctionInstance {
    pub(super) const fn symbol(&self) -> hir::InstanceSymbol {
        self.symbol
    }

    pub(super) fn all_arguments(&self) -> &[hir::TypeId] {
        self.arguments.as_slice()
    }
}

pub(super) fn function_instance(function: &hir::Function) -> Option<FunctionInstance> {
    let hir::FunctionEmission::Materialized { arguments, symbol } = &function.emission else {
        return None;
    };
    Some(FunctionInstance {
        materialization: function.materialization,
        arguments: arguments.clone(),
        symbol: *symbol,
    })
}

impl Lowerer {
    pub(super) fn record_function_instance(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
        mir_id: mir::FunctionId,
    ) {
        let Some(instance) = function_instance(&module.functions[hir_id]) else {
            return;
        };
        self.instances.record(
            hir_id,
            mir_id,
            self.functions[mir_id].symbol.clone(),
            self.functions[mir_id].name.clone(),
            instance.materialization,
        );
    }
}

/// MIR references every substituted callable through its persistent
/// materialization; request-local source categories and origin arenas do not
/// survive the HIR boundary.
#[derive(Default)]
pub(super) struct InstanceRegistry {
    by_function: HashMap<hir::FunctionId, mir::MonomorphizedFunctionId>,
    pub(super) meta: Arena<mir::MonomorphizedFunction>,
}

impl InstanceRegistry {
    pub(super) fn record(
        &mut self,
        hir_function: hir::FunctionId,
        function: mir::FunctionId,
        symbol: String,
        display_name: String,
        materialization: hir::CallableMaterialization,
    ) -> mir::MonomorphizedFunctionId {
        let id = self.meta.alloc(mir::MonomorphizedFunction {
            function,
            symbol,
            display_name,
            materialization,
        });
        assert!(self.by_function.insert(hir_function, id).is_none());
        id
    }

    pub(super) fn get(&self, source: hir::FunctionId) -> Option<mir::MonomorphizedFunctionId> {
        self.by_function.get(&source).copied()
    }
}
