use super::*;
use crate::expr::MemberCallKind;
use hir::ImportedCallableSource;

mod write;

#[derive(Clone)]
pub(crate) struct ResolvedImportedMemberProperty {
    getter: hir::ImportedCallableDeclaration,
    capability: hir::PropertyCapabilityV1,
    pub(crate) value_type: hir::TypeId,
}

impl ResolvedImportedMemberProperty {
    pub(crate) fn has_setter(&self) -> bool {
        self.capability.setter().is_some()
    }
}

impl Lowerer {
    pub(crate) fn resolve_imported_member_property(
        &mut self,
        receiver: hir::TypeId,
        name: &ast::Ident,
    ) -> Result<Option<ResolvedImportedMemberProperty>, ()> {
        self.resolve_imported_member_receiver(receiver, name.span)?;
        let candidates = self
            .imported_member_candidates(
                receiver,
                hir::ImportedMemberLookup::PropertyGetter(&name.text),
            )
            .map_err(|error| {
                self.error(name.span, format!("invalid dependency property: {error}"))
            })?;
        let mut candidates = candidates.into_iter();
        let Some(getter) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            self.error(
                name.span,
                format!("ambiguous dependency property `{}`", name.text),
            );
            return Err(());
        }
        let scoop_identity::CallableTemplateOrigin::Accessor(accessor) =
            getter.interface().declaration()
        else {
            unreachable!("a dependency property getter refers to an accessor")
        };
        let capability = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.property_for_accessor(accessor))
            .expect("the member getter belongs to the selected property")
            .capability();
        let value_type = self
            .imported_property_signature_type(
                getter.interface().result(),
                "dependency property",
                name.span,
            )
            .ok_or(())?;
        Ok(Some(ResolvedImportedMemberProperty {
            getter,
            capability,
            value_type,
        }))
    }

    pub(in crate::expr) fn lower_imported_member_property_read(
        &mut self,
        receiver: &hir::Expr,
        name: &ast::Ident,
        span: ast::Span,
        expected: Option<hir::TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let Some(property) = self.resolve_imported_member_property(receiver.ty, name)? else {
            return Ok(None);
        };
        if let Some(expected) = expected
            && !self.is_subtype(property.value_type, expected)
        {
            self.error(
                span,
                format!(
                    "dependency property `{}` has type {}, expected {}",
                    name.text,
                    self.type_name(property.value_type),
                    self.type_name(expected)
                ),
            );
            return Err(());
        }
        self.emit_imported_member_property_read(&property, receiver.clone(), span)
            .map(Some)
            .ok_or(())
    }

    pub(crate) fn emit_imported_member_property_read(
        &mut self,
        property: &ResolvedImportedMemberProperty,
        receiver: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        self.emit_imported_member_property_read_with_kind(
            property,
            receiver,
            span,
            MemberCallKind::Ordinary,
        )
    }

    pub(crate) fn emit_imported_member_property_read_with_kind(
        &mut self,
        property: &ResolvedImportedMemberProperty,
        receiver: hir::Expr,
        span: ast::Span,
        kind: MemberCallKind,
    ) -> Option<hir::Expr> {
        self.emit_imported_member_accessor(
            property.getter.clone(),
            receiver,
            Vec::new(),
            property.value_type,
            span,
            "reading an unsafe dependency property",
            kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_imported_member_accessor(
        &mut self,
        candidate: hir::ImportedCallableDeclaration,
        receiver: hir::Expr,
        values: Vec<hir::Expr>,
        result_type: hir::TypeId,
        span: ast::Span,
        unsafe_operation: &str,
        kind: MemberCallKind,
    ) -> Option<hir::Expr> {
        let interface = candidate.interface();
        if kind == MemberCallKind::DirectSuper
            && interface.modality() == hir::CallableModalityV1::Abstract
        {
            self.error(
                span,
                "abstract interface accessor cannot be called with qualified `super`".into(),
            );
            return None;
        }
        if interface.effects().safety() == hir::CallableSafetyV1::Unsafe {
            self.require_unsafe_operation(span, unsafe_operation);
        }
        let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(owner)) =
            interface.owner()
        else {
            unreachable!("a resolved dependency property has a nominal owner")
        };
        let owner = self
            .imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(owner))
            .expect("a dependency property receiver type was resolved during lookup");
        let static_type = receiver.ty;
        let mut args = Vec::with_capacity(values.len() + 1);
        args.push(self.adapt_to(receiver, owner));
        args.extend(values);
        let callee = self
            .select_imported_callable_declaration_use_with_kind(candidate, kind)
            .map_err(|error| {
                self.error(
                    span,
                    format!("invalid dependency property accessor: {error}"),
                );
            })
            .ok()?;
        Some(hir::Expr {
            kind: hir::ExprKind::ImportedDependencyCall {
                callee,
                binding: None,
                args,
                receiver: hir::SourceCallReceiver::Receiver { static_type },
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        })
    }
}
