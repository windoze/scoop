use crate::{
    CallableInfixV1, CallableModifiers, CallableOperatorRoleV1, CallableOperatorV1,
    CallableSafetyV1, CallableSourceEffectsV1, CallingConvention, FunctionAttributes, GcEffect,
    OperatorKind, PropertyDelegateOperatorKind, PropertyDelegateOperatorV1, Safety,
};

impl CallableSourceEffectsV1 {
    pub fn function_attributes(self) -> FunctionAttributes {
        FunctionAttributes {
            safety: match self.safety() {
                CallableSafetyV1::Safe => Safety::Safe,
                CallableSafetyV1::Unsafe => Safety::Unsafe,
            },
            gc_effect: match self.gc_effect() {
                scoop_identity::GcEffect::Managed => GcEffect::Managed,
                scoop_identity::GcEffect::NoGc => GcEffect::NoGc,
            },
            calling_convention: CallingConvention::Cdecl,
        }
    }

    pub fn callable_modifiers(self) -> CallableModifiers {
        let (operator, property_delegate_operator) = match self.operator_role() {
            CallableOperatorRoleV1::None => (None, None),
            CallableOperatorRoleV1::Language(operator) => (Some(import_operator(operator)), None),
            CallableOperatorRoleV1::PropertyDelegate(operator) => (
                None,
                Some(match operator {
                    PropertyDelegateOperatorV1::ProvideDelegate => {
                        PropertyDelegateOperatorKind::ProvideDelegate
                    }
                    PropertyDelegateOperatorV1::GetValue => PropertyDelegateOperatorKind::GetValue,
                    PropertyDelegateOperatorV1::SetValue => PropertyDelegateOperatorKind::SetValue,
                }),
            ),
        };
        CallableModifiers {
            operator,
            property_delegate_operator,
            is_infix: self.infix() == CallableInfixV1::Infix,
        }
    }
}

fn import_operator(operator: CallableOperatorV1) -> OperatorKind {
    match operator {
        CallableOperatorV1::UnaryPlus => OperatorKind::UnaryPlus,
        CallableOperatorV1::UnaryMinus => OperatorKind::UnaryMinus,
        CallableOperatorV1::Not => OperatorKind::Not,
        CallableOperatorV1::Inc => OperatorKind::Inc,
        CallableOperatorV1::Dec => OperatorKind::Dec,
        CallableOperatorV1::Plus => OperatorKind::Plus,
        CallableOperatorV1::Minus => OperatorKind::Minus,
        CallableOperatorV1::Times => OperatorKind::Times,
        CallableOperatorV1::Div => OperatorKind::Div,
        CallableOperatorV1::Rem => OperatorKind::Rem,
        CallableOperatorV1::RangeTo => OperatorKind::RangeTo,
        CallableOperatorV1::RangeUntil => OperatorKind::RangeUntil,
        CallableOperatorV1::Contains => OperatorKind::Contains,
        CallableOperatorV1::Get => OperatorKind::Get,
        CallableOperatorV1::Set => OperatorKind::Set,
        CallableOperatorV1::Invoke => OperatorKind::Invoke,
        CallableOperatorV1::PlusAssign => OperatorKind::PlusAssign,
        CallableOperatorV1::MinusAssign => OperatorKind::MinusAssign,
        CallableOperatorV1::TimesAssign => OperatorKind::TimesAssign,
        CallableOperatorV1::DivAssign => OperatorKind::DivAssign,
        CallableOperatorV1::RemAssign => OperatorKind::RemAssign,
        CallableOperatorV1::CompareTo => OperatorKind::CompareTo,
        CallableOperatorV1::Equals => OperatorKind::Equals,
        CallableOperatorV1::Component { index } => OperatorKind::Component { index },
        CallableOperatorV1::Iterator => OperatorKind::Iterator,
    }
}
