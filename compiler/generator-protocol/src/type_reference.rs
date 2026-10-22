//! Type messages decode to the existing unresolved signature representation.

use crate::Reference;
use scoop_identity::{
    CallingConvention, DecodedSignatureTypeKey, Effect, NonEmptyVec, PersistentGenericTypeId,
    PersistentTypeId,
};
use serde::{Deserialize, Deserializer, de};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeReference(pub DecodedSignatureTypeKey);

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Shape {
    Nominal {
        declaration: Reference<PersistentTypeId>,
    },
    NominalApplication {
        declaration: Reference<PersistentGenericTypeId>,
        arguments: Vec<TypeReference>,
    },
    Tuple {
        elements: Vec<TypeReference>,
    },
    Function {
        effect: FunctionEffect,
        receiver: Option<Box<TypeReference>>,
        parameters: Vec<TypeReference>,
        result: Box<TypeReference>,
    },
    RawPointer {
        pointee: Box<TypeReference>,
    },
    NativeFunctionPointer {
        calling_convention: NativeConvention,
        parameters: Vec<TypeReference>,
        result: Box<TypeReference>,
    },
    Binder {
        depth: u32,
        index: u32,
    },
    Companion {
        host: Box<TypeReference>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum FunctionEffect {
    Ordinary,
    Suspend,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum NativeConvention {
    C,
}

impl<'de> Deserialize<'de> for TypeReference {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use DecodedSignatureTypeKey as Type;
        let ty = match Shape::deserialize(deserializer)? {
            Shape::Nominal { declaration } => Type::Nominal(declaration.into()),
            Shape::NominalApplication {
                declaration,
                arguments,
            } => Type::NominalApplication {
                origin: declaration.into(),
                arguments: NonEmptyVec::new(types(arguments)).map_err(de::Error::custom)?,
            },
            Shape::Tuple { elements } => {
                Type::Tuple(NonEmptyVec::new(types(elements)).map_err(de::Error::custom)?)
            }
            Shape::Function {
                effect,
                receiver,
                parameters,
                result,
            } => Type::Function {
                effect: match effect {
                    FunctionEffect::Ordinary => Effect::Ordinary,
                    FunctionEffect::Suspend => Effect::Suspend,
                },
                receiver: receiver.map(|ty| Box::new(ty.0)),
                parameters: types(parameters),
                result: Box::new(result.0),
            },
            Shape::RawPointer { pointee } => Type::RawPointer(Box::new(pointee.0)),
            Shape::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => Type::NativeFunctionPointer {
                calling_convention: match calling_convention {
                    NativeConvention::C => CallingConvention::C,
                },
                parameters: types(parameters),
                result: Box::new(result.0),
            },
            Shape::Binder { depth, index } => Type::Binder { depth, index },
            Shape::Companion { host } => Type::Companion(Box::new(host.0)),
        };
        Ok(Self(ty))
    }
}

fn types(values: Vec<TypeReference>) -> Vec<DecodedSignatureTypeKey> {
    values.into_iter().map(|ty| ty.0).collect()
}
