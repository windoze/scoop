use super::*;
use crate::expr::MemberCallKind;
use hir::ImportedCallableSource;

mod write;

#[derive(Clone)]
pub(crate) struct ResolvedImportedMemberProperty {
    getter: hir::ImportedCallableDeclaration,
    accessors: hir::PropertyAccessorsV1,
    storage: Option<hir::FieldRef>,
    pub(crate) value_type: hir::TypeId,
}

impl ResolvedImportedMemberProperty {
    pub(crate) fn has_setter(&self) -> bool {
        self.accessors.setter().is_some()
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
        let property = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.property_for_accessor(accessor))
            .expect("the member getter belongs to the selected property")
            .clone();
        let accessors = property.accessors();
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = getter.interface().owner() else {
            unreachable!("member properties retain their nominal owner")
        };
        let owner = self
            .imported_member_owner_type(receiver, owner)
            .expect("the selected property occurs in the receiver hierarchy");
        let arguments = self.types[owner]
            .imported_nominal_application()
            .map(|(_, arguments)| arguments)
            .unwrap_or(&[]);
        let bindings = arguments
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    scoop_identity::SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    *ty,
                )
            })
            .collect();
        let value_type = self
            .imported_signature_type_with_bindings(getter.interface().result(), &bindings)
            .map_err(|error| self.error(name.span, error.diagnostic("dependency property")))?;
        let storage = match (&self.types[owner], property.declaration()) {
            (
                hir::Type::ImportedClass(class),
                scoop_identity::PropertyOwner::Property(property),
            ) if !class.arguments.is_empty() => class
                .declaration
                .field_sources
                .iter()
                .zip(&class.fields)
                .find_map(|(source, field)| {
                    (source.backing_property == Some(property)).then_some(
                        hir::FieldRef::ImportedClass {
                            owner,
                            field: field.identity,
                        },
                    )
                }),
            _ => None,
        };
        Ok(Some(ResolvedImportedMemberProperty {
            getter,
            accessors,
            storage,
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
        if let Some(field) = property.storage
            && property.accessors.getter_source().implementation()
                == hir::PropertyAccessorImplementationV1::Storage
            && (property.getter.interface().modality() == hir::CallableModalityV1::Final
                || kind == MemberCallKind::DirectSuper)
        {
            if property.getter.interface().effects().safety() == hir::CallableSafetyV1::Unsafe {
                self.require_unsafe_operation(span, "reading an unsafe dependency property");
            }
            return Some(hir::Expr {
                kind: hir::ExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                },
                ty: property.value_type,
                span,
                origin: self.expression_origin(span),
            });
        }
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
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = interface.owner() else {
            unreachable!("a resolved member property has a nominal owner")
        };
        let owner_type = self
            .imported_member_owner_type(receiver.ty, owner)
            .expect("a resolved property belongs to the receiver hierarchy");
        if matches!(owner, hir::SourceNominalId::GenericTemplate(_))
            && (candidate.callable_body().is_some()
                || interface.modality() == hir::CallableModalityV1::Abstract)
        {
            let arguments = hir::ImportedCallableArguments::Method {
                owner: owner_type,
                method_arguments: Vec::new(),
            };
            let template = self
                .request_imported_generic_template(candidate)
                .map_err(|error| self.error(span, error))
                .ok()?;
            let application =
                self.imported_generic_applications
                    .alloc(hir::ImportedGenericCallableApplication {
                        template,
                        arguments,
                    });
            let static_type = receiver.ty;
            let mut args = vec![self.adapt_to(receiver, owner_type)];
            args.extend(values);
            return Some(hir::Expr {
                kind: hir::ExprKind::ImportedGenericCall {
                    application,
                    kind: match kind {
                        MemberCallKind::Ordinary => hir::ImportedGenericCallKind::Ordinary,
                        MemberCallKind::DirectSuper => hir::ImportedGenericCallKind::DirectSuper,
                    },
                    binding: None,
                    args,
                    receiver: hir::SourceCallReceiver::Receiver { static_type },
                },
                ty: result_type,
                span,
                origin: self.expression_origin(span),
            });
        }
        let owner = owner_type;
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
