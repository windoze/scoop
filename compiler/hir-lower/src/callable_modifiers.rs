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
        params: &[FnParam],
        return_ty: TypeId,
    ) -> hir::CallableModifiers {
        let diagnostics_before = self.diagnostics.len();
        let operator = decl.operator.and_then(|modifier| {
            if receiver.is_none() {
                self.error(
                    modifier.span,
                    "`operator` requires a member or extension receiver".to_string(),
                );
                return None;
            }
            let kind = self.operator_kind(&decl.name, modifier.span)?;
            self.validate_operator_shape(decl, kind, receiver, is_member, params, return_ty);
            Some(kind)
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
                is_infix: decl.infix.is_some(),
            }
        }
    }

    fn operator_kind(
        &mut self,
        name: &ast::Ident,
        modifier_span: ast::Span,
    ) -> Option<hir::OperatorKind> {
        let kind = match name.text.as_str() {
            "unaryPlus" => hir::OperatorKind::UnaryPlus,
            "unaryMinus" => hir::OperatorKind::UnaryMinus,
            "not" => hir::OperatorKind::Not,
            "inc" => hir::OperatorKind::Inc,
            "dec" => hir::OperatorKind::Dec,
            "plus" => hir::OperatorKind::Plus,
            "minus" => hir::OperatorKind::Minus,
            "times" => hir::OperatorKind::Times,
            "div" => hir::OperatorKind::Div,
            "rem" => hir::OperatorKind::Rem,
            "rangeTo" => hir::OperatorKind::RangeTo,
            "rangeUntil" => hir::OperatorKind::RangeUntil,
            "contains" => hir::OperatorKind::Contains,
            "get" => hir::OperatorKind::Get,
            "set" => hir::OperatorKind::Set,
            "invoke" => hir::OperatorKind::Invoke,
            "plusAssign" => hir::OperatorKind::PlusAssign,
            "minusAssign" => hir::OperatorKind::MinusAssign,
            "timesAssign" => hir::OperatorKind::TimesAssign,
            "divAssign" => hir::OperatorKind::DivAssign,
            "remAssign" => hir::OperatorKind::RemAssign,
            "compareTo" => hir::OperatorKind::CompareTo,
            "equals" => hir::OperatorKind::Equals,
            "iterator" => hir::OperatorKind::Iterator,
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
                hir::OperatorKind::Component { index }
            }
            text => {
                self.error(modifier_span, format!("unknown operator role `{text}`"));
                return None;
            }
        };
        Some(kind)
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
