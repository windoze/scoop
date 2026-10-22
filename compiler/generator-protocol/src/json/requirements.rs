use super::QueryJson;
use scoop_identity::{
    MemberRequirementKey, RequiredArgumentKey, RequiredEffectsKey, RequiredMemberKey,
    SignatureTypeKey,
};
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, MemberRequirementKey> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "subject": QueryJson(&self.0.subject), "call": QueryJson(&self.0.call),
            "arguments": QueryJson(self.0.arguments.as_slice()), "result": QueryJson(&self.0.result),
            "effects": QueryJson(&self.0.effects),
        })
    }
}

impl Serialize for QueryJson<'_, RequiredMemberKey<SignatureTypeKey>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            RequiredMemberKey::Constructor => object!(serializer, {"kind": "constructor"}),
            RequiredMemberKey::Method {
                name,
                type_arguments,
            } => object!(serializer, {
                "kind": "method", "name": name, "typeArguments": QueryJson(type_arguments.as_slice()),
            }),
        }
    }
}

impl Serialize for QueryJson<'_, RequiredArgumentKey<SignatureTypeKey>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"name": self.0.name, "spread": self.0.spread, "valueType": QueryJson(&self.0.ty)})
    }
}

impl Serialize for QueryJson<'_, RequiredEffectsKey> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"suspend": self.0.is_suspend, "unsafe": self.0.is_unsafe, "noGc": self.0.is_no_gc})
    }
}
