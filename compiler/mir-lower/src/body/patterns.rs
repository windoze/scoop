//! Pattern tests and binding extraction over structured MIR expressions.

use super::*;

impl BodyLowerer<'_> {
    /// Lower a pattern matching the value at `root` + `path` (of MIR
    /// type `ty`) into ordered decision steps and binding initializers.
    /// Bindings are emitted only after every test succeeds. Each nested enum
    /// value is first placed in an immutable local so its typed payload
    /// projections are dominated by the matching `VariantTest` true edge.
    pub(super) fn lower_pattern(
        &mut self,
        pattern: &hir::Pattern,
        root: mir::LocalId,
        path: &mut Vec<Access>,
        ty: &mir::Type,
        steps: &mut Vec<smir::PatternDecisionStep>,
        bindings: &mut Vec<(mir::LocalId, smir::Expr)>,
    ) {
        match pattern {
            hir::Pattern::Binding { local } => {
                let init = self.accessed(root, path);
                bindings.push((self.local_map[local], init));
            }
            hir::Pattern::Wildcard => {}
            hir::Pattern::Literal {
                value,
                equality,
                subject_ty,
            } => {
                debug_assert_eq!(self.lower_type(*subject_ty), *ty);
                let test = match equality {
                    hir::LiteralPatternEquality::Integer { kind, .. } => {
                        let kind = lower_integer_kind(*kind);
                        debug_assert_eq!(*ty, mir::Type::Integer(kind));
                        smir::Expr::integer_compare(
                            mir::IntegerComparisonOperation::new(
                                kind,
                                mir::IntegerComparisonOperator::Equal,
                            ),
                            self.accessed(root, path),
                            self.lower_expr(value),
                        )
                    }
                    hir::LiteralPatternEquality::Ordinary { equals } => {
                        let function = self.module.callable_function(*equals);
                        let callee = self.instances.get(function).map_or_else(
                            || mir::Callee::User(self.function_map[&function]),
                            mir::Callee::Monomorphized,
                        );
                        smir::Expr::new(
                            mir::Type::Boolean,
                            smir::ExprKind::Call(smir::Call {
                                target: mir::CallTarget {
                                    kind: mir::CallKind::Direct,
                                    callee,
                                },
                                args: vec![self.accessed(root, path), self.lower_expr(value)],
                                return_ty: mir::Type::Boolean,
                            }),
                        )
                    }
                };
                steps.push(smir::PatternDecisionStep::Test(test));
            }
            hir::Pattern::Variant {
                variant, fields, ..
            } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant pattern matches an enum value")
                };
                let enum_id = *enum_id;
                let variant = variant.into_raw();
                let variant_ref = self.enums.variant_ref(enum_id, variant);
                let saved_path = if path.is_empty() {
                    None
                } else {
                    let init = self.accessed(root, path);
                    let local = self.new_hidden("pattern", ty.clone(), false);
                    steps.push(smir::PatternDecisionStep::Materialize { local, init });
                    Some((local, std::mem::take(path)))
                };
                let variant_root = saved_path.as_ref().map_or(root, |(local, _)| *local);
                steps.push(smir::PatternDecisionStep::Test(smir::Expr::variant_test(
                    &self.enums.defs,
                    smir::Expr::local(variant_root, ty.clone()),
                    variant_ref,
                )));
                for (index, sub) in fields {
                    let field = self.enums.variant_field_ref(variant_ref, *index);
                    let field_ty = field
                        .definition(&self.enums.defs)
                        .expect("a checked variant field has a MIR definition")
                        .ty
                        .clone();
                    path.push(Access::VariantField(field));
                    self.lower_pattern(sub, variant_root, path, &field_ty, steps, bindings);
                    path.pop();
                }
                if let Some((_, saved_path)) = saved_path {
                    *path = saved_path;
                }
            }
            hir::Pattern::Tuple(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple pattern matches a tuple value")
                };
                let element_types = element_types.clone();
                for (index, sub) in elements.iter().enumerate() {
                    path.push(Access::Field(index as u32));
                    self.lower_pattern(sub, root, path, &element_types[index], steps, bindings);
                    path.pop();
                }
            }
            hir::Pattern::Struct { fields, .. } => {
                let mir::Type::Struct(struct_id) = ty else {
                    unreachable!("a struct pattern matches a struct value")
                };
                let field_types: Vec<mir::Type> = self.structs.defs[*struct_id]
                    .declared_fields()
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                for (index, sub) in fields {
                    path.push(Access::Field(*index));
                    self.lower_pattern(
                        sub,
                        root,
                        path,
                        &field_types[*index as usize],
                        steps,
                        bindings,
                    );
                    path.pop();
                }
            }
        }
    }
}
