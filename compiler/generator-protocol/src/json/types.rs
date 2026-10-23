use super::QueryJson;
use crate::Reference;
use scoop_identity::{CallingConvention, Effect, SignatureTypeKey};
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, SignatureTypeKey> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            SignatureTypeKey::Nominal(id) => object!(serializer, {
                "kind": "nominal", "declaration": Reference::from(*id),
            }),
            SignatureTypeKey::NominalApplication { origin, arguments } => object!(serializer, {
                "kind": "nominalApplication", "declaration": Reference::from(*origin),
                "arguments": QueryJson(arguments.as_slice()),
            }),
            SignatureTypeKey::Tuple(elements) => object!(serializer, {
                "kind": "tuple", "elements": QueryJson(elements.as_slice()),
            }),
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
                receiver,
            } => object!(serializer, {
                "kind": "function", "effect": QueryJson(effect),
                "receiver": receiver.as_ref().map(|ty| QueryJson(ty.as_ref())),
                "parameters": QueryJson(parameters.as_slice()), "result": QueryJson(result.as_ref()),
            }),
            SignatureTypeKey::RawPointer(pointee) => object!(serializer, {
                "kind": "rawPointer", "pointee": QueryJson(pointee.as_ref()),
            }),
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => object!(serializer, {
                "kind": "nativeFunctionPointer", "callingConvention": QueryJson(calling_convention),
                "parameters": QueryJson(parameters.as_slice()), "result": QueryJson(result.as_ref()),
            }),
            SignatureTypeKey::Binder { depth, index } => object!(serializer, {
                "kind": "binder", "depth": depth, "index": index,
            }),
            SignatureTypeKey::Companion(host) => object!(serializer, {
                "kind": "companion", "host": QueryJson(host.as_ref()),
            }),
        }
    }
}

impl Serialize for QueryJson<'_, Effect> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            Effect::Ordinary => "ordinary",
            Effect::Suspend => "suspend",
        })
    }
}

impl Serialize for QueryJson<'_, CallingConvention> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            CallingConvention::C => "c",
        })
    }
}
