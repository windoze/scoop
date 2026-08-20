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
        let mut element_sinks = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            let hint = expected_elements.as_ref().map(|expected| expected[index]);
            let mut setup = Vec::new();
            lowered.push(self.lower_expr(element, &mut setup, hint)?);
            element_sinks.push(setup);
        }
        let slots = lowered
            .iter()
            .enumerate()
            .map(|(index, element)| {
                if self.is_nothing_ty(element.ty)
                    && let Some(expected) = &expected_elements
                {
                    expected[index]
                } else {
                    element.ty
                }
            })
            .collect();
        let ty = self.intern_type(Type::Tuple(slots));
        let lowered = self.materialize_aggregate_elements(lowered, element_sinks, sink);
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
            let mut element_sinks = Vec::with_capacity(elements.len());
            for element in elements {
                let mut setup = Vec::new();
                let element = self.lower_expr(element, &mut setup, Some(element_ty))?;
                let would_auto_box = self.is_value_ty(element.ty)
                    && self.is_ref_ty(element_ty)
                    && !self.types_equal(element.ty, element_ty);
                if !self.is_subtype(element.ty, element_ty) || would_auto_box {
                    let expected = self.type_name(element_ty);
                    let found = self.type_name(element.ty);
                    let message = self.with_nominal_invariance_detail(
                        format!("array literal element must be of type {expected}, found {found}"),
                        element.ty,
                        element_ty,
                    );
                    self.error(element.span, message);
                    return None;
                }
                lowered.push(self.adapt_to(element, element_ty));
                element_sinks.push(setup);
            }
            let lowered = self.materialize_aggregate_elements(lowered, element_sinks, sink);
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
        let literal_elements = elements.iter().collect::<Vec<_>>();
        let all_direct_literals = literal_elements
            .iter()
            .all(|element| crate::expr::integer_literal_candidate_kinds(element).is_some());
        let common_literal = crate::expr::common_integer_literal_kind(&literal_elements)
            .map(|kind| self.integer_type(kind));
        let mut lowered: Vec<Option<hir::Expr>> = (0..elements.len()).map(|_| None).collect();
        let mut element_sinks: Vec<Vec<hir::Statement>> =
            (0..elements.len()).map(|_| Vec::new()).collect();
        if let Some(hint) = common_literal {
            for (index, element) in elements.iter().enumerate() {
                lowered[index] =
                    Some(self.lower_expr(element, &mut element_sinks[index], Some(hint))?);
            }
        } else if all_direct_literals {
            for (index, element) in elements.iter().enumerate() {
                lowered[index] = Some(self.lower_expr(element, &mut element_sinks[index], None)?);
            }
        } else {
            for (index, element) in elements.iter().enumerate() {
                if !self.expr_requires_expected_type(element) {
                    lowered[index] =
                        Some(self.lower_expr(element, &mut element_sinks[index], None)?);
                }
            }
            if lowered.iter().all(Option::is_none)
                && let Some(seed) = elements
                    .iter()
                    .enumerate()
                    .filter_map(|(index, element)| {
                        self.expr_default_seed_rank(element)
                            .map(|rank| (rank, index))
                    })
                    .max_by_key(|(rank, _)| *rank)
                    .map(|(_, index)| index)
            {
                lowered[seed] =
                    Some(self.lower_expr(&elements[seed], &mut element_sinks[seed], None)?);
            }
            let hint = self.array_element_hint(&lowered);
            for (index, element) in elements.iter().enumerate() {
                if lowered[index].is_none() {
                    lowered[index] =
                        Some(self.lower_expr(element, &mut element_sinks[index], hint)?);
                }
            }
        }
        let lowered = lowered
            .into_iter()
            .map(|element| element.expect("every array element was lowered"))
            .collect::<Vec<_>>();
        let first_ty = lowered
            .iter()
            .find(|element| !self.is_nothing_ty(element.ty))
            .unwrap_or(&lowered[0])
            .ty;
        if lowered.iter().any(|element| self.is_value_ty(element.ty)) {
            for element in &lowered {
                if self.is_nothing_ty(element.ty) || self.types_equal(first_ty, element.ty) {
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
        let lowered = self.materialize_aggregate_elements(lowered, element_sinks, sink);
        let ty = self.array_type(ArrayKind::Immutable, element_ty);
        Some(hir::Expr {
            kind: ExprKind::ArrayLiteral(lowered),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// Each element completes before the next element's statement expansion.
    fn materialize_aggregate_elements(
        &mut self,
        elements: Vec<hir::Expr>,
        setups: Vec<Vec<hir::Statement>>,
        sink: &mut Vec<hir::Statement>,
    ) -> Vec<hir::Expr> {
        if setups.iter().all(Vec::is_empty) {
            return elements;
        }
        elements
            .into_iter()
            .zip(setups)
            .enumerate()
            .map(|(index, (element, setup))| {
                sink.extend(setup);
                let span = element.span;
                self.materialize_temporary(format!("$aggregate.{index}"), element, span, sink)
            })
            .collect()
    }

    /// Derive the provisional array element from expressions already lowered
    /// in this fixed point. Value types must agree exactly; reference types
    /// use the same least upper bound as the final array validation.
    fn array_element_hint(&mut self, elements: &[Option<hir::Expr>]) -> Option<TypeId> {
        let types = elements
            .iter()
            .filter_map(|element| element.as_ref().map(|element| element.ty))
            .filter(|ty| !self.is_nothing_ty(*ty))
            .collect::<Vec<_>>();
        let &first = types.first()?;
        if types.iter().any(|ty| self.is_value_ty(*ty)) {
            return types
                .iter()
                .all(|ty| self.types_equal(*ty, first))
                .then_some(first);
        }
        Some(self.reference_lob(&types))
    }

    /// `receiver[indices]` enters the ordinary typed `operator get`
    /// resolver. Core arrays participate through source declarations and are
    /// normalized only after their registered intrinsic target wins.
    pub(super) fn lower_index_read(
        &mut self,
        receiver: &ast::Expr,
        indices: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(receiver, sink, None)?;
        let arguments = indices
            .iter()
            .cloned()
            .map(ast::CallArgument::positional)
            .collect::<Vec<_>>();
        self.lower_named_call_on_receiver(
            receiver,
            &ast::Ident {
                text: "get".to_string(),
                span,
            },
            CallSite {
                type_args: &[],
                args: &arguments,
                span,
            },
            sink,
            None,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Get),
                infix: false,
                ..Default::default()
            },
        )
    }
}
