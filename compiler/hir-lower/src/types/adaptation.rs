use super::*;

impl Lowerer {
    /// The least representable upper bound of a non-empty set of
    /// reference types (spec 10.3). The current type system has no
    /// intersection types: if several incomparable minimal common
    /// supertypes remain, `Any` is the only representable result.
    pub(crate) fn reference_lob(&mut self, element_types: &[TypeId]) -> TypeId {
        debug_assert!(!element_types.is_empty());
        debug_assert!(element_types.iter().all(|&ty| self.is_ref_ty(ty)));

        self.least_upper_bound(element_types)
    }

    /// The least representable upper bound used by structured expression
    /// branches. Unlike array inference this also accepts value types;
    /// `is_subtype` includes their boxing conversions to interfaces/`Any`.
    /// With no intersection types, incomparable minimal bounds fall back to
    /// `Any`.
    pub(crate) fn least_upper_bound(&mut self, types: &[TypeId]) -> TypeId {
        debug_assert!(!types.is_empty());

        let mut common_supertypes = Vec::new();
        let candidates: Vec<TypeId> = self.types.iter().map(|(id, _)| id).collect();
        for candidate in candidates {
            if !types
                .iter()
                .all(|&element| self.is_subtype(element, candidate))
                || common_supertypes
                    .iter()
                    .any(|&existing| self.types_equal(existing, candidate))
            {
                continue;
            }
            common_supertypes.push(candidate);
        }

        let minimal: Vec<TypeId> = common_supertypes
            .iter()
            .copied()
            .filter(|&candidate| {
                !common_supertypes.iter().copied().any(|other| {
                    !self.types_equal(other, candidate) && self.is_subtype(other, candidate)
                })
            })
            .collect();
        if minimal.len() == 1 {
            minimal[0]
        } else {
            self.any
        }
    }

    /// Adapt an expression to a target type it is a subtype of (callers
    /// check `is_subtype` first and diagnose otherwise): a value type
    /// crossing into a reference target is boxed (`ExprKind::Box`,
    /// target in `Expr::ty`); a reference crossing to a supertype is a
    /// zero-cost retype. Equal types pass through unchanged.
    pub(crate) fn adapt_to(&mut self, expr: hir::Expr, target: TypeId) -> hir::Expr {
        if self.types_equal(expr.ty, target) {
            return expr;
        }
        let span = expr.span;
        if let (Type::Function(source), Type::Function(target_type)) =
            (self.types[expr.ty].clone(), self.types[target].clone())
        {
            let key = (source, target_type);
            let coercion = self
                .function_coercion_by_types
                .get(&key)
                .copied()
                .unwrap_or_else(|| {
                    let id = self.function_coercions.alloc(hir::FunctionCoercion {
                        source,
                        target: target_type,
                    });
                    self.function_coercion_by_types.insert(key, id);
                    id
                });
            return hir::Expr {
                kind: hir::ExprKind::FunctionCoercion {
                    source: Box::new(expr),
                    coercion,
                    target_type,
                },
                ty: target,
                span,
                origin: self.expression_origin(span),
            };
        }
        if self.is_value_ty(expr.ty) {
            hir::Expr {
                kind: hir::ExprKind::Box(Box::new(expr)),
                ty: target,
                span,
                origin: self.expression_origin(span),
            }
        } else {
            hir::Expr {
                kind: expr.kind,
                ty: target,
                span,
                origin: self.expression_origin(span),
            }
        }
    }
}
