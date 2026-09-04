//! Declaration-time validation of the closed operator and infix contracts.

use std::num::NonZeroU32;

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{FnParam, FnParamCalling, Lowerer, TypeId};

impl Lowerer {
    pub(crate) fn validate_callable_modifiers(
        &mut self,
        decl: &ast::FunctionDecl,
        receiver: Option<TypeId>,
        is_member: bool,
        is_ordinary: bool,
        params: &[FnParam],
        return_ty: TypeId,
    ) -> hir::CallableModifiers {
        let diagnostics_before = self.diagnostics.len();
        let mut property_delegate_operator = None;
        let operator = decl.operator.and_then(|modifier| {
            if receiver.is_none() {
                self.error(
                    modifier.span,
                    "`operator` requires a member or extension receiver".to_string(),
                );
                return None;
            }
            match self.operator_kind(&decl.name, modifier.span)? {
                CallableOperatorKind::Ordinary(kind) => {
                    self.validate_operator_shape(
                        decl, kind, receiver, is_member, params, return_ty,
                    );
                    Some(kind)
                }
                CallableOperatorKind::PropertyDelegate(kind) => {
                    self.validate_property_delegate_operator_shape(
                        decl,
                        kind,
                        is_ordinary,
                        params,
                        return_ty,
                    );
                    property_delegate_operator = Some(kind);
                    None
                }
            }
        });
        if let Some(infix) = decl.infix {
            if receiver.is_none() {
                self.error(
                    infix.span,
                    "`infix` requires a member or extension receiver".to_string(),
                );
            }
            if params.len() != 1 {
                self.error(
                    decl.name.span,
                    format!(
                        "infix function `{}` must have exactly one parameter, found {}",
                        decl.name.text,
                        params.len()
                    ),
                );
            } else if !matches!(params[0].calling, FnParamCalling::Required) {
                self.error(
                    decl.params[0].span,
                    format!(
                        "infix function `{}` requires one required, non-vararg parameter",
                        decl.name.text
                    ),
                );
            }
        }
        if self.diagnostics.len() != diagnostics_before {
            hir::CallableModifiers::default()
        } else {
            hir::CallableModifiers {
                operator,
                property_delegate_operator,
                is_infix: decl.infix.is_some(),
            }
        }
    }

