//! Persistent LocalConcrete callable locations retained by MIR lowering.

use scoop_hir::concrete as hir;
use scoop_mir as mir;

#[derive(Default)]
pub(super) struct SourceCallableRegistry {
    entries: Vec<mir::SourceCallableMaterialization>,
}

impl SourceCallableRegistry {
    pub(super) fn record(
        &mut self,
        function: mir::FunctionId,
        materialization: hir::CallableMaterialization,
    ) {
        self.entries.push(mir::SourceCallableMaterialization::new(
            function,
            materialization,
        ));
    }

    pub(super) fn finish(self) -> mir::SourceCallableMaterializations {
        mir::SourceCallableMaterializations::checked(self.entries)
            .expect("MIR lowering records every source callable exactly once")
    }
}
