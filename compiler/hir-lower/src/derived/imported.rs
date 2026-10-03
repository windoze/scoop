use super::*;
use hir::ImportedCallableSource;
use scoop_identity::SignatureTypeKey;

impl Lowerer {
    pub(super) fn imported_equality_candidate(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> Result<Option<DerivedEqualityCandidate>, String> {
        if let Some(target) = self.imported_derived_equality(ty) {
            return Ok(Some(DerivedEqualityCandidate::Imported(target)));
        }
        // Only a failed candidate needs field diagnostics. Reuse the ordinary
        // derivation in a discarded probe; no consumer body is published.
        let mut probe = self.clone();
        probe.build_derived_equality_body(ty, span, &mut vec![ty])?;
        Err(format!(
            "the defining Cone does not export the derived `equals` body for `{}`",
            self.type_name(ty)
        ))
    }

    pub(super) fn imported_derived_equality(
        &self,
        ty: hir::TypeId,
    ) -> Option<hir::ImportedDerivedEquality> {
        let (declaration, arguments) = self.dependency_nominal_application(ty)?;
        if !arguments.is_empty() {
            return None;
        }
        let hir::SourceNominalId::Concrete(owner) = declaration.owner() else {
            return None;
        };
        self.dependencies.as_ref()?.derived_equality(owner)
    }

    pub(crate) fn imported_equality_call(
        &mut self,
        target: hir::ImportedDerivedEquality,
        lhs: hir::Expr,
        rhs: hir::Expr,
        span: ast::Span,
    ) -> hir::Expr {
        let owner = lhs.ty;
        hir::Expr {
            kind: hir::ExprKind::MethodCall {
                receiver: Box::new(lhs),
                callee: self.imported_equality_callee(target, owner),
                args: vec![rhs],
            },
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(crate) fn imported_equality_callee(
        &mut self,
        target: hir::ImportedDerivedEquality,
        owner: hir::TypeId,
    ) -> hir::MethodCallee {
        let existing = self
            .imported_derived_equalities
            .iter()
            .find_map(|(id, value)| (*value == target).then_some(id));
        let target = existing.unwrap_or_else(|| self.imported_derived_equalities.alloc(target));
        hir::MethodCallee::ImportedDerivedEquality { target, owner }
    }

    pub(super) fn has_imported_same_type_equals(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<bool, String> {
        let (declaration, arguments) = self
            .dependency_nominal_application(ty)
            .expect("an imported equality owner retains its declaration");
        let owner = declaration.owner();
        let bindings = arguments
            .iter()
            .enumerate()
            .map(|(index, &argument)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    argument,
                )
            })
            .collect();
        let candidates = self
            .dependencies
            .as_ref()
            .expect("an imported equality owner retains its provider")
            .member_callable_candidates(
                owner,
                hir::ImportedMemberLookup::Operator(hir::CallableOperatorRoleV1::Language(
                    hir::CallableOperatorV1::Equals,
                )),
            )
            .map_err(|error| error.to_string())?;
        for candidate in candidates {
            let [parameter] = candidate.interface().parameters().parameters() else {
                continue;
            };
            let parameter = self
                .imported_signature_type_with_bindings(parameter.value_type(), &bindings)
                .map_err(|error| error.diagnostic("equals parameter"))?;
            if self.types_equal(parameter, ty) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