    fn operator_kind(
        &mut self,
        name: &ast::Ident,
        modifier_span: ast::Span,
    ) -> Option<CallableOperatorKind> {
        let kind = match name.text.as_str() {
            "provideDelegate" => CallableOperatorKind::PropertyDelegate(
                hir::PropertyDelegateOperatorKind::ProvideDelegate,
            ),
            "getValue" => {
                CallableOperatorKind::PropertyDelegate(hir::PropertyDelegateOperatorKind::GetValue)
            }
            "setValue" => {
                CallableOperatorKind::PropertyDelegate(hir::PropertyDelegateOperatorKind::SetValue)
            }
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
                let suffix = &text["component".len()..];
                let Ok(index) = suffix.parse::<u32>() else {
                    self.error(
                        name.span,
                        format!("operator `{text}` must use a positive decimal component index"),
                    );
                    return None;
                };
                let Some(index) = NonZeroU32::new(index) else {
                    self.error(name.span, "operator `component0` is not valid".to_string());
                    return None;
                };
                CallableOperatorKind::Ordinary(hir::OperatorKind::Component { index })
            }
            text => {
                self.error(modifier_span, format!("unknown operator role `{text}`"));
                return None;
            }
        };
        Some(kind)
    }

    fn validate_property_delegate_operator_shape(
        &mut self,
        decl: &ast::FunctionDecl,
        kind: hir::PropertyDelegateOperatorKind,
        is_ordinary: bool,
        params: &[FnParam],
        return_ty: TypeId,
    ) {
        let name = decl.name.text.as_str();
        let expected = match kind {
            hir::PropertyDelegateOperatorKind::ProvideDelegate => 0,
            hir::PropertyDelegateOperatorKind::GetValue => 1,
            hir::PropertyDelegateOperatorKind::SetValue => 2,
        };
        if !is_ordinary {
            self.error(
                decl.name.span,
                format!("property delegate operator `{name}` must be an ordinary function"),
            );
        }
        if params.len() != expected {
            self.error(
                decl.name.span,
                format!(
                    "property delegate operator `{name}` must have exactly {expected} parameters, found {}",
                    params.len()
                ),
            );
        }
        for (index, parameter) in params.iter().enumerate() {
            if !matches!(parameter.calling, FnParamCalling::Required) {
                self.error(
                    decl.params[index].span,
                    format!(
                        "property delegate operator `{name}` requires required, non-vararg parameters"
                    ),
                );
            }
        }
        if decl.is_suspend {
            self.error(
                decl.name.span,
                format!("property delegate operator `{name}` must not be suspend"),
            );
        }
        if !decl.type_params.is_empty() {
            self.error(
                decl.name.span,
                format!("property delegate operator `{name}` must not declare type parameters"),
            );
        }
        if kind == hir::PropertyDelegateOperatorKind::SetValue
            && !self.types_equal(return_ty, self.unit)
        {
            self.error(
                decl.name.span,
                format!(
                    "property delegate operator `setValue` must return Unit, found {}",
                    self.type_name(return_ty)
                ),
            );
        }
    }

    fn validate_operator_shape(
        &mut self,
        decl: &ast::FunctionDecl,
        kind: hir::OperatorKind,
        receiver: Option<TypeId>,
        is_member: bool,
        params: &[FnParam],
        return_ty: TypeId,
    ) {
        use hir::OperatorKind as K;

        let exact_arity = match kind {
            K::UnaryPlus
            | K::UnaryMinus
            | K::Not
            | K::Inc
            | K::Dec
            | K::Component { .. }
            | K::Iterator => Some(0),
            K::Plus
            | K::Minus
            | K::Times
            | K::Div
            | K::Rem
            | K::RangeTo
            | K::RangeUntil
            | K::Contains
            | K::PlusAssign
            | K::MinusAssign
            | K::TimesAssign
            | K::DivAssign
            | K::RemAssign
            | K::CompareTo
            | K::Equals => Some(1),
            K::Get | K::Set | K::Invoke => None,
        };
        if let Some(expected) = exact_arity
            && params.len() != expected
        {
            let expected = if expected == 1 {
                "exactly one parameter".to_string()
            } else {
                format!("exactly {expected} parameters")
            };
            self.error(
                decl.name.span,
                format!(
                    "operator `{}` must have {expected}, found {}",
                    decl.name.text,
                    params.len()
                ),
            );
        }
        if !matches!(kind, K::Get | K::Set | K::Invoke)
            && let Some((index, _)) = params
                .iter()
                .enumerate()
                .find(|(_, parameter)| matches!(parameter.calling, FnParamCalling::Vararg { .. }))
        {
            self.error(
                decl.params[index].span,
                format!(
                    "operator `{}` does not accept a `vararg` parameter",
                    decl.name.text
                ),
            );
        }
        match kind {
            K::Get if params.is_empty() => self.error(
                decl.name.span,
                "operator `get` must have at least one parameter".to_string(),
            ),
            K::Set if params.len() < 2 => self.error(
                decl.name.span,
                format!(
                    "operator `set` must have at least two parameters, found {}",
                    params.len()
                ),
            ),
            K::Set => {
                if matches!(
                    params.last().map(|parameter| &parameter.calling),
                    Some(FnParamCalling::Vararg { .. })
                ) {
                    self.error(
                        decl.params.last().expect("set has parameters").span,
                        "operator `set` value parameter must not be `vararg`".to_string(),
                    );
                }
                self.require_operator_return(decl, return_ty, self.unit, "Unit");
            }
            K::Contains => self.require_operator_return(decl, return_ty, self.boolean, "Boolean"),
            K::PlusAssign | K::MinusAssign | K::TimesAssign | K::DivAssign | K::RemAssign => {
                self.require_operator_return(decl, return_ty, self.unit, "Unit")
            }
            K::CompareTo => self.require_operator_return(decl, return_ty, self.int, "Int"),
            K::Equals => {
                self.require_operator_return(decl, return_ty, self.boolean, "Boolean");
                if !is_member {
                    self.error(
                        decl.name.span,
                        "operator `equals` must be a member function".to_string(),
                    );
                }
                if decl.is_suspend {
                    self.error(
                        decl.name.span,
                        "operator `equals` must not be suspend".to_string(),
                    );
                }
                if !decl.type_params.is_empty() {
                    self.error(
                        decl.name.span,
                        "operator `equals` must not declare type parameters".to_string(),
                    );
                }
            }
            K::Inc | K::Dec => {
                if let Some(receiver) = receiver
                    && !self.is_subtype(return_ty, receiver)
                {
                    self.error(
                        decl.name.span,
                        format!(
                            "operator `{}` must return a subtype of receiver type {}, found {}",
                            decl.name.text,
                            self.type_name(receiver),
                            self.type_name(return_ty)
                        ),
                    );
                }
            }
            _ => {}
        }
    }

    fn require_operator_return(
        &mut self,
        decl: &ast::FunctionDecl,
        actual: TypeId,
        expected: TypeId,
        expected_name: &str,
    ) {
        if !self.types_equal(actual, expected) {
            self.error(
                decl.name.span,
                format!(
                    "operator `{}` must return {expected_name}, found {}",
                    decl.name.text,
                    self.type_name(actual)
                ),
            );
        }
    }
}

#[derive(Clone, Copy)]
enum CallableOperatorKind {
    Ordinary(hir::OperatorKind),
    PropertyDelegate(hir::PropertyDelegateOperatorKind),
}
