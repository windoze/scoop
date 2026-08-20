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
        let export::ExprKind::Call { callee, args, .. } = &source.kind else {
            return None;
        };
        let implementation = match callee {
            export::CallableTarget::Local(callee) => {
                &self.source.functions[self.source.callable_function(*callee)].kind
            }
            export::CallableTarget::Application(application) => {
                let template = self.source.imported_generic_applications[*application].template;
                &self.source.imported_generic_templates[template].implementation
            }
            export::CallableTarget::Dependency(_) => return None,
        };
        if !args.is_empty()
            || !matches!(implementation,
            export::FunctionKind::Intrinsic(intrinsic)
                if intrinsic.kind == export::IntrinsicFunctionKind::CurrentSourceLocation)
        {
            return None;
        }

        let evaluation = origin.evaluation;
        let file = &self.source.source_files[evaluation.file as usize];
        assert_eq!(
            file.provider, evaluation.provider,
            "evaluation origin provider must match its source file"
        );
        let (line, column) = match &file.canonical_record {
            Some(record) => {
                let point = record
                    .point(u64::from(evaluation.span.start))
                    .expect("imported evaluation origins have canonical source points");
                (point.line(), point.column())
            }
            None => source_line_column(&file.source, evaluation.span.start),
        };
        let file_name = file.name.clone();
        let context = &self.source.source_contexts[evaluation.context];
        assert_eq!(
            context.source(),
            &file.identity,
            "evaluation context source must match its source file"
        );
        let (function_name, type_name) = self.source.source_context_names(evaluation.context);
        let concrete::TypeKind::Struct(location) = self.types[ty].kind else {
            unreachable!("current_source_location returns the SourceLocation struct")
        };
        let string_type = self.lower_type(self.source.string, substitution);
        let long_type = self.lower_integer_type(export::IntegerKind::SIGNED_64, substitution);
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
                            line,
                        )),
                        long_type,
                    ),
                    literal(
                        concrete::ExprKind::IntegerLiteral(export::HirIntegerConstant::Signed64(
                            column,
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

fn source_line_column(source: &str, offset: u32) -> (u64, u64) {
    let mut line = 1;
    let mut column = 1;
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
