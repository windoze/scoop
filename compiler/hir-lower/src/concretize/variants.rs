use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_enum_variant(
        &mut self,
        source: export::EnumVariantApplication,
        substitution: &[concrete::TypeId],
    ) -> concrete::EnumVariantRef {
        let owner = self.lower_type(source.owner, substitution);
        let concrete::TypeKind::Enum(enumeration) = self.types[owner].kind else {
            unreachable!("an enum variant retains its complete enum owner")
        };
        let index = self.enums[enumeration]
            .variants
            .iter()
            .position(|variant| variant.identity == source.variant)
            .expect("an enum application retains its original variant");
        concrete::EnumVariantRef::checked(
            &self.enums,
            enumeration,
            concrete::VariantId::from_raw(index as u32),
        )
        .expect("the variant belongs to this enum representation")
    }

    pub(super) fn lower_enum_variant_field(
        &mut self,
        source: export::EnumVariantFieldApplication,
        substitution: &[concrete::TypeId],
    ) -> concrete::EnumVariantFieldRef {
        let variant = self.lower_enum_variant(source.variant, substitution);
        let index = self.enums[variant.enumeration()].variants
            [variant.variant().into_raw() as usize]
            .fields
            .iter()
            .position(|field| field.identity == source.field)
            .expect("a variant retains its original payload field");
        concrete::EnumVariantFieldRef::checked(&self.enums, variant, index as u32)
            .expect("the field belongs to this variant representation")
    }

    pub(super) fn lower_applied_enum_variant_ref(
        &mut self,
        source: export::AppliedEnumVariantRef,
        substitution: &[concrete::TypeId],
    ) -> concrete::EnumVariantRef {
        self.lower_enum_variant(
            export::EnumVariantApplication {
                owner: self.source.enum_applications[source.application()].canonical_type,
                variant: self.source.enum_member_identities[source.declaration()].id(),
            },
            substitution,
        )
    }

    pub(super) fn lower_applied_enum_variant_field_ref(
        &mut self,
        source: export::AppliedEnumVariantFieldRef,
        substitution: &[concrete::TypeId],
    ) -> concrete::EnumVariantFieldRef {
        let variant = source.variant();
        self.lower_enum_variant_field(
            export::EnumVariantFieldApplication {
                variant: export::EnumVariantApplication {
                    owner: self.source.enum_applications[variant.application()].canonical_type,
                    variant: self.source.enum_member_identities[variant.declaration()].id(),
                },
                field: self.source.enum_member_identities[source.declaration()].id(),
            },
            substitution,
        )
    }
}
