//! Persistent LocalConcrete value locations retained by MIR lowering.

use scoop_hir::concrete as hir;
use scoop_mir as mir;

#[derive(Default)]
pub(super) struct SourceLocalValueRegistry {
    entries: Vec<mir::SourceLocalValueIdentity>,
}

impl SourceLocalValueRegistry {
    pub(super) fn record(
        &mut self,
        function: mir::FunctionId,
        local: mir::LocalId,
        identity: &hir::LocalValueIdentityRecord,
    ) {
        self.entries.push(mir::SourceLocalValueIdentity::new(
            function,
            local,
            identity.clone(),
        ));
    }

    pub(super) fn finish(self) -> mir::SourceLocalValueIdentities {
        mir::SourceLocalValueIdentities::checked(self.entries)
            .expect("MIR lowering records each source local location exactly once")
    }
}
