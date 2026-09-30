use super::*;

pub(super) struct ResolvedEnumDefinition<'a> {
    pub origin: export::HirNominalIdentity,
    pub name: String,
    pub owner: Option<concrete::NominalOwner>,
    pub variants: Vec<ResolvedEnumVariant<'a>>,
    pub interfaces: &'a [export::TypeId],
    pub interface_implementations: &'a [export::InterfaceImplementation],
    pub methods: &'a [export::FunctionId],
    pub span: scoop_ast::Span,
}

pub(super) struct ResolvedEnumVariant<'a> {
    pub identity: scoop_identity::PersistentEnumVariantId,
    pub name: &'a str,
    pub fields: Vec<ResolvedEnumField<'a>>,
}

pub(super) struct ResolvedEnumField<'a> {
    pub identity: scoop_identity::PersistentEnumVariantFieldId,
    pub name: &'a str,
    pub ty: export::TypeId,
}

impl<'input> Concretizer<'input> {
    pub(super) fn source_enum_definition(
        &self,
        id: export::EnumId,
    ) -> ResolvedEnumDefinition<'input> {
        let declaration = &self.source.enums[id];
        let variants = declaration
            .variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let reference = export::EnumVariantRef::checked(
                    &self.source.enums,
                    id,
                    u32::try_from(index).expect("source variant indices fit u32"),
                )
                .expect("a declared enum retains each variant");
                let fields = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let reference = export::EnumVariantFieldRef::checked(
                            &self.source.enums,
                            reference,
                            u32::try_from(index).expect("source field indices fit u32"),
                        )
                        .expect("a declared variant retains each payload field");
                        ResolvedEnumField {
                            identity: self.source.enum_member_identities[reference].id(),
                            name: &field.name,
                            ty: field.ty,
                        }
                    })
                    .collect();
                ResolvedEnumVariant {
                    identity: self.source.enum_member_identities[reference].id(),
                    name: &variant.name,
                    fields,
                }
            })
            .collect();
        ResolvedEnumDefinition {
            origin: self.source.nominal_identities[id].clone(),
            name: self.source_nominal_name(&declaration.name, declaration.owner),
            owner: self.lower_nominal_owner(declaration.owner),
            variants,
            interfaces: &declaration.interfaces,
            interface_implementations: &declaration.interface_implementations,
            methods: &declaration.methods,
            span: declaration.span,
        }
    }
}

impl<'a> ResolvedEnumDefinition<'a> {
    pub(super) fn from_dependency(source: &'a export::ImportedEnumType) -> Self {
        Self {
            origin: export::HirNominalIdentity::Source(source.declaration.identity.clone()),
            name: source.declaration.name().to_owned(),
            owner: None,
            variants: source
                .variants
                .iter()
                .map(|variant| ResolvedEnumVariant {
                    identity: variant.identity,
                    name: &variant.name,
                    fields: variant
                        .fields
                        .iter()
                        .map(|field| ResolvedEnumField {
                            identity: field.identity,
                            name: &field.name,
                            ty: field.ty,
                        })
                        .collect(),
                })
                .collect(),
            interfaces: &source.interfaces,
            interface_implementations: &source.interface_implementations,
            methods: &[],
            span: scoop_ast::Span::new(0, 0),
        }
    }
}
