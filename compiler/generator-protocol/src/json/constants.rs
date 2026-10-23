use super::QueryJson;
use scoop_hir::{
    CanonicalConstValueKindV1, CanonicalConstValueV1, CanonicalIntegerConstantV1, IntegerKind,
    IntegerSignedness,
};
use scoop_identity::FloatConstant;
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, CanonicalIntegerConstantV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use CanonicalIntegerConstantV1::*;
        let value = match *self.0 {
            Signed8(bits) => (bits as i8).to_string(),
            Signed16(bits) => (bits as i16).to_string(),
            Signed32(bits) => (bits as i32).to_string(),
            Signed64(bits) => (bits as i64).to_string(),
            Unsigned8(value) => value.to_string(),
            Unsigned16(value) => value.to_string(),
            Unsigned32(value) => value.to_string(),
            Unsigned64(value) => value.to_string(),
        };
        object!(serializer, {
            "kind": "integer", "signed": self.0.kind().signedness() == IntegerSignedness::Signed,
            "width": self.0.kind().width().bits(), "value": value,
        })
    }
}

impl Serialize for QueryJson<'_, FloatConstant> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let bits = match self.0 {
            FloatConstant::F32(bits) => format!("{bits:08x}"),
            FloatConstant::F64(bits) => format!("{bits:016x}"),
        };
        object!(serializer, { "kind": "float", "width": self.0.kind().bits(), "bits": bits })
    }
}

impl Serialize for QueryJson<'_, CanonicalConstValueV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            CanonicalConstValueV1::Integer(value) => QueryJson(value).serialize(serializer),
            CanonicalConstValueV1::Float(value) => QueryJson(value).serialize(serializer),
            CanonicalConstValueV1::Boolean(value) => {
                object!(serializer, { "kind": "boolean", "value": value.value() })
            }
            CanonicalConstValueV1::Char(value) => {
                object!(serializer, { "kind": "char", "value": value.value() })
            }
            CanonicalConstValueV1::String(value) => {
                object!(serializer, { "kind": "string", "value": value })
            }
        }
    }
}

impl Serialize for QueryJson<'_, IntegerKind> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "kind": "integer", "signed": self.0.signedness() == IntegerSignedness::Signed,
            "width": self.0.width().bits(),
        })
    }
}

impl Serialize for QueryJson<'_, CanonicalConstValueKindV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            CanonicalConstValueKindV1::Integer(kind) => QueryJson(kind).serialize(serializer),
            CanonicalConstValueKindV1::Float(kind) => {
                object!(serializer, { "kind": "float", "width": kind.bits() })
            }
            CanonicalConstValueKindV1::Boolean => object!(serializer, { "kind": "boolean" }),
            CanonicalConstValueKindV1::Char => object!(serializer, { "kind": "char" }),
            CanonicalConstValueKindV1::String => object!(serializer, { "kind": "string" }),
        }
    }
}
