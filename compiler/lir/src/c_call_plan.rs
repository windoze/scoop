//! Physical outbound C calls, independent of the caller's GC protocol.

use std::num::NonZeroU64;

use crate::{
    AbiArgument, AbiDirectParts, AbiDirectValue, AbiReturn, AbiValue, CallingConvention,
    GeneratedBridgeEntryIdentity, ScoopAbiSignature,
};

#[derive(Debug)]
pub enum CAbiCallPlan {
    Direct(DirectCSignature),
    StorageBridge {
        entry: Box<GeneratedBridgeEntryIdentity>,
        result: scoop_identity::CResultAdaptation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectCSignature {
    pub params: Vec<DirectCArgument>,
    pub result: DirectCReturn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectCArgument {
    Scalar(DirectCValue),
    DirectParts(AbiDirectParts),
    Indirect {
        value: AbiValue,
        passing: CIndirectPassing,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CIndirectPassing {
    ByValue { alignment: NonZeroU64 },
    CallerCopy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectCReturn {
    Void,
    Value(DirectCValue),
    DirectParts(AbiDirectParts),
    Indirect(AbiValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectCValue {
    pub storage: AbiValue,
    pub extension: CIntegerExtension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CIntegerExtension {
    None,
    Sign,
    Zero,
}

impl DirectCArgument {
    pub fn abi_argument(&self) -> AbiArgument {
        match self {
            Self::Scalar(value) => AbiArgument::Direct(value.storage.clone().into()),
            Self::DirectParts(parts) => {
                AbiArgument::Direct(AbiDirectValue::DirectParts(parts.clone()))
            }
            Self::Indirect { value, .. } => AbiArgument::Indirect(value.clone()),
        }
    }

    pub fn value(&self) -> &AbiValue {
        match self {
            Self::Scalar(value) => &value.storage,
            Self::DirectParts(parts) => parts.value(),
            Self::Indirect { value, .. } => value,
        }
    }
}

impl DirectCReturn {
    pub fn abi_return(&self) -> AbiReturn {
        match self {
            Self::Void => AbiReturn::UnitVoid,
            Self::Value(value) => AbiReturn::Direct(value.storage.clone().into()),
            Self::DirectParts(parts) => {
                AbiReturn::Direct(AbiDirectValue::DirectParts(parts.clone()))
            }
            Self::Indirect(value) => AbiReturn::Indirect(value.clone()),
        }
    }
}

impl DirectCSignature {
    /// Reuse the typed call storage and carrier model. C attributes remain in
    /// this plan, since indirect C parameters are not uniformly Scoop byval.
    pub fn abi_signature(&self) -> ScoopAbiSignature {
        ScoopAbiSignature::new(
            self.params
                .iter()
                .map(DirectCArgument::abi_argument)
                .collect(),
            self.result.abi_return(),
            CallingConvention::Cdecl,
        )
    }

    pub fn dump(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|argument| match argument {
                DirectCArgument::Scalar(value) => scalar_name(value),
                DirectCArgument::DirectParts(parts) => parts_name(parts),
                DirectCArgument::Indirect { value, passing } => {
                    let kind = match passing {
                        CIndirectPassing::ByValue { alignment } => {
                            format!("byval/{}", alignment.get())
                        }
                        CIndirectPassing::CallerCopy => "copy-ptr".to_owned(),
                    };
                    format!("{kind}<{}>", value.storage_type().dump())
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let result = match &self.result {
            DirectCReturn::Void => "void".to_owned(),
            DirectCReturn::Value(value) => scalar_name(value),
            DirectCReturn::DirectParts(parts) => parts_name(parts),
            DirectCReturn::Indirect(value) => format!("sret<{}>", value.storage_type().dump()),
        };
        format!("({params})->{result}")
    }
}

fn scalar_name(value: &DirectCValue) -> String {
    let extension = match value.extension {
        CIntegerExtension::None => "",
        CIntegerExtension::Sign => " signext",
        CIntegerExtension::Zero => " zeroext",
    };
    format!("{}{extension}", value.storage.storage_type().dump())
}

fn parts_name(parts: &AbiDirectParts) -> String {
    crate::dump::abi_direct_name(&AbiDirectValue::DirectParts(parts.clone()))
}
