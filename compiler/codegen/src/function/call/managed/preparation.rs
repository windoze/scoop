use super::*;

pub(super) enum CallPreparation<'a, 'ctx> {
    Managed {
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
    },
    Native(PreparedNativeTransition<'a, 'ctx>),
    Ordinary,
}

impl<'ctx> CallPreparation<'_, 'ctx> {
    pub(super) fn managed_live(&self) -> Option<&MaterializedStatepointLive<'ctx>> {
        match self {
            Self::Managed { live, .. } => Some(live),
            Self::Native(_) | Self::Ordinary => None,
        }
    }
}

pub(super) struct PreparedNativeTransition<'a, 'ctx> {
    kind: NativeTransitionKind,
    roots: &'a [scoop_lir::CallerRoot],
    transition: NativeTransition<'ctx>,
    result: Option<RootStorage<'ctx>>,
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn prepare_call_protocol<'a>(
        &mut self,
        protocol: &CallProtocol<'a>,
        result: &TypedCallResult<'_>,
    ) -> Result<CallPreparation<'a, 'ctx>, CodegenError> {
        let (kind, roots, safepoint, result_root) = match protocol {
            CallProtocol::Managed { safepoint, live } => {
                return Ok(CallPreparation::Managed {
                    safepoint: *safepoint,
                    live: self.materialize_statepoint_live(live, *safepoint)?,
                });
            }
            CallProtocol::NativeSafe { safepoint, roots } => {
                if result_scan(result) != &RefScan::None {
                    return Err(CodegenError(format!(
                        "native-safe call @{} has a managed result",
                        self.function.symbol()
                    )));
                }
                (
                    NativeTransitionKind::Safe,
                    roots.as_slice(),
                    *safepoint,
                    None,
                )
            }
            CallProtocol::NativeBorrowed {
                safepoint,
                roots,
                result: publication,
            } => {
                let result_root = match *publication {
                    scoop_lir::NativeBorrowedResultPublication::DirectRooted { storage, scan }
                    | scoop_lir::NativeBorrowedResultPublication::IndirectResultRooted {
                        storage,
                        scan,
                    } => {
                        let ty = basic_ty(
                            self.context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            self.function.locals[storage].ty(),
                        )?;
                        let pointer = self.local_pointer(storage)?;
                        self.builder
                            .build_store(pointer, ty.const_zero())
                            .map_err(|error| {
                                CodegenError(format!("zero native result: {error}"))
                            })?;
                        Some((RootStorage { pointer, ty }, scan.as_ref_scan()))
                    }
                    scoop_lir::NativeBorrowedResultPublication::Void
                    | scoop_lir::NativeBorrowedResultPublication::ElidedZst
                    | scoop_lir::NativeBorrowedResultPublication::DirectGcFree
                    | scoop_lir::NativeBorrowedResultPublication::IndirectResultGcFree => None,
                };
                (
                    NativeTransitionKind::Borrowed,
                    roots.as_slice(),
                    *safepoint,
                    result_root,
                )
            }
            CallProtocol::ManagedInvoke { .. }
            | CallProtocol::NoGc
            | CallProtocol::NativeGcLeaf
            | CallProtocol::ReleaseNativeLeaf => return Ok(CallPreparation::Ordinary),
        };
        // The entry handshake consumes this protocol's safepoint identity.
        // Keep only the roots, frame and result needed to finish the same call.
        let transition = self.publish_native_roots(
            roots,
            result_root.map(|(storage, scan)| (storage.pointer, scan)),
            kind,
            safepoint,
        )?;
        Ok(CallPreparation::Native(PreparedNativeTransition {
            kind,
            roots,
            transition,
            result: result_root.map(|(storage, _)| storage),
        }))
    }

    pub(super) fn finish_prepared_native(
        &mut self,
        native: PreparedNativeTransition<'_, 'ctx>,
        result: &TypedCallResult<'_>,
        direct_value: Option<BasicValueEnum<'ctx>>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        if let (TypedCallResult::Direct { .. }, Some(value), Some(storage)) =
            (result, direct_value, native.result)
        {
            self.builder
                .build_store(storage.pointer, value)
                .map_err(|error| CodegenError(format!("store native result: {error}")))?;
        }
        self.leave_native_transition(&native.transition, native.kind)?;
        let reloaded_roots =
            self.reload_published_roots(native.roots.iter().map(|root| root.source))?;
        let result = native
            .result
            .map(|storage| {
                self.builder
                    .build_load(storage.ty, storage.pointer, "native_result_reload")
                    .map_err(|error| CodegenError(format!("reload rooted native result: {error}")))
            })
            .transpose()?;
        self.pop_native_roots(native.transition)?;
        self.restore_reloaded_roots(reloaded_roots)?;
        Ok(result)
    }
}
