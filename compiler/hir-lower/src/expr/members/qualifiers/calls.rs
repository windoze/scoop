use super::*;

pub(in crate::expr) enum CurrentQualifiedCall {
    Struct {
        id: hir::StructId,
        ty: TypeId,
    },
    Class(hir::ClassId),
    Variant {
        target: hir::EnumVariantRef,
        expected: Option<TypeId>,
    },
    Singleton(hir::Expr),
}

impl Lowerer {
    pub(in crate::expr) fn resolve_current_qualified_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        type_args: &[ast::CallTypeArgument],
        expected: Option<TypeId>,
    ) -> Result<Option<CurrentQualifiedCall>, ()> {
        let applied = self.resolve_applied_qualifier(receiver)?;
        let direct_alias = self.resolve_direct_alias_qualifier(receiver)?;
        let qualifier = applied.map(|(_, target)| target).or_else(|| {
            direct_alias
                .as_ref()
                .map(|(_, target)| *target)
                .or_else(|| self.nominal_qualifier_target(receiver))
        });
        let Some(qualifier) = qualifier else {
            return Ok(None);
        };
        if let Some(target) = self.nested_nominal_target(qualifier.owner(), &name.text) {
            return self
                .resolve_static_nested_constructor(target, name)
                .map(Some)
                .ok_or(());
        }
        if let NominalTarget::Enum(enum_id) = qualifier
            && let Some(target) = self.find_variant_ref(enum_id, &name.text)
        {
            let expected = self
                .alias_fixed_expected(
                    direct_alias.as_ref().map(|(alias, _)| alias),
                    type_args,
                    applied.map(|(ty, _)| ty).or(expected),
                )
                .ok_or(())?;
            return Ok(Some(CurrentQualifiedCall::Variant { target, expected }));
        }
        let forwarded = self.companion_forwarding_object(qualifier, &name.text);
        let object = match qualifier {
            NominalTarget::Object(object) => Some(forwarded.unwrap_or(object)),
            _ => forwarded,
        };
        if let Some(object) = object {
            return self
                .lower_qualified_singleton(object, applied.map(|(ty, _)| ty), receiver.span())
                .map(CurrentQualifiedCall::Singleton)
                .map(Some)
                .ok_or(());
        }
        let member_kind = if matches!(qualifier, NominalTarget::Enum(_)) {
            "variant"
        } else {
            "nested type"
        };
        let owner_kind = if matches!(qualifier, NominalTarget::Enum(_)) {
            "enum"
        } else {
            "type"
        };
        self.error(
            name.span,
            format!(
                "{owner_kind} `{}` has no {member_kind} `{}`",
                qualifier.owner().describe_name(self),
                name.text
            ),
        );
        Err(())
    }
}
