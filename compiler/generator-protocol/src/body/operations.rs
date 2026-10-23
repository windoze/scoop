use super::*;

macro_rules! leaf {
    ($ty:ty, {$($variant:ident: $name:literal),+ $(,)?}) => {
        impl Serialize for QueryJson<'_, $ty> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(match self.0 { $(<$ty>::$variant => $name),+ })
            }
        }
    };
}

mod atomic;
mod numeric;

leaf!(hir::DefaultPrimitiveBinaryKindV1, {StringConcat: "stringConcat", StringCompareTo: "stringCompareTo"});
leaf!(hir::DefaultPrimitiveUnaryKindV1, {BooleanNot: "booleanNot"});
leaf!(hir::DefaultArrayAccessKindV1, {ImmutableGet: "immutableGet", MutableGet: "mutableGet", MutableSet: "mutableSet"});
leaf!(hir::DefaultForeignCallbackOperationV1, {Retain: "retain", Release: "release", State: "state", Failure: "failure"});
leaf!(hir::DefaultBinaryOperatorV1, {Lt: "lt", Le: "le", Gt: "gt", Ge: "ge", RefEq: "refEq", RefNe: "refNe", And: "and", Or: "or"});
leaf!(hir::DefaultUnaryOperatorV1, {Not: "not"});

impl Serialize for BodyJson<'_, hir::MaybeUninitOperation<hir::DefaultExpressionV1>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::MaybeUninitOperation::Uninit => object!(serializer, {"kind": "uninit"}),
            hir::MaybeUninitOperation::Initialized(value) => {
                object!(serializer, {"kind": "initialized", "value": self.child(value)})
            }
            hir::MaybeUninitOperation::AssumeInit(value) => {
                object!(serializer, {"kind": "assumeInit", "value": self.child(value)})
            }
        }
    }
}
