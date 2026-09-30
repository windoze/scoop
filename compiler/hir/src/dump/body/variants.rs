use super::*;

pub(super) fn enum_variant_names(
    module: &Module,
    application: EnumVariantApplication,
) -> (&str, &str, &[TypeId], u32) {
    match &module.types[application.owner] {
        Type::Enum(owner) => {
            let owner = &module.enum_applications[*owner];
            let variant = module
                .enum_member_identities
                .variant_declaration(application.variant)
                .expect("a current variant has its declaration identity");
            let declaration = &module.enums[variant.enumeration()];
            (
                &declaration.name,
                &declaration.variants[variant.local_index() as usize].name,
                &owner.arguments,
                variant.local_index(),
            )
        }
        Type::ImportedEnum(owner) => {
            let index = owner
                .variants
                .iter()
                .position(|variant| variant.identity == application.variant)
                .expect("a variant belongs to its original enum declaration");
            (
                owner.declaration.name(),
                &owner.variants[index].name,
                &owner.arguments,
                index as u32,
            )
        }
        _ => unreachable!("an enum variant retains its complete enum owner"),
    }
}

pub(super) fn enum_field_name(module: &Module, field: EnumVariantFieldApplication) -> &str {
    match &module.types[field.variant.owner] {
        Type::Enum(_) => {
            let declaration = module
                .enum_member_identities
                .field_declaration(field.field)
                .expect("a current payload field has its declaration identity");
            let variant = declaration.variant();
            &module.enums[variant.enumeration()].variants[variant.local_index() as usize].fields
                [declaration.local_index() as usize]
                .name
        }
        Type::ImportedEnum(owner) => {
            &owner
                .variants
                .iter()
                .find(|variant| variant.identity == field.variant.variant)
                .expect("a payload belongs to its original variant")
                .fields
                .iter()
                .find(|value| value.identity == field.field)
                .expect("a payload retains its original field identity")
                .name
        }
        _ => unreachable!("an enum payload retains its complete enum owner"),
    }
}
