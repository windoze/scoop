use super::*;
use scoop_lir::{CallSite, CallerRootSource, InvokeSite, StatepointLiveSet};

fn debug_storage(source: CallerRootSource) -> DebugStorage {
    match source {
        CallerRootSource::Param(index) => DebugStorage::Parameter(index),
        CallerRootSource::Local(local) => DebugStorage::Local(local),
        CallerRootSource::Temp(temp) => DebugStorage::Temporary(temp),
    }
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn observe_debug_root(
        &self,
        source: CallerRootSource,
    ) -> Result<(), CodegenError> {
        let Some(debug) = &self.debug else {
            return Ok(());
        };
        let debug = debug.borrow();
        let storage = debug_storage(source);
        if debug.observes(storage) {
            // This slot already exists and was restored by the real GC path.
            // Describing its address adds no load, root, or lifetime extension.
            debug.declare(
                storage,
                self.root_source_storage(source)?.pointer,
                self.builder.get_insert_block().expect("root continuation"),
            );
        }
        Ok(())
    }

    pub(in crate::function) fn finish_debug_safepoint(
        &self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let Some(debug) = &self.debug else {
            return Ok(());
        };
        if instruction.safepoint().is_none() || !debug.borrow().is_full() {
            return Ok(());
        }
        let live_roots = |live: &StatepointLiveSet| {
            live.as_slice()
                .iter()
                .filter(|value| value.restore)
                .map(|value| value.source)
                .collect::<Vec<_>>()
        };
        let roots = match instruction {
            Instruction::ManagedPoll { site } => live_roots(&site.live),
            Instruction::Call {
                site: CallSite::Managed(site),
            } => live_roots(&site.live),
            Instruction::ArrayAllocDynamic { live, .. }
            | Instruction::ArrayAlloc { live, .. }
            | Instruction::ArrayAssembly { live, .. }
            | Instruction::StringConcat { live, .. }
            | Instruction::StringCopy { live, .. }
            | Instruction::ArrayClone { live, .. }
            | Instruction::BoxValue { live, .. } => live_roots(live),
            Instruction::Call {
                site: CallSite::NativeSafe(site),
            } => site
                .roots
                .as_slice()
                .iter()
                .map(|root| root.source)
                .collect(),
            Instruction::Call {
                site: CallSite::NativeBorrowed(site),
            } => site
                .roots
                .as_slice()
                .iter()
                .map(|root| root.source)
                .collect(),
            Instruction::NativeGlobalLoad { protocol, .. }
            | Instruction::NativeGlobalStore { protocol, .. }
            | Instruction::NativeGlobalAddress { protocol, .. } => {
                protocol.roots().iter().map(|root| root.source).collect()
            }
            Instruction::Invoke {
                site: InvokeSite::Managed(site),
            } => site
                .roots
                .as_slice()
                .iter()
                .filter(|root| root.normal_live)
                .map(|root| root.root.source)
                .collect(),
            _ => unreachable!("only safepoint instructions reach debug restoration"),
        };
        let kept = roots.iter().copied().map(debug_storage).collect::<Vec<_>>();
        debug.borrow().invalidate_managed_except(
            self.builder
                .get_insert_block()
                .expect("safepoint continuation"),
            &kept,
        );
        for root in roots {
            self.observe_debug_root(root)?;
        }
        Ok(())
    }
}
