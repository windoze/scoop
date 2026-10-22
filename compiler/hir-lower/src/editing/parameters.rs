use super::*;
use crate::{FnParamCalling, FnVarargOmission};

impl DeclarationAnalysis {
    pub(super) fn parameters(&self, function: hir::FunctionId) -> Vec<hir::EditingParameter> {
        let file = self
            .lowerer
            .function_definition_file(function)
            .expect("source functions retain their definition file");
        self.lowerer.signatures[&function]
            .params
            .iter()
            .map(|parameter| {
                let (vararg_element_type, declared_default) = match &parameter.calling {
                    FnParamCalling::Required => (None, None),
                    FnParamCalling::Default { expression } => (None, Some(expression.span())),
                    FnParamCalling::Vararg {
                        element_ty,
                        omission,
                    } => {
                        let default = match omission {
                            FnVarargOmission::EmptyArray => None,
                            FnVarargOmission::Default { expression } => Some(expression.span()),
                        };
                        (Some(*element_ty), default)
                    }
                };
                hir::EditingParameter {
                    name: parameter.name.text.clone(),
                    value_type: parameter.ty,
                    vararg_element_type,
                    declared_default,
                    origin: hir::SemanticSourceLocation {
                        source: self.lowerer.source_identity(file).clone(),
                        span: parameter.name.span,
                    },
                }
            })
            .collect()
    }
}
