use scoop_identity::{Effect, GcEffect};

use super::CallableEffectProjectionError;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableOperatorRoleV1, CallableOperatorV1,
    CallableSafetyV1, CallableSourceEffectsV1, ExternAbi, Function, FunctionAttributes,
    FunctionKind, OperatorKind, PropertyDelegateOperatorKind, PropertyDelegateOperatorV1,
};

pub(in crate::production) fn function(
    export: &crate::ExportHir,
    declaration: &Function,
) -> Result<CallableSourceEffectsV1, CallableEffectProjectionError> {
    let implementation = match declaration.kind {
        FunctionKind::User(_) => CallableImplementationV1::Scoop,
        FunctionKind::Intrinsic(_) => CallableImplementationV1::Intrinsic,
        FunctionKind::Extern(id) => {
            let Some(external) = super::arena_get(&export.extern_functions, id) else {
                return Err(CallableEffectProjectionError::UnknownExternFunction(
                    super::raw_index(id),
                ));
            };
            match external.abi {
                ExternAbi::Scoop => CallableImplementationV1::SourceExternScoop,
                ExternAbi::C => CallableImplementationV1::SourceExternC,
            }
        }
        FunctionKind::DerivedEquality => {
            return Err(CallableEffectProjectionError::ConflictingOperatorRoles);
        }
    };
    let operator_role = match (
        declaration.modifiers.operator,
        declaration.modifiers.property_delegate_operator,
    ) {
        (Some(_), Some(_)) => {
            return Err(CallableEffectProjectionError::ConflictingOperatorRoles);
        }
        (Some(operator), None) => CallableOperatorRoleV1::Language(map_operator(operator)),
        (None, Some(operator)) => {
            CallableOperatorRoleV1::PropertyDelegate(map_delegate_operator(operator))
        }
        (None, None) => CallableOperatorRoleV1::None,
    };
    build(
        declaration.is_suspend,
        declaration.attributes,
        implementation,
        operator_role,
        declaration.modifiers.is_infix,
    )
}

pub(in crate::production) fn accessor(
    attributes: FunctionAttributes,
) -> Result<CallableSourceEffectsV1, CallableEffectProjectionError> {
    build(
        false,
        attributes,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        false,
    )
}

pub(in crate::production) fn source_constructor(
    safety: crate::Safety,
    gc_effect: crate::GcEffect,
) -> Result<CallableSourceEffectsV1, CallableEffectProjectionError> {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        match safety {
            crate::Safety::Safe => CallableSafetyV1::Safe,
            crate::Safety::Unsafe => CallableSafetyV1::Unsafe,
        },
        match gc_effect {
            crate::GcEffect::Managed => GcEffect::Managed,
            crate::GcEffect::NoGc => GcEffect::NoGc,
        },
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .map_err(CallableEffectProjectionError::Build)
}

fn build(
    is_suspend: bool,
    attributes: FunctionAttributes,
    implementation: CallableImplementationV1,
    operator_role: CallableOperatorRoleV1,
    is_infix: bool,
) -> Result<CallableSourceEffectsV1, CallableEffectProjectionError> {
    CallableSourceEffectsV1::try_new(
        if is_suspend {
            Effect::Suspend
        } else {
            Effect::Ordinary
        },
        match attributes.safety {
            crate::Safety::Safe => CallableSafetyV1::Safe,
            crate::Safety::Unsafe => CallableSafetyV1::Unsafe,
        },
        match attributes.gc_effect {
            crate::GcEffect::Managed => GcEffect::Managed,
            crate::GcEffect::NoGc => GcEffect::NoGc,
        },
        implementation,
        operator_role,
        if is_infix {
            CallableInfixV1::Infix
        } else {
            CallableInfixV1::Ordinary
        },
    )
    .map_err(CallableEffectProjectionError::Build)
}

fn map_operator(operator: OperatorKind) -> CallableOperatorV1 {
    match operator {
        OperatorKind::UnaryPlus => CallableOperatorV1::UnaryPlus,
        OperatorKind::UnaryMinus => CallableOperatorV1::UnaryMinus,
        OperatorKind::Not => CallableOperatorV1::Not,
        OperatorKind::Inc => CallableOperatorV1::Inc,
        OperatorKind::Dec => CallableOperatorV1::Dec,
        OperatorKind::Plus => CallableOperatorV1::Plus,
        OperatorKind::Minus => CallableOperatorV1::Minus,
        OperatorKind::Times => CallableOperatorV1::Times,
        OperatorKind::Div => CallableOperatorV1::Div,
        OperatorKind::Rem => CallableOperatorV1::Rem,
        OperatorKind::RangeTo => CallableOperatorV1::RangeTo,
        OperatorKind::RangeUntil => CallableOperatorV1::RangeUntil,
        OperatorKind::Contains => CallableOperatorV1::Contains,
        OperatorKind::Get => CallableOperatorV1::Get,
        OperatorKind::Set => CallableOperatorV1::Set,
        OperatorKind::Invoke => CallableOperatorV1::Invoke,
        OperatorKind::PlusAssign => CallableOperatorV1::PlusAssign,
        OperatorKind::MinusAssign => CallableOperatorV1::MinusAssign,
        OperatorKind::TimesAssign => CallableOperatorV1::TimesAssign,
        OperatorKind::DivAssign => CallableOperatorV1::DivAssign,
        OperatorKind::RemAssign => CallableOperatorV1::RemAssign,
        OperatorKind::CompareTo => CallableOperatorV1::CompareTo,
        OperatorKind::Equals => CallableOperatorV1::Equals,
        OperatorKind::Component { index } => CallableOperatorV1::Component { index },
        OperatorKind::Iterator => CallableOperatorV1::Iterator,
    }
}

fn map_delegate_operator(operator: PropertyDelegateOperatorKind) -> PropertyDelegateOperatorV1 {
    match operator {
        PropertyDelegateOperatorKind::ProvideDelegate => {
            PropertyDelegateOperatorV1::ProvideDelegate
        }
        PropertyDelegateOperatorKind::GetValue => PropertyDelegateOperatorV1::GetValue,
        PropertyDelegateOperatorKind::SetValue => PropertyDelegateOperatorV1::SetValue,
    }
}
