use super::QueryJson;
use scoop_hir::{
    ContextParameterLabel, GcEffect, OperatorKind, PropertyDelegateOperatorKind, Safety,
};
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, Safety> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            Safety::Safe => "safe",
            Safety::Unsafe => "unsafe",
        })
    }
}

impl Serialize for QueryJson<'_, GcEffect> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            GcEffect::Managed => "managed",
            GcEffect::NoGc => "noGc",
        })
    }
}

impl Serialize for QueryJson<'_, PropertyDelegateOperatorKind> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self.0 {
            PropertyDelegateOperatorKind::ProvideDelegate => "provideDelegate",
            PropertyDelegateOperatorKind::GetValue => "getValue",
            PropertyDelegateOperatorKind::SetValue => "setValue",
        })
    }
}

impl Serialize for QueryJson<'_, ContextParameterLabel> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            ContextParameterLabel::Named(name) => {
                object!(serializer, {"kind": "named", "name": name})
            }
            ContextParameterLabel::Unnamed => object!(serializer, {"kind": "unnamed"}),
        }
    }
}

impl Serialize for QueryJson<'_, OperatorKind> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use OperatorKind::*;
        let name = match self.0 {
            UnaryPlus => "unaryPlus",
            UnaryMinus => "unaryMinus",
            Not => "not",
            Inc => "inc",
            Dec => "dec",
            Plus => "plus",
            Minus => "minus",
            Times => "times",
            Div => "div",
            Rem => "rem",
            RangeTo => "rangeTo",
            RangeUntil => "rangeUntil",
            Contains => "contains",
            Get => "get",
            Set => "set",
            Invoke => "invoke",
            PlusAssign => "plusAssign",
            MinusAssign => "minusAssign",
            TimesAssign => "timesAssign",
            DivAssign => "divAssign",
            RemAssign => "remAssign",
            CompareTo => "compareTo",
            Equals => "equals",
            Iterator => "iterator",
            Component { index } => {
                return object!(serializer, {"kind": "component", "index": index.get()});
            }
        };
        object!(serializer, {"kind": name})
    }
}
