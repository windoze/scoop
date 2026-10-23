use super::*;

impl Serialize for BodyJson<'_, hir::DefaultWhenV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let arms: Vec<_> = self.node.arms().collect();
        object!(serializer, {
            "subject": self.node.subject().map(|value| self.child(value)),
            "arms": self.child(arms.as_slice()), "fallback": self.child(&self.node.fallback()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::DefaultWhenArmRefV1<'_>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"condition": self.child(&self.node.condition()), "guard": self.child(self.node.guard()), "body": self.child(self.node.body()), "origin": QueryJson(self.node.definition_origin())})
    }
}

impl Serialize
    for BodyJson<'_, hir::DefaultWhenConditionV1<&hir::DefaultPatternV1, &hir::DefaultWhenGuardV1>>
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultWhenConditionV1::Case(pattern) => {
                object!(serializer, {"kind": "case", "pattern": self.child(*pattern)})
            }
            hir::DefaultWhenConditionV1::Predicate(guard) => {
                object!(serializer, {"kind": "predicate", "guard": self.child(*guard)})
            }
            hir::DefaultWhenConditionV1::Always => object!(serializer, {"kind": "always"}),
        }
    }
}

impl Serialize for BodyJson<'_, hir::OptionalDefaultWhenGuardV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node.as_ref() {
            Some(guard) => self.child(guard).serialize(serializer),
            None => serializer.serialize_none(),
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultWhenGuardV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"setup": self.child(self.node.setup()), "condition": self.child(self.node.condition())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultWhenFallbackViewV1<'_>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultWhenFallbackViewV1::*;
        match self.node {
            Fallthrough => object!(serializer, {"kind": "fallthrough"}),
            Else(body) => object!(serializer, {"kind": "else", "body": self.child(*body)}),
            IrrefutableArm { subject_type } => {
                object!(serializer, {"kind": "irrefutableArm", "subjectType": self.child(*subject_type)})
            }
            PatternMatrix { subject_type } => {
                object!(serializer, {"kind": "patternMatrix", "subjectType": self.child(*subject_type)})
            }
            EnumPatternMatrix {
                subject_type,
                owner_type,
            } => {
                object!(serializer, {"kind": "enumPatternMatrix", "subjectType": self.child(*subject_type), "ownerType": self.child(*owner_type)})
            }
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultTryV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"body": self.child(self.node.body()), "catches": self.child(self.node.catches()), "finallyBody": self.child(self.node.finally_body())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultCatchV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"local": QueryJson(self.node.local()), "valueType": self.child(self.node.value_type()), "body": self.child(self.node.body()), "origin": QueryJson(self.node.definition_origin())})
    }
}
