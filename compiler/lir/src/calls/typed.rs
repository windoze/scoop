use super::*;

#[derive(Debug)]
pub enum TypedCall<Destination> {
    Void {
        target: VoidCallTargetId<Destination>,
        args: Vec<Value>,
    },
    Direct {
        target: DirectCallTargetId<Destination>,
        out: TempId,
        args: Vec<Value>,
    },
    IndirectResult {
        target: IndirectResultCallTargetId<Destination>,
        storage: LocalId,
        args: Vec<Value>,
    },
}

pub type ManagedTypedCall = TypedCall<ManagedCallDestination>;
pub type NoGcTypedCall = TypedCall<NoGcCallDestination>;
pub type NativeSafeTypedCall = TypedCall<NativeSafeCallDestination>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedCallResult {
    Void,
    Direct(TempId),
    IndirectResult(LocalId),
}

impl<Destination> TypedCall<Destination> {
    pub fn args(&self) -> &[Value] {
        match self {
            Self::Void { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match *self {
            Self::Direct { out, .. } => Some(out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result_storage(&self) -> Option<LocalId> {
        match *self {
            Self::IndirectResult { storage, .. } => Some(storage),
            Self::Void { .. } | Self::Direct { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match *self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::Direct { out, .. } => TypedCallResult::Direct(out),
            Self::IndirectResult { storage, .. } => TypedCallResult::IndirectResult(storage),
        }
    }
}

/// A Scoop-ABI result-root request used only while constructing LIR. The
/// completed `NativeBorrowedTypedCall` stores an aligned return-convention sum
/// and cannot expose this request independently from its call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBorrowedResultRoot {
    GcFree,
    Rooted { storage: LocalId },
}

#[derive(Debug)]
enum NativeBorrowedCall {
    Void(TypedCall<NativeBorrowedCallDestination>),
    DirectGcFree(TypedCall<NativeBorrowedCallDestination>),
    DirectRooted {
        call: TypedCall<NativeBorrowedCallDestination>,
        storage: LocalId,
        scan: NonEmptyRefScan,
    },
    IndirectResultGcFree(TypedCall<NativeBorrowedCallDestination>),
    IndirectResultRooted {
        call: TypedCall<NativeBorrowedCallDestination>,
        storage: LocalId,
        scan: NonEmptyRefScan,
    },
}

/// A native-borrowed call whose result publication is structurally aligned
/// with its return convention. Construction is private to `CallTargets`,
/// which derives the rooted scan from the target's authoritative signature.
#[derive(Debug)]
pub struct NativeBorrowedTypedCall(NativeBorrowedCall);

#[derive(Debug, Clone, Copy)]
pub enum NativeBorrowedResultPublication<'a> {
    Void,
    DirectGcFree,
    DirectRooted {
        storage: LocalId,
        scan: &'a NonEmptyRefScan,
    },
    IndirectResultGcFree,
    IndirectResultRooted {
        storage: LocalId,
        scan: &'a NonEmptyRefScan,
    },
}

pub struct NativeBorrowedTypedCallView<'a> {
    pub call: TypedCallView<'a>,
    pub result: NativeBorrowedResultPublication<'a>,
}

impl NativeBorrowedTypedCall {
    fn call(&self) -> &TypedCall<NativeBorrowedCallDestination> {
        match &self.0 {
            NativeBorrowedCall::Void(call)
            | NativeBorrowedCall::DirectGcFree(call)
            | NativeBorrowedCall::DirectRooted { call, .. }
            | NativeBorrowedCall::IndirectResultGcFree(call)
            | NativeBorrowedCall::IndirectResultRooted { call, .. } => call,
        }
    }

    pub fn args(&self) -> &[Value] {
        self.call().args()
    }

    pub fn direct_out(&self) -> Option<TempId> {
        self.call().direct_out()
    }

    pub fn result(&self) -> TypedCallResult {
        self.call().result()
    }

    pub fn view<'a>(&'a self, targets: &'a CallTargets) -> NativeBorrowedTypedCallView<'a> {
        let result = match &self.0 {
            NativeBorrowedCall::Void(_) => NativeBorrowedResultPublication::Void,
            NativeBorrowedCall::DirectGcFree(_) => NativeBorrowedResultPublication::DirectGcFree,
            NativeBorrowedCall::DirectRooted { storage, scan, .. } => {
                NativeBorrowedResultPublication::DirectRooted {
                    storage: *storage,
                    scan,
                }
            }
            NativeBorrowedCall::IndirectResultGcFree(_) => {
                NativeBorrowedResultPublication::IndirectResultGcFree
            }
            NativeBorrowedCall::IndirectResultRooted { storage, scan, .. } => {
                NativeBorrowedResultPublication::IndirectResultRooted {
                    storage: *storage,
                    scan,
                }
            }
        };
        NativeBorrowedTypedCallView {
            call: targets.typed_call_view(
                self.call(),
                &targets.native_borrowed_targets,
                NativeBorrowedCallDestination::view,
            ),
            result,
        }
    }
}

/// Protocol-neutral read-only projection used by dump and codegen after the
/// enclosing callsite arm has already selected the protocol-specific arena.
pub enum TypedCallView<'a> {
    Void {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a VoidCallSignature,
        args: &'a [Value],
    },
    Direct {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a DirectCallSignature,
        out: TempId,
        args: &'a [Value],
    },
    IndirectResult {
        target: u32,
        signature_id: u32,
        destination: CallDestination,
        signature: &'a IndirectResultCallSignature,
        storage: LocalId,
        args: &'a [Value],
    },
}

impl TypedCallView<'_> {
    pub fn target_raw(&self) -> u32 {
        match self {
            Self::Void { target, .. }
            | Self::Direct { target, .. }
            | Self::IndirectResult { target, .. } => *target,
        }
    }

    pub const fn return_convention_name(&self) -> &'static str {
        match self {
            Self::Void { .. } => "void",
            Self::Direct { .. } => "direct",
            Self::IndirectResult { .. } => "indirect",
        }
    }

