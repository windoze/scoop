use super::*;

impl Serialize for QueryJson<'_, hir::CallableSourceEffectsV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let attributes = self.0.function_attributes();
        let modifiers = self.0.callable_modifiers();
        object!(serializer, {
            "execution": match self.0.execution() { identity::Effect::Ordinary => "ordinary", identity::Effect::Suspend => "suspend" },
            "safety": QueryJson(&attributes.safety), "gc": QueryJson(&attributes.gc_effect),
            "callingConvention": match attributes.calling_convention { hir::CallingConvention::Cdecl => "c" },
            "operator": QueryJson(&modifiers.operator), "propertyDelegateOperator": QueryJson(&modifiers.property_delegate_operator),
            "infix": modifiers.is_infix, "release": QueryJson(self.0.release_callability()),
        })
    }
}

impl Serialize for QueryJson<'_, hir::CallableReleaseCallabilityV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::ConditionalReleaseCallability::Unavailable => {
                object!(serializer, {"kind": "unavailable"})
            }
            hir::ConditionalReleaseCallability::NoTransition { requirements } => {
                object!(serializer, {"kind": "noTransition", "requirements": QueryJson(requirements.as_slice())})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::ReleaseValueBinderV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        QueryJson(&self.0.signature()).serialize(serializer)
    }
}
