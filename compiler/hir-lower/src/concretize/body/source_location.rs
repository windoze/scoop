//! Materialization of the source-location intrinsic.

use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_current_source_location(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        ty: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> Option<concrete::Expr> {
        let export::CoreProtocols::Defined(protocols) = self.core else {
            return None;
        };
        let export::ExprKind::Call {
            callee: export::CallableTarget::Local(callee),
            args,
            ..
        } = &source.kind
        else {
            return None;
        };
        if !args.is_empty()
            || self.source.callable_function(*callee) != protocols.source_location.current
        {
            return None;
        }

        let evaluation = origin.evaluation;
        let file = &self.source.source_files[evaluation.file as usize];
        assert_eq!(
            file.provider, evaluation.provider,
            "evaluation origin provider must match its source file"
        );
        let (line, column) = source_line_column(&file.source, evaluation.span.start);
        let file_name = file.name.clone();
        let context = &self.source.source_contexts[evaluation.context];
        assert_eq!(
            context.source(),
            &file.identity,
            "evaluation context source must match its source file"
        );
        let (function_name, type_name) = self.source.source_context_names(evaluation.context);
        let location_application =
            self.source.structs[protocols.source_location.location].self_application;
        let location = self.lower_struct_application(location_application, substitution);
        let string_type = self.lower_type(self.source.string, substitution);
        let long_type = self.lower_integer_type(export::IntegerKind::SIGNED_64, substitution);
        assert_eq!(
            self.struct_type[&location], ty,
            "current_source_location return type must be SourceLocation"
        );
        let literal = |kind, ty| concrete::Expr {
            kind,
            ty,
            span: source.span,
            origin,
        };
        Some(concrete::Expr {
            kind: concrete::ExprKind::StructInit {
                struct_id: location,
                args: vec![
                    literal(
                        concrete::ExprKind::StringLiteral {
                            value: file_name,
                            owner: export::StringConstantOwner::CurrentDefinition,
                        },
                        string_type,
                    ),
                    literal(
                        concrete::ExprKind::IntegerLiteral(export::HirIntegerConstant::Signed64(
                            line as u64,
                        )),
                        long_type,
                    ),
                    literal(
                        concrete::ExprKind::IntegerLiteral(export::HirIntegerConstant::Signed64(
                            column as u64,
                        )),
                        long_type,
                    ),
                    literal(
                        concrete::ExprKind::StringLiteral {
                            value: function_name,
                            owner: export::StringConstantOwner::CurrentDefinition,
                        },
                        string_type,
                    ),
                    literal(
                        concrete::ExprKind::StringLiteral {
                            value: type_name,
                            owner: export::StringConstantOwner::CurrentDefinition,
                        },
                        string_type,
                    ),
                ],
            },
            ty,
            span: source.span,
            origin,
        })
    }
}

fn source_line_column(source: &str, offset: u32) -> (i64, i64) {
    let mut line = 1_i64;
    let mut column = 1_i64;
    for (index, character) in source.char_indices() {
        if index as u32 >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