    pub fn destination(&self) -> CallDestination {
        match self {
            Self::Void { destination, .. }
            | Self::Direct { destination, .. }
            | Self::IndirectResult { destination, .. } => *destination,
        }
    }

    pub fn args(&self) -> &[Value] {
        match self {
            Self::Void { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn result_scan(&self) -> &RefScan {
        match self {
            Self::Void { .. } => &RefScan::None,
            Self::Direct { signature, .. } => &signature.result_scan,
            Self::IndirectResult { signature, .. } => &signature.result.scan,
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match self {
            Self::Direct { out, .. } => Some(*out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result(&self) -> TypedCallResult {
        match self {
            Self::Void { .. } => TypedCallResult::Void,
            Self::Direct { out, .. } => TypedCallResult::Direct(*out),
            Self::IndirectResult { storage, .. } => TypedCallResult::IndirectResult(*storage),
        }
    }
}

impl CallTargets {
    pub fn typed_call_view<'a, Destination: Copy>(
        &'a self,
        call: &'a TypedCall<Destination>,
        targets: &'a ProtocolCallTargets<Destination>,
        destination_view: fn(Destination) -> CallDestination,
    ) -> TypedCallView<'a> {
        match call {
            TypedCall::Void { target, args } => {
                let target_value = &targets.void[*target];
                TypedCallView::Void {
                    target: target.into_raw().into_u32(),
                    signature_id: target_value.signature.into_raw().into_u32(),
                    destination: destination_view(target_value.destination),
                    signature: &self.void_signatures[target_value.signature],
                    args,
                }
            }
            TypedCall::Direct { target, out, args } => {
                let target_value = &targets.direct[*target];
                TypedCallView::Direct {
                    target: target.into_raw().into_u32(),
                    signature_id: target_value.signature.into_raw().into_u32(),
                    destination: destination_view(target_value.destination),
                    signature: &self.direct_signatures[target_value.signature],
                    out: *out,
                    args,
                }
            }
            TypedCall::IndirectResult {
                target,
                storage,
                args,
            } => {
                let target_value = &targets.indirect_result[*target];
                TypedCallView::IndirectResult {
                    target: target.into_raw().into_u32(),
                    signature_id: target_value.signature.into_raw().into_u32(),
                    destination: destination_view(target_value.destination),
                    signature: &self.indirect_result_signatures[target_value.signature],
                    storage: *storage,
                    args,
                }
            }
        }
    }

    /// Seal a native-borrowed typed call together with its result publication.
    /// The construction request is checked once at the MIR→LIR boundary;
    /// codegen receives only the aligned, immutable sum above.
    pub fn bind_native_borrowed_call(
        &self,
        call: TypedCall<NativeBorrowedCallDestination>,
        result_root: NativeBorrowedResultRoot,
    ) -> NativeBorrowedTypedCall {
        enum ResultShape {
            Void,
            Direct(RefScan),
            IndirectResult { storage: LocalId, scan: RefScan },
        }

        let shape = match self.typed_call_view(
            &call,
            &self.native_borrowed_targets,
            NativeBorrowedCallDestination::view,
        ) {
            TypedCallView::Void { .. } => ResultShape::Void,
            TypedCallView::Direct { signature, .. } => {
                ResultShape::Direct(signature.result_scan.clone())
            }
            TypedCallView::IndirectResult {
                signature, storage, ..
            } => ResultShape::IndirectResult {
                storage,
                scan: signature.result.scan.clone(),
            },
        };

        let call = match (shape, result_root) {
            (ResultShape::Void, NativeBorrowedResultRoot::GcFree) => NativeBorrowedCall::Void(call),
            (ResultShape::Direct(RefScan::None), NativeBorrowedResultRoot::GcFree) => {
                NativeBorrowedCall::DirectGcFree(call)
            }
            (ResultShape::Direct(scan), NativeBorrowedResultRoot::Rooted { storage }) => {
                let scan = NonEmptyRefScan::new(scan)
                    .expect("rooted native-borrowed direct result has a non-empty scan");
                NativeBorrowedCall::DirectRooted {
                    call,
                    storage,
                    scan,
                }
            }
            (
                ResultShape::IndirectResult {
                    scan: RefScan::None,
                    ..
                },
                NativeBorrowedResultRoot::GcFree,
            ) => NativeBorrowedCall::IndirectResultGcFree(call),
            (
                ResultShape::IndirectResult {
                    storage: call_storage,
                    scan,
                },
                NativeBorrowedResultRoot::Rooted { storage },
            ) => {
                assert_eq!(
                    storage, call_storage,
                    "native-borrowed indirect result must publish its own storage"
                );
                let scan = NonEmptyRefScan::new(scan)
                    .expect("rooted native-borrowed indirect result has a non-empty scan");
                NativeBorrowedCall::IndirectResultRooted {
                    call,
                    storage,
                    scan,
                }
            }
            _ => panic!("native-borrowed result publication disagrees with its typed signature"),
        };
        NativeBorrowedTypedCall(call)
    }
}
