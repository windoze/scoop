use super::*;
use crate::types::ArrayKind;

impl Lowerer {
    pub(crate) fn resolve_fn_param(&mut self, param: &ast::Param) -> Option<FnParam> {
        let (ty, calling) = self.resolve_parameter(&param.ty, &param.syntax)?;
        Some(FnParam {
            name: param.name.clone(),
            calling,
            ty,
        })
    }

    pub(crate) fn resolve_parameter(
        &mut self,
        ty: &ast::TypeRef,
        syntax: &ast::ParameterSyntax,
    ) -> Option<(TypeId, FnParamCalling)> {
        let declared_ty = self.resolve_type_ref(ty)?;
        let resolved = match syntax {
            ast::ParameterSyntax::Required => (declared_ty, FnParamCalling::Required),
            ast::ParameterSyntax::Default { expression, .. } => (
                declared_ty,
                FnParamCalling::Default {
                    expression: expression.clone(),
                },
            ),
            ast::ParameterSyntax::Vararg { default, .. } => {
                let ty = self.array_type(ArrayKind::Immutable, declared_ty);
                let omission = match default {
                    ast::VarargDefaultSyntax::EmptyWhenOmitted => FnVarargOmission::EmptyArray,
                    ast::VarargDefaultSyntax::Expression { expression, .. } => {
                        FnVarargOmission::Default {
                            expression: expression.clone(),
                        }
                    }
                };
                (
                    ty,
                    FnParamCalling::Vararg {
                        element_ty: declared_ty,
                        omission,
                    },
                )
            }
        };
        Some(resolved)
    }
}
