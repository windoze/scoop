use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(in crate::expr) fn lower_imported_member_property_read(
        &mut self,
        receiver: &hir::Expr,
        name: &ast::Ident,
        span: ast::Span,
        expected: Option<hir::TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        if self.imported_nominal_declaration(receiver.ty).is_none() {
            return Ok(None);
        }
        let candidates = self
            .imported_member_candidates(
                receiver.ty,
                hir::ImportedMemberLookup::PropertyGetter(&name.text),
            )
            .map_err(|error| {
                self.error(name.span, format!("invalid dependency property: {error}"))
            })?;
        let mut candidates = candidates.into_iter();
        let Some(candidate) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            self.error(
                name.span,
                format!("ambiguous dependency property `{}`", name.text),
            );
            return Err(());
        }
        let interface = candidate.interface();
        let result_type = self
            .imported_property_signature_type(interface.result(), "dependency property", span)
            .ok_or(())?;
        if let Some(expected) = expected
            && !self.is_subtype(result_type, expected)
        {
            self.error(
                span,
                format!(
                    "dependency property `{}` has type {}, expected {}",
                    name.text,
                    self.type_name(result_type),
                    self.type_name(expected)
                ),
            );
            return Err(());
        }
        if interface.effects().safety() == hir::CallableSafetyV1::Unsafe {
            self.require_unsafe_operation(span, "reading an unsafe dependency property");
        }
        let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(owner)) =
            interface.owner()
        else {
            unreachable!("a resolved dependency property has a nominal owner")
        };
        let owner = self
            .imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(owner))
            .expect("a dependency property receiver type was resolved during lookup");
        let argument = self.adapt_to(receiver.clone(), owner);
        let callee = self
            .select_imported_callable_declaration_use(candidate)
            .map_err(|error| {
                self.error(span, format!("invalid dependency property getter: {error}"));
            })?;
        Ok(Some(hir::Expr {
            kind: hir::ExprKind::ImportedDependencyCall {
                callee,
                binding: None,
                args: vec![argument],
                receiver: hir::SourceCallReceiver::Receiver {
                    static_type: receiver.ty,
                },
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        }))
    }
}
