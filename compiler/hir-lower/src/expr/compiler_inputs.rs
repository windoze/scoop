//! Definition-site expansion of compiler input calls after ordinary resolution.

use super::*;

#[derive(Clone)]
pub(crate) struct ConstantCallContext {
    pub file: usize,
    pub span: Span,
    pub intrinsic: Option<hir::IntrinsicFunctionKind>,
}

impl Lowerer {
    pub(crate) fn record_constant_call(
        &mut self,
        span: Span,
        intrinsic: Option<hir::IntrinsicFunctionKind>,
    ) {
        if let Some(context) = &mut self.constant_call_context
            && context.file == self.current_file
            && context.span == span
        {
            context.intrinsic = intrinsic;
        }
    }

    pub(in crate::expr) fn normalize_compile_env(
        &mut self,
        arguments: &[hir::Expr],
        source: Option<&ast::Expr>,
        result_type: hir::TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let [name] = arguments else {
            unreachable!("the compile_env signature has one String parameter")
        };
        let value = match source {
            Some(source) => self.compiler_input_string(source),
            None => match self.expression_constant(name) {
                Some(hir::HirConstantImage::String(value)) => Some(value),
                _ => None,
            },
        };
        let Some(value) = value else {
            self.error(
                name.span,
                "`env` requires a compile-time constant String name".into(),
            );
            return None;
        };
        let name = match hir::CompileTimeEnvironmentName::parse(&value) {
            Ok(name) => name,
            Err(message) => {
                self.error(name.span, message.into());
                return None;
            }
        };
        let Some(inputs) = &self.compile_time_inputs else {
            self.error(
                span,
                "no compile-time input host was supplied for `env`".into(),
            );
            return None;
        };
        let value = match inputs.environment(&name) {
            Ok(value) => value,
            Err(message) => {
                self.error(arguments[0].span, message);
                return None;
            }
        };
        let hir::Type::Enum(owner) = self.types[result_type] else {
            unreachable!("compile_env returns the checked Option<String> type")
        };
        let variant = match (&self.core, value.is_some()) {
            (crate::CoreLoweringAuthority::Imported(protocols), true) => {
                protocols.option().some().persistent()
            }
            (crate::CoreLoweringAuthority::Imported(protocols), false) => {
                protocols.option().none().persistent()
            }
            (_, present) => {
                let option = self.option_core.expect("Option is validated before bodies");
                let variant = if present {
                    option.some()
                } else {
                    option.none()
                };
                self.enum_member_identities
                    .as_ref()
                    .expect("enum identities are complete")[variant]
                    .id()
            }
        };
        let value = hir::HirConstantImage::Enum {
            variant: hir::EnumVariantApplication { owner, variant },
            fields: value
                .into_iter()
                .map(hir::HirConstantImage::String)
                .collect(),
        };
        Some(self.constant_expression(
            value,
            result_type,
            hir::StringConstantOwner::CurrentDefinition,
            span,
            self.expression_origin(span),
        ))
    }
}
