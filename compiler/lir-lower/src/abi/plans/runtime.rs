//! Logical storage projections of the compiler's fixed runtime operations.

use scoop_lir as lir;

pub(crate) fn runtime_signature(
    function: lir::RuntimeFunction,
) -> (Vec<lir::LirType>, lir::LirType) {
    use lir::{LirType as T, MANAGED_PTR as P, METADATA_PTR as M, RAW_PTR as R};
    use lir::{
        ManagedRuntimeFunction as Managed, NoGcRuntimeFunction as Leaf, RuntimeFunction as F,
    };
    match function {
        F::Managed(function) => match function {
            Managed::ContextPush => (vec![R, P, M], P),
            Managed::ContextFork => (vec![P, M], P),
            Managed::ContextEnsureRoot => (vec![M], P),
            Managed::Safepoint | Managed::GcCollect => (vec![], T::Void),
            Managed::Alloc => (
                vec![M, T::MachineScalar(lir::MachineScalarKind::ByteSize)],
                P,
            ),
            Managed::BoxZst => (vec![M], P),
            Managed::BoxValue => (vec![M, R], P),
            Managed::StringConcat => (vec![P, P], P),
            Managed::InitializationEnter => (
                vec![M],
                T::MachineScalar(lir::MachineScalarKind::InitializationOutcome),
            ),
            Managed::InitializationSucceed => (vec![M], T::Void),
            Managed::InitializationFail => (vec![M, P], T::Void),
            Managed::InitializationFailure | Managed::InitializationCycleMessage => (vec![M], P),
        },
        F::NoGc(function) => match function {
            Leaf::ContextTryGet => (vec![R], P),
            Leaf::ContextRestore => (vec![P, P], T::Void),
            Leaf::ContextSnapshot | Leaf::ContextCurrent => (vec![], P),
            Leaf::ContextEnter => (vec![P], P),
            Leaf::ContextLeave => (vec![P], T::Void),
            Leaf::IsInstance => (vec![P, M], T::I1),
            Leaf::ITableLookup => (vec![M, M], M),
            Leaf::Pin | Leaf::GetHandle => (vec![P], T::I64),
            Leaf::Unpin | Leaf::ReleaseHandle => (vec![T::I64], P),
            Leaf::GcStats => (vec![], T::I64),
            Leaf::StringCompare => (vec![P, P], T::I64),
            Leaf::StringByteLength => (vec![P], T::I64),
            Leaf::StringByteAt => (vec![P, T::I64], T::I8),
            Leaf::StringCharacterAtByte => (vec![P, T::I64], T::I32),
            Leaf::Trap => (vec![R], T::Void),
            Leaf::Throw => (vec![P], T::Void),
            Leaf::Rethrow => (vec![], T::Void),
            Leaf::UnboxZst => (vec![P, M], T::Void),
            Leaf::UnboxValue => (vec![P, M, R], T::Void),
            Leaf::PushRecursiveRegion => (vec![R, R, T::I64], T::Void),
            Leaf::PopRecursiveRegion => (vec![R], T::Void),
        },
    }
}
