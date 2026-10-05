use super::*;
use crate::constructor_resolution::NominalConstructorCall;
use crate::expr::CallSite;

impl Lowerer {
    pub(super) fn call_decoding_constructor(
        &mut self,
        result: TypeId,
        record: &DecodeRecord,
        values: Vec<hir::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.push_scope();
        let mut arguments = self.decoding_arguments(values, span, sink);
        if record.keyed {
            for (argument, parameter) in arguments.iter_mut().zip(&record.parameters) {
                argument.name = ast::CallArgumentName::Named(ast::Ident {
                    text: parameter.name.clone(),
                    span,
                });
            }
        }
        let value = match &record.constructor {
            DecodeConstructor::Current { source, .. } => {
                let owner_arguments = self
                    .nominal_application(result)
                    .expect("a constructor has a nominal result")
                    .arguments;
                self.resolve_nominal_constructor_overload(
                    &self.type_name(result),
                    &[*source],
                    NominalConstructorCall {
                        explicit_type_args: &[],
                        expected_type_args: Some(&owner_arguments),
                        arguments: &arguments,
                        span,
                    },
                    sink,
                )
                .map(|resolved| {
                    let kind = match resolved.source {
                        NominalConstructorSource::Struct(constructor) => {
                            let Type::Struct(application) = self.types[result] else {
                                unreachable!("a struct constructor returns its owner")
                            };
                            hir::ExprKind::StructInit {
                                constructor: self
                                    .struct_constructor_application(constructor, application),
                                args: resolved.args,
                            }
                        }
                        NominalConstructorSource::Class(constructor) => {
                            let Type::Class(application) = self.types[result] else {
                                unreachable!("a class constructor returns its owner")
                            };
                            hir::ExprKind::ClassInit {
                                constructor: self
                                    .class_constructor_application(constructor, application),
                                args: resolved.args,
                            }
                        }
                        NominalConstructorSource::Variant(variant) => {
                            let Type::Enum(application) = self.types[result] else {
                                unreachable!("a variant returns its enum")
                            };
                            hir::ExprKind::VariantConstruct {
                                variant: self.enum_variant_at(application, variant.local_index()),
                                args: resolved.args,
                            }
                        }
                        _ => {
                            unreachable!("automatic decoding selects ordinary source constructors")
                        }
                    };
                    self.decoding_expr(kind, result, span)
                })
            }
            DecodeConstructor::Dependency { declaration, .. } => {
                match self.probe_imported_value_constructor(
                    declaration.as_ref().clone(),
                    &ast::Ident {
                        text: declaration.name().into(),
                        span,
                    },
                    CallSite {
                        type_args: &[],
                        args: &arguments,
                        span,
                    },
                    Some(result),
                ) {
                    Ok(probe) => self.commit_imported_dependency_callable(probe, sink),
                    Err(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        None
                    }
                }
            }
        };
        self.pop_scope();
        value
    }

    pub(super) fn decoding_default(
        &mut self,
        constructor: &DecodeConstructor,
        default: DecodeDefault,
        previous: &[hir::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        match (constructor, default) {
            (DecodeConstructor::Current { bindings, .. }, DecodeDefault::Current(source)) => {
                self.instantiate_default(source, bindings, None, previous, span, sink)
            }
            (
                DecodeConstructor::Dependency {
                    declaration,
                    bindings,
                },
                DecodeDefault::Dependency(key),
            ) => {
                let template = declaration
                    .default_template(key)
                    .expect("the selected parameter retains its default")
                    .clone();
                let arguments = template
                    .type_parameters()
                    .arguments()
                    .iter()
                    .map(|argument| self.imported_signature_type_with_bindings(argument, bindings))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| {
                        self.error(span, format!("cannot resolve decoding default: {error:?}"))
                    })
                    .ok()?;
                let prepared = self
                    .prepare_imported_default_with_arguments(
                        declaration.as_ref(),
                        &template,
                        arguments,
                    )
                    .map_err(|error| {
                        self.error(span, format!("cannot prepare decoding default: {error}"))
                    })
                    .ok()?;
                self.materialize_imported_default(&prepared, None, previous, span, sink)
                    .map_err(|error| {
                        self.error(
                            span,
                            format!("cannot instantiate decoding default: {error}"),
                        )
                    })
                    .ok()
            }
            _ => unreachable!("a default belongs to its selected constructor"),
        }
    }
}
