use super::*;

impl Lowerer {
    pub(super) fn lower_tuple_literal(
        &mut self,
        elements: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        // The parser never produces an empty tuple literal (`()` is a
        // `UnitLiteral`); reject it here so every AST shape is handled.
        if elements.is_empty() {
            self.error(
                span,
                "tuple literal must contain at least one element".to_string(),
            );
            return None;
        }
        let expected_elements = expected.and_then(|ty| match &self.types[ty] {
            Type::Tuple(expected) if expected.len() == elements.len() => Some(expected.clone()),
            _ => None,
        });
        let mut lowered = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            let hint = expected_elements.as_ref().map(|expected| expected[index]);
            lowered.push(self.lower_expr(element, sink, hint)?);
        }
        let ty = self.intern_type(Type::Tuple(
            lowered.iter().map(|element| element.ty).collect(),
        ));
        Some(hir::Expr {
            kind: ExprKind::TupleLiteral(lowered),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// `[e1, ...]` (spec 10.2/10.3, milestone5 DESIGN.md 2.2). With an
    /// expected `Array<U>` / `MutableArray<U>` the literal takes that
    /// kind and every element must be a subtype of `U`, except that a
    /// value element may not cross into a reference type by implicit
    /// boxing (an empty literal is only legal in this case). Without an
    /// array expectation, value elements require exact equality while
    /// an all-reference literal infers their least upper bound. (An
    /// expected type of any other shape is ignored: the context's own
    /// check reports the mismatch.)
    pub(super) fn lower_array_literal(
        &mut self,
        elements: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let expected_array =
            expected.and_then(|ty| self.array_type_info(ty).map(|array| (ty, array.element)));
        if let Some((array_ty, element_ty)) = expected_array {
            let mut lowered = Vec::with_capacity(elements.len());
            for element in elements {
                let element = self.lower_expr(element, sink, Some(element_ty))?;
                let would_auto_box = self.is_value_ty(element.ty)
                    && self.is_ref_ty(element_ty)
                    && !self.types_equal(element.ty, element_ty);
                if !self.is_subtype(element.ty, element_ty) || would_auto_box {
                    let expected = self.type_name(element_ty);
                    let found = self.type_name(element.ty);
                    self.error(
                        element.span,
                        format!("array literal element must be of type {expected}, found {found}"),
                    );
                    return None;
                }
                lowered.push(self.adapt_to(element, element_ty));
            }
            return Some(hir::Expr {
                kind: ExprKind::ArrayLiteral(lowered),
                ty: array_ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        if elements.is_empty() {
            self.error(
                span,
                "cannot infer the element type of an empty array literal".to_string(),
            );
            return None;
        }
        let mut lowered = Vec::with_capacity(elements.len());
        for element in elements {
            lowered.push(self.lower_expr(element, sink, None)?);
        }
        let first_ty = lowered[0].ty;
        if lowered.iter().any(|element| self.is_value_ty(element.ty)) {
            for element in &lowered[1..] {
                if self.types_equal(first_ty, element.ty) {
                    continue;
                }
                let first = self.type_name(first_ty);
                let found = self.type_name(element.ty);
                self.error(
                    element.span,
                    format!(
                        "array literal elements must have the same type, found {first} and {found}"
                    ),
                );
                return None;
            }
        }
        let element_ty = if self.is_ref_ty(first_ty) {
            let element_types: Vec<TypeId> = lowered.iter().map(|element| element.ty).collect();
            self.reference_lob(&element_types)
        } else {
            first_ty
        };
        let lowered = lowered
            .into_iter()
            .map(|element| self.adapt_to(element, element_ty))
            .collect();
        let ty = self.array_type(ArrayKind::Immutable, element_ty);
        Some(hir::Expr {
            kind: ExprKind::ArrayLiteral(lowered),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// `receiver[index]` (spec 10.5): the receiver must be an
    /// `Array<T>` / `MutableArray<T>` and the index an `Int`; the
    /// result is the element type `T`.
    pub(super) fn lower_index_read(
        &mut self,
        receiver: &ast::Expr,
        indices: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let [index] = indices else {
            self.error(
                span,
                format!(
                    "array subscript takes exactly one index, but {} were supplied",
                    indices.len()
                ),
            );
            return None;
        };
        let receiver = self.lower_expr(receiver, sink, None)?;
        let Some(element_ty) = self.array_element_ty(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                receiver.span,
                format!("subscript is only supported on arrays, found {found}"),
            );
            return None;
        };
        let index = self.lower_expr(index, sink, Some(self.int))?;
        if index.ty != self.int {
            let found = self.type_name(index.ty);
            self.error(
                index.span,
                format!("array index must be Int, found {found}"),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::Index {
                receiver: Box::new(receiver),
                index: Box::new(index),
            },
            ty: element_ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
