//! Physical outbound C calls, independent of the caller's GC protocol.

use crate::{
    CCodePointerStorage, CDataPointerStorage, FloatKind, GeneratedBridgeEntryIdentity, IntegerKind,
    LirType, PointerKind,
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
    pub params: Vec<DirectCValue>,
    pub result: DirectCReturn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectCReturn {
    Void,
    Value(DirectCValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectCValue {
    pub ty: DirectCType,
    pub extension: CIntegerExtension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CIntegerExtension {
    None,
    Sign,
    Zero,
}

/// Only canonical C scalar representations can appear in DirectC. Nullable
/// pointer storage retains its exact enum identity; Boolean is an LLVM i1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectCType {
    Integer(IntegerKind),
    Boolean,
    Float(FloatKind),
    DataPointer(CDataPointerStorage),
    CodePointer(CCodePointerStorage),
}

impl DirectCType {
    pub fn storage_type(&self) -> LirType {
        match self {
            Self::Integer(kind) => kind.scalar_type(),
            Self::Boolean => LirType::I1,
            Self::Float(kind) => LirType::floating(*kind),
            Self::DataPointer(CDataPointerStorage::Direct) => LirType::Ptr(PointerKind::Raw),
            Self::DataPointer(CDataPointerStorage::Nullable(reference)) => {
                LirType::Enum(reference.definition())
            }
            Self::CodePointer(CCodePointerStorage::Direct) => LirType::Ptr(PointerKind::Code),
            Self::CodePointer(CCodePointerStorage::Nullable(reference)) => {
                LirType::Enum(reference.definition())
            }
        }
    }
}

impl DirectCSignature {
    pub fn dump(&self) -> String {
        fn value(value: &DirectCValue) -> String {
            let extension = match value.extension {
                CIntegerExtension::None => "",
                CIntegerExtension::Sign => " signext",
                CIntegerExtension::Zero => " zeroext",
            };
            format!("{}{extension}", value.ty.storage_type().dump())
        }
        let params = self.params.iter().map(value).collect::<Vec<_>>().join(",");
        let result = match &self.result {
            DirectCReturn::Void => "void".to_owned(),
            DirectCReturn::Value(result) => value(result),
        };
        format!("({params})->{result}")
    }
}
