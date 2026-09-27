use super::*;

impl Writer<'_, '_> {
    pub(super) fn call(&mut self, site: &CallSite) -> Result {
        let targets = &self.function.call_targets;
        match site {
            CallSite::Managed(site) => {
                let call = targets.typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    ManagedCallDestination::view,
                );
                record!(self, 1; self.typed_call(call), self.safepoint(site.safepoint), self.live(&site.live))
            }
            CallSite::NoGc(site) => {
                let call = targets.typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    NoGcCallDestination::view,
                );
                record!(self, 2; self.typed_call(call))
            }
            CallSite::NativeSafe(site) => {
                let call = targets.typed_call_view(
                    &site.call,
                    &targets.native_safe_targets,
                    NativeSafeCallDestination::view,
                );
                record!(self, 3; self.typed_call(call), self.safepoint(site.safepoint), self.caller_roots(site.roots.as_slice()))
            }
            CallSite::NativeBorrowed(site) => {
                let view = site.call.view(targets);
                record!(self, 4; self.typed_call(view.call), self.borrowed_result(view.result), self.safepoint(site.safepoint), self.caller_roots(site.roots.as_slice()))
            }
        }
    }

    pub(super) fn invoke(&mut self, site: &InvokeSite) -> Result {
        let targets = &self.function.call_targets;
        match site {
            InvokeSite::Managed(site) => {
                let call = targets.typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    ManagedCallDestination::view,
                );
                record!(self, 1; self.typed_call(call), self.safepoint(site.safepoint), self.exceptional_roots(&site.roots), self.block(site.normal), self.block(site.unwind))
            }
            InvokeSite::NoGc(site) => {
                let call = targets.typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    NoGcCallDestination::view,
                );
                record!(self, 2; self.typed_call(call), self.block(site.normal), self.block(site.unwind))
            }
        }
    }

    pub(super) fn poll(&mut self, site: &ManagedPollSite) -> Result {
        let targets = &self.function.call_targets;
        let target = &targets.managed_targets.void[site.target];
        let signature = &targets.void_signatures[target.signature];
        record!(self, 1; self.void_call(target.destination.view(), signature, &[]), self.safepoint(site.safepoint), self.live(&site.live))
    }

    fn typed_call(&mut self, call: TypedCallView<'_>) -> Result {
        match call {
            TypedCallView::Void {
                destination,
                signature,
                args,
                ..
            } => self.void_call(destination, signature, args),
            TypedCallView::ElidedZst {
                destination,
                signature,
                out,
                args,
                ..
            } => {
                record!(self, 2; self.temp(out), self.destination(destination), self.id(&signature.calling_convention()), self.arguments(signature.arguments()), self.abi_zst(signature.result()), self.call_arguments(args))
            }
            TypedCallView::Direct {
                destination,
                signature,
                out,
                args,
                ..
            } => {
                record!(self, 3; self.temp(out), self.destination(destination), self.id(&signature.calling_convention()), self.arguments(signature.arguments()), self.abi_value(signature.result()), self.call_arguments(args))
            }
            TypedCallView::IndirectResult {
                destination,
                signature,
                storage,
                args,
                ..
            } => {
                let convention = match signature.convention() {
                    IndirectResultConvention::ScoopSret => 1,
                    IndirectResultConvention::CStoragePointer => 2,
                };
                record!(self, 4; self.local(storage), self.destination(destination), self.id(&signature.calling_convention()), self.arguments(signature.arguments()), self.abi_value(signature.result()), self.u(convention), self.call_arguments(args))
            }
        }
    }

    fn void_call(
        &mut self,
        destination: CallDestination,
        signature: &VoidCallSignature,
        args: &[AbiCallArgument],
    ) -> Result {
        record!(self, 1; self.destination(destination), self.id(&signature.calling_convention()), self.arguments(signature.arguments()), self.call_arguments(args))
    }

    pub(super) fn destination(&mut self, destination: CallDestination) -> Result {
        match destination {
            CallDestination::Local(id) => record!(self, 1; self.local_function(id)),
            CallDestination::External(id) => {
                record!(self, 1; self.id(&self.module.meta.external_callables[id].body()))
            }
            CallDestination::Runtime(function) => {
                record!(self, 2; self.u(function.wire_family_tag()), self.u(function.wire_function_tag()))
            }
            CallDestination::Extern(id) => {
                let declaration = &self.module.extern_functions[id];
                match &declaration.kind {
                    ExternFunctionKind::C { bridge, signature } => {
                        record!(self, 3; self.id(&bridge.unit()), self.c_signature(signature))
                    }
                    ExternFunctionKind::Scoop {
                        gc_effect,
                        signature,
                    } => {
                        record!(self, 4; self.text(&declaration.library), self.text(&declaration.native_symbol), self.id(&declaration.calling_convention), self.gc_effect(*gc_effect), self.signature(signature))
                    }
                }
            }
            CallDestination::Dispatch { table, slot } => {
                let slot = self.function.call_targets.dispatch_slots[slot];
                let kind = match slot.kind {
                    DispatchKind::Virtual => 1,
                    DispatchKind::Interface => 2,
                    DispatchKind::Closure => 3,
                    DispatchKind::FunctionBridge => 4,
                };
                record!(self, 5; self.value(table), self.u(kind), self.u(u64::from(slot.index)))
            }
        }
    }

    fn borrowed_result(&mut self, result: NativeBorrowedResultPublication<'_>) -> Result {
        match result {
            NativeBorrowedResultPublication::Void => record!(self, 1;),
            NativeBorrowedResultPublication::ElidedZst => record!(self, 2;),
            NativeBorrowedResultPublication::DirectGcFree => record!(self, 3;),
            NativeBorrowedResultPublication::DirectRooted { storage, scan } => {
                record!(self, 4; self.local(storage), self.scan(scan.as_ref_scan()))
            }
            NativeBorrowedResultPublication::IndirectResultGcFree => record!(self, 5;),
            NativeBorrowedResultPublication::IndirectResultRooted { storage, scan } => {
                record!(self, 6; self.local(storage), self.scan(scan.as_ref_scan()))
            }
        }
    }
}
