use super::floating_values::{convert_float_constant, evaluate_float_binary, evaluate_float_unary};
use super::*;

pub(in crate::globals) struct ResolvedConstFloatIntrinsic {
    pub(in crate::globals) kind: hir::FloatIntrinsicKind,
    pub(in crate::globals) parameters: Vec<String>,
}

impl Lowerer {
    pub(in crate::globals) fn resolve_const_float_intrinsic(
        &self,
        source: hir::TypeId,
        source_name: &str,
    ) -> Option<ResolvedConstFloatIntrinsic> {
        let owner = if let Some(kind) = self.float_kind(source) {
            hir::IntrinsicTypeKind::Float(kind)
        } else if let hir::Type::Integer(kind) = self.types[source] {
            hir::IntrinsicTypeKind::Integer(kind)
        } else {
            return None;
        };
        for kind in hir::float_intrinsic_kinds()
            .into_iter()
            .filter(|kind| kind.owner() == owner)
        {
            let key = hir::IntrinsicFunctionKind::Float(kind);
            if let Some(&(function, _)) = self.intrinsic_functions.get(&key) {
                if self.functions[function].name.rsplit('.').next() != Some(source_name) {
                    continue;
                }
                return Some(ResolvedConstFloatIntrinsic {
                    kind,
                    parameters: self.signatures[&function]
                        .params
                        .iter()
                        .map(|param| param.name.text.clone())
                        .collect(),
                });
            }
            if let Some(callable) = self.dependencies.as_ref().and_then(|dependencies| {
                dependencies.intrinsic_callable(key, scoop_identity::GcEffect::NoGc)
            }) && callable.name().as_str() == source_name
            {
                return Some(ResolvedConstFloatIntrinsic {
                    kind,
                    parameters: callable
                        .source()
                        .parameters()
                        .parameters()
                        .iter()
                        .map(|param| param.name().as_str().to_owned())
                        .collect(),
                });
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::globals::consts) fn evaluate_const_float_call(
        &mut self,
        resolved: ResolvedConstFloatIntrinsic,
        receiver: EvaluatedConst,
        args: &[ast::CallArgument],
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        if !resolved.arguments_match(args) {
            self.error(
                span,
                "const floating intrinsic arguments do not match its declaration".into(),
            );
            return None;
        }
        let right = if let [argument] = args {
            let value = self.evaluate_const_expression(
                &argument.expression,
                Some(receiver.ty),
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            if !self.types_equal(receiver.ty, value.ty) {
                self.error(
                    argument.span,
                    format!(
                        "const floating intrinsic expected {}, found {}",
                        self.type_name(receiver.ty),
                        self.type_name(value.ty)
                    ),
                );
                return None;
            }
            Some(value.value)
        } else {
            None
        };
        let value = evaluate_float_call(resolved.kind, receiver.value, right);
        let ty = self.float_const_result_type(&value)?;
        Some(EvaluatedConst { value, ty })
    }

    pub(in crate::globals) fn float_const_result_type(
        &mut self,
        value: &hir::ConstPropertyValue,
    ) -> Option<hir::TypeId> {
        Some(match value {
            hir::ConstPropertyValue::Float(value) => self.core_float_type(value.kind()).ok()?,
            hir::ConstPropertyValue::Integer(value) => self.integer_type(value.kind()),
            hir::ConstPropertyValue::Boolean(_) => self.boolean,
            hir::ConstPropertyValue::String(_) | hir::ConstPropertyValue::Char(_) => {
                unreachable!("floating operations have numeric or Boolean results")
            }
        })
    }
}

impl ResolvedConstFloatIntrinsic {
    pub(in crate::globals) fn arguments_match(&self, arguments: &[ast::CallArgument]) -> bool {
        self.parameters.len() == arguments.len()
            && self
                .parameters
                .iter()
                .zip(arguments)
                .all(|(name, argument)| {
                    matches!(argument.spread, ast::SpreadSyntax::Plain)
                        && match &argument.name {
                            ast::CallArgumentName::Positional => true,
                            ast::CallArgumentName::TrailingLambda => false,
                            ast::CallArgumentName::Named(found) => found.text == *name,
                        }
                })
    }
}

pub(in crate::globals) fn evaluate_float_call(
    intrinsic: hir::FloatIntrinsicKind,
    receiver: hir::ConstPropertyValue,
    argument: Option<hir::ConstPropertyValue>,
) -> hir::ConstPropertyValue {
    match (intrinsic, receiver, argument) {
        (
            hir::FloatIntrinsicKind::Unary { operation, .. },
            hir::ConstPropertyValue::Float(value),
            None,
        ) => evaluate_float_unary(operation, value),
        (
            hir::FloatIntrinsicKind::Binary { operation, .. },
            hir::ConstPropertyValue::Float(lhs),
            Some(hir::ConstPropertyValue::Float(rhs)),
        ) => evaluate_float_binary(operation, lhs, rhs),
        (hir::FloatIntrinsicKind::Conversion(conversion), value, None) => {
            convert_float_constant(conversion, value)
        }
        _ => unreachable!("constant call operands have the resolved intrinsic signature"),
    }
}
