//! Pattern tests and binding extraction over structured MIR expressions.

use super::*;

impl BodyLowerer<'_> {
    /// Lower a pattern matching the value at `root` + `path` (of MIR
    /// type `ty`): returns the match condition (`None` when the
    /// pattern matches unconditionally) and appends the binding
    /// initializers — `local = <value at path>` — in declaration
    /// order. CFG normalization expands the condition's `&&` chain, so a
    /// variant field is only extracted once its tag test has passed.
    pub(super) fn lower_pattern(
        &mut self,
        pattern: &hir::Pattern,
        root: mir::LocalId,
        path: &mut Vec<Access>,
        ty: &mir::Type,
        bindings: &mut Vec<(mir::LocalId, smir::Expr)>,
    ) -> Option<smir::Expr> {
        match pattern {
            hir::Pattern::Binding { local } => {
                let init = self.accessed(root, path);
                bindings.push((self.local_map[local], init));
                None
            }
            hir::Pattern::Wildcard => None,
            hir::Pattern::Literal {
                value,
                equals,
                subject_ty,
            } => {
                debug_assert_eq!(self.lower_type(*subject_ty), *ty);
                let function = self.module.callable_function(*equals);
                let callee = self.instances.get(function).map_or_else(
                    || mir::Callee::User(self.function_map[&function]),
                    mir::Callee::Monomorphized,
                );
                Some(smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee,
                        },
                        args: vec![self.accessed(root, path), self.lower_expr(value)],
                        return_ty: mir::Type::Boolean,
                    }),
                ))
            }
            hir::Pattern::Variant {
                variant, fields, ..
            } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant pattern matches an enum value")
                };
                let enum_id = *enum_id;
                let variant = variant.into_raw();
                let tag = smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::EnumTag(Box::new(self.accessed(root, path))),
                );
                let mut cond = smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Binary {
                        op: mir::BinOp::IntEq,
                        lhs: Box::new(tag),
                        rhs: Box::new(smir::Expr::int(i64::from(variant))),
                    },
                );
                for (index, sub) in fields {
                    let field_ty = self.enums.defs[enum_id].variants[variant as usize].fields
                        [*index as usize]
                        .ty
                        .clone();
                    path.push(Access::EnumField {
                        variant,
                        index: *index,
                    });
                    if let Some(sub_cond) = self.lower_pattern(sub, root, path, &field_ty, bindings)
                    {
                        cond = and(cond, sub_cond);
                    }
                    path.pop();
                }
                Some(cond)
            }
            hir::Pattern::Tuple(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple pattern matches a tuple value")
                };
                let element_types = element_types.clone();
                let mut cond: Option<smir::Expr> = None;
                for (index, sub) in elements.iter().enumerate() {
                    path.push(Access::Field(index as u32));
                    if let Some(sub_cond) =
                        self.lower_pattern(sub, root, path, &element_types[index], bindings)
                    {
                        cond = Some(match cond {
                            None => sub_cond,
                            Some(acc) => and(acc, sub_cond),
                        });
                    }
                    path.pop();
                }
                cond
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
                let mut cond: Option<smir::Expr> = None;
                for (index, sub) in fields {
                    path.push(Access::Field(*index));
                    if let Some(sub_cond) =
                        self.lower_pattern(sub, root, path, &field_types[*index as usize], bindings)
                    {
                        cond = Some(match cond {
                            None => sub_cond,
                            Some(acc) => and(acc, sub_cond),
                        });
                    }
                    path.pop();
                }
                cond
            }
        }
    }
}
