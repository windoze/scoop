//! Declaration-time validation of the closed operator and infix contracts.

mod results;
mod roles;

pub(crate) use roles::{CallableOperatorKind, callable_operator_kind};
use roles::{OperatorNameError, property_delegate_operator_kind};

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{FnParam, FnParamCalling, Lowerer, TypeId};

impl Lowerer {
    pub(crate) fn resolve_callable_parameters(
        &mut self,
        decl: &ast::FunctionSignature,
    ) -> Vec<FnParam> {
        let delegate =
            decl.operator.is_some() && property_delegate_operator_kind(&decl.name.text).is_some();
        decl.params
            .iter()
            .filter_map(|parameter| {
                if delegate && !matches!(parameter.syntax, ast::ParameterSyntax::Required) {
                    // Invalid role parameters must not request an implicit Array application.
                    self.error(
                        parameter.span,
                        format!(
                            "property delegate operator `{}` requires required, non-vararg parameters",
                            decl.name.text,
                        ),
                    );
                    None
                } else {
                    self.resolve_fn_param(parameter)
                }
            })
            .collect()
    }

    pub(crate) fn validate_callable_modifiers(
        &mut self,
        decl: ast::FunctionHeaderRef<'_>,
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
        match callable_operator_kind(&name.text) {
            Ok(kind) => Some(kind),
            Err(error) => {
                let (span, message) = match error {
                    OperatorNameError::InvalidComponent => (
                        name.span,
                        format!(
                            "operator `{}` must use a positive decimal component index",
                            name.text
                        ),
                    ),
                    OperatorNameError::ZeroComponent => {
                        (name.span, "operator `component0` is not valid".to_owned())
                    }
                    OperatorNameError::Unknown => (
                        modifier_span,
                        format!("unknown operator role `{}`", name.text),
                    ),
                };
                self.error(span, message);
                None
            }
        }
    }

    fn validate_property_delegate_operator_shape(
        &mut self,
        decl: ast::FunctionHeaderRef<'_>,
        kind: hir::PropertyDelegateOperatorKind,
        is_ordinary: bool,
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
        if decl.params.len() != expected {
            self.error(
                decl.name.span,
                format!(
                    "property delegate operator `{name}` must have exactly {expected} parameters, found {}",
                    decl.params.len()
                ),
            );
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
            && !crate::declaration_inference::infers_header_result(decl)
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
        decl: ast::FunctionHeaderRef<'_>,
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
            }
            K::Equals => {
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
            _ => {}
        }
        if !crate::declaration_inference::infers_header_result(decl) {
            self.validate_operator_result(decl.signature, kind, receiver, return_ty);
        }
    }

    fn require_operator_return(
        &mut self,
        decl: &ast::FunctionSignature,
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
