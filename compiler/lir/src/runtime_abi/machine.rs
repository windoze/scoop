//! Machine signatures of the existing, closed compiler/runtime ABI.
use super::*;
use scoop_identity::GcEffect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerNativeValueV1 {
    Void,
    Pointer,
    Integer(u8),
    Float(crate::FloatKind),
    Boolean,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerNativeContractV1 {
    Function {
        parameters: &'static [CompilerNativeValueV1],
        result: CompilerNativeValueV1,
        effect: GcEffect,
    },
    Data {
        byte_size: u64,
        alignment: u64,
        thread_local: bool,
        mutable: bool,
    },
}

impl RuntimeAbiSymbolV1 {
    pub const fn machine_contract(self) -> CompilerNativeContractV1 {
        use CompilerNativeValueV1::{Boolean, Integer as I, Pointer as P, Void as V};
        use ManagedRuntimeFunction as M;
        use NoGcRuntimeFunction as N;
        match self {
            Self::LirCall(RuntimeFunction::Managed(function)) => {
                let (parameters, result): (&'static [_], _) = match function {
                    M::ContextPush => (&[P, P, P], P),
                    M::ContextEnsureRoot => (&[P], P),
                    M::Safepoint | M::GcCollect => (&[], V),
                    M::Alloc => (&[P, I(64)], P),
                    M::InitializationFailure | M::InitializationCycleMessage => (&[P], P),
                    M::StringConcat => (&[P, P], P),
                    M::InitializationEnter => (&[P], I(64)),
                    M::InitializationSucceed => (&[P], V),
                    M::InitializationFail => (&[P, P], V),
                };
                CompilerNativeContractV1::Function {
                    parameters,
                    result,
                    effect: GcEffect::Managed,
                }
            }
            Self::LirCall(RuntimeFunction::NoGc(function)) => {
                let (parameters, result): (&'static [_], _) = match function {
                    N::ContextTryGet => (&[P], P),
                    N::IsInstance => (&[P, P], Boolean),
                    N::ITableLookup | N::ITableFind => (&[P, P], P),
                    N::Pin | N::Unpin => (&[P], P),
                    N::GetHandle => (&[P], I(64)),
                    N::ReleaseHandle => (&[I(64)], P),
                    N::GcStats => (&[], I(64)),
                    N::StringCompare => (&[P, P], I(64)),
                    N::StringByteLength => (&[P], I(64)),
                    N::StringByteAt => (&[P, I(64)], I(8)),
                    N::StringCharacterAtByte => (&[P, I(64)], I(32)),
                    N::Trap | N::Throw => (&[P], V),
                    N::Rethrow => (&[], V),
                    N::UnboxZst => (&[P, P], V),
                    N::UnboxValue => (&[P, P, P], V),
                };
                leaf(parameters, result)
            }
            Self::CoreStringTypeDescriptor => CompilerNativeContractV1::Data {
                byte_size: 152,
                alignment: 8,
                thread_local: false,
                mutable: false,
            },
            Self::AllocationContext | Self::PollState | Self::ContextSlot => {
                CompilerNativeContractV1::Data {
                    byte_size: 8,
                    alignment: 8,
                    thread_local: true,
                    mutable: true,
                }
            }
            Self::GcEpoch => CompilerNativeContractV1::Data {
                byte_size: 8,
                alignment: 8,
                thread_local: false,
                mutable: true,
            },
            Self::PageMap => CompilerNativeContractV1::Data {
                byte_size: 32768,
                alignment: 8,
                thread_local: false,
                mutable: true,
            },
            Self::WorldPhase => CompilerNativeContractV1::Data {
                byte_size: 4,
                alignment: 4,
                thread_local: false,
                mutable: true,
            },
            Self::AllocateSlow => managed(&[P, I(64)], P),
            Self::WriteBarrier | Self::RecordDirtyCardGroup => leaf(&[P, I(64)], V),
            Self::PushPinFrame => leaf(&[P, P], V),
            Self::PopPinFrame => leaf(&[P], V),
            Self::FinishTlabAllocation | Self::PushCallerRoots | Self::PushCompilerRoots => {
                leaf(&[P, P, I(64)], V)
            }
            Self::BeginCatch | Self::CallbackRetain | Self::CallbackFailure => leaf(&[P], P),
            Self::EndCatch | Self::PopTopCompilerRoots => leaf(&[], V),
            Self::PopCallerRoots
            | Self::PopCompilerRoots
            | Self::LeaveNativeSafe
            | Self::LeaveNativeBorrowed
            | Self::CallbackRelease => leaf(&[P], V),
            Self::EnterNativeSafe | Self::EnterNativeBorrowed => leaf(&[P, I(64)], V),
            Self::CallbackRegister => leaf(&[P, P, P, I(32)], P),
            Self::CallbackState => leaf(&[P], I(32)),
            Self::CallbackInvoke => managed(&[P, P, P, P], I(32)),
        }
    }
}

impl TargetEhSupportV1 {
    pub const fn machine_contract(self) -> CompilerNativeContractV1 {
        use CompilerNativeValueV1::{Integer as I, Pointer as P, Void as V};
        match self {
            Self::ScoopPersonality => leaf(&[I(32), I(32), I(64), P, P], I(32)),
            Self::UnwindResume => leaf(&[P], V),
        }
    }
}

const fn leaf(
    parameters: &'static [CompilerNativeValueV1],
    result: CompilerNativeValueV1,
) -> CompilerNativeContractV1 {
    CompilerNativeContractV1::Function {
        parameters,
        result,
        effect: GcEffect::NoGc,
    }
}
const fn managed(
    parameters: &'static [CompilerNativeValueV1],
    result: CompilerNativeValueV1,
) -> CompilerNativeContractV1 {
    CompilerNativeContractV1::Function {
        parameters,
        result,
        effect: GcEffect::Managed,
    }
}
