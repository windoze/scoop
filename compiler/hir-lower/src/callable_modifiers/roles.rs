//! The operator roles shared by declaration validation and candidate lookup.

use scoop_hir as hir;
use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableOperatorKind {
    Ordinary(hir::OperatorKind),
    PropertyDelegate(hir::PropertyDelegateOperatorKind),
}

pub(super) fn property_delegate_operator_kind(
    name: &str,
) -> Option<hir::PropertyDelegateOperatorKind> {
    match name {
        "provideDelegate" => Some(hir::PropertyDelegateOperatorKind::ProvideDelegate),
        "getValue" => Some(hir::PropertyDelegateOperatorKind::GetValue),
        "setValue" => Some(hir::PropertyDelegateOperatorKind::SetValue),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum OperatorNameError {
    InvalidComponent,
    ZeroComponent,
    Unknown,
}

pub(crate) fn callable_operator_kind(
    name: &str,
) -> Result<CallableOperatorKind, OperatorNameError> {
    if let Some(kind) = property_delegate_operator_kind(name) {
        return Ok(CallableOperatorKind::PropertyDelegate(kind));
    }
    let kind = match name {
        "unaryPlus" => CallableOperatorKind::Ordinary(hir::OperatorKind::UnaryPlus),
        "unaryMinus" => CallableOperatorKind::Ordinary(hir::OperatorKind::UnaryMinus),
        "not" => CallableOperatorKind::Ordinary(hir::OperatorKind::Not),
        "inc" => CallableOperatorKind::Ordinary(hir::OperatorKind::Inc),
        "dec" => CallableOperatorKind::Ordinary(hir::OperatorKind::Dec),
        "plus" => CallableOperatorKind::Ordinary(hir::OperatorKind::Plus),
        "minus" => CallableOperatorKind::Ordinary(hir::OperatorKind::Minus),
        "times" => CallableOperatorKind::Ordinary(hir::OperatorKind::Times),
        "div" => CallableOperatorKind::Ordinary(hir::OperatorKind::Div),
        "rem" => CallableOperatorKind::Ordinary(hir::OperatorKind::Rem),
        "rangeTo" => CallableOperatorKind::Ordinary(hir::OperatorKind::RangeTo),
        "rangeUntil" => CallableOperatorKind::Ordinary(hir::OperatorKind::RangeUntil),
        "contains" => CallableOperatorKind::Ordinary(hir::OperatorKind::Contains),
        "get" => CallableOperatorKind::Ordinary(hir::OperatorKind::Get),
        "set" => CallableOperatorKind::Ordinary(hir::OperatorKind::Set),
        "invoke" => CallableOperatorKind::Ordinary(hir::OperatorKind::Invoke),
        "plusAssign" => CallableOperatorKind::Ordinary(hir::OperatorKind::PlusAssign),
        "minusAssign" => CallableOperatorKind::Ordinary(hir::OperatorKind::MinusAssign),
        "timesAssign" => CallableOperatorKind::Ordinary(hir::OperatorKind::TimesAssign),
        "divAssign" => CallableOperatorKind::Ordinary(hir::OperatorKind::DivAssign),
        "remAssign" => CallableOperatorKind::Ordinary(hir::OperatorKind::RemAssign),
        "compareTo" => CallableOperatorKind::Ordinary(hir::OperatorKind::CompareTo),
        "equals" => CallableOperatorKind::Ordinary(hir::OperatorKind::Equals),
        "iterator" => CallableOperatorKind::Ordinary(hir::OperatorKind::Iterator),
        text if text.starts_with("component") => {
            let index = text["component".len()..]
                .parse::<u32>()
                .map_err(|_| OperatorNameError::InvalidComponent)?;
            let index = NonZeroU32::new(index).ok_or(OperatorNameError::ZeroComponent)?;
            CallableOperatorKind::Ordinary(hir::OperatorKind::Component { index })
        }
        _ => return Err(OperatorNameError::Unknown),
    };
    Ok(kind)
}
