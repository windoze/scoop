use super::*;

impl FunctionLowerer<'_> {
    fn bind_native_borrowed_call(
        &mut self,
        destination: lir::NativeBorrowedCallDestination,
        call: PendingTypedCall,
    ) -> StorageResult<lir::NativeBorrowedTypedCall> {
        use lir::NativeBorrowedTypedCall as Call;
        Ok(match call {
            PendingTypedCall::Void { signature, args } => {
                let target =
                    self.call_targets
                        .native_borrowed_targets
                        .void
                        .alloc(lir::CallTarget {
                            destination,
                            signature,
                        });
                Call::Void { target, args }
            }
            PendingTypedCall::ElidedZst {
                signature,
                out,
                args,
            } => {
                let target =
                    self.call_targets
                        .native_borrowed_targets
                        .elided_zst
                        .alloc(lir::CallTarget {
                            destination,
                            signature,
                        });
                Call::ElidedZst { target, out, args }
            }
            PendingTypedCall::Direct {
                signature,
                out,
                args,
            } => {
                let result = self.call_targets.direct_signatures[signature].result();
                let scan = lir::NonEmptyRefScan::new(result.scan().clone());
                let ty = result.storage_type().clone();
                let target =
                    self.call_targets
                        .native_borrowed_targets
                        .direct
                        .alloc(lir::CallTarget {
                            destination,
                            signature,
                        });
                match scan {
                    Some(scan) => Call::DirectRooted {
                        target,
                        out,
                        args,
                        scan,
                        storage: self.new_hidden_local(ty)?,
                    },
                    None => Call::DirectGcFree { target, out, args },
                }
            }
            PendingTypedCall::IndirectResult {
                signature,
                storage,
                args,
            } => {
                let signature_value = &self.call_targets.indirect_result_signatures[signature];
                assert_eq!(
                    signature_value.convention(),
                    lir::IndirectResultConvention::Sret,
                    "native-borrowed calls use Scoop sret storage"
                );
                let scan = lir::NonEmptyRefScan::new(signature_value.result().scan().clone());
                let target = self
                    .call_targets
                    .native_borrowed_targets
                    .indirect_result
                    .alloc(lir::CallTarget {
                        destination,
                        signature,
                    });
                match scan {
                    Some(scan) => Call::IndirectResultRooted {
                        target,
                        storage,
                        args,
                        scan,
                    },
                    None => Call::IndirectResultGcFree {
                        target,
                        storage,
                        args,
                    },
                }
            }
        })
    }

    pub(in crate::function) fn native_call_site(
        &mut self,
        destination: NativeCallDestination,
        call: PendingTypedCall,
    ) -> StorageResult<lir::CallSite> {
        Ok(match destination {
            NativeCallDestination::C(destination, mode) => {
                assert_eq!(
                    call.result_scan(&self.call_targets),
                    &lir::RefScan::None,
                    "native-safe C ABI results must be GC-free"
                );
                let call = bind_typed_call(&mut self.call_targets.c_targets, destination, call);
                if self.callable_body.release_owner().is_some() {
                    lir::CallSite::ReleaseNativeLeaf(lir::ReleaseNativeLeafCallSite { call })
                } else if mode == lir::CAbiCallMode::GcLeaf {
                    lir::CallSite::NativeGcLeaf(lir::NativeGcLeafCallSite { call })
                } else {
                    lir::CallSite::NativeSafe(lir::NativeSafeCallSite {
                        call,
                        safepoint: self.new_safepoint(lir::SafepointSiteRole::NativeSafeTransition),
                        roots: lir::NativeSafeRootSet::default(),
                    })
                }
            }
            NativeCallDestination::Borrowed(destination) => {
                let call = self.bind_native_borrowed_call(destination, call)?;
                lir::CallSite::NativeBorrowed(lir::NativeBorrowedCallSite {
                    call,
                    safepoint: self.new_safepoint(lir::SafepointSiteRole::NativeBorrowedTransition),
                    roots: lir::NativeBorrowedRootSet::default(),
                })
            }
        })
    }
}
