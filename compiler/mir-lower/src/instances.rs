//! Persistent callable materialization metadata for concrete HIR functions.

use super::*;

pub(super) fn function_instance(function: &hir::Function) -> Option<hir::CallableMaterialization> {
    let hir::FunctionEmission::Materialized { .. } = &function.emission else {
        return None;
    };
    Some(function.materialization)
}

impl Lowerer {
    pub(super) fn record_function_instance(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
        mir_id: mir::FunctionId,
    ) {
        let Some(materialization) = function_instance(&module.functions[hir_id]) else {
            return;
        };
        self.instances.record(
            hir_id,
            mir_id,
            self.functions[mir_id].name.clone(),
            materialization,
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
        display_name: String,
        materialization: hir::CallableMaterialization,
    ) -> mir::MonomorphizedFunctionId {
        let id = self.meta.alloc(mir::MonomorphizedFunction {
            function,
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
