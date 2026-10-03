use super::*;

pub(super) fn enum_variant_names(
    module: &Module,
    application: EnumVariantApplication,
) -> (&str, &str, &[TypeId], u32) {
    let Type::Enum(owner) = module.types[application.owner] else {
        unreachable!("an enum variant retains its complete enum owner")
    };
    let owner = &module.enum_applications[owner];
    let index = module.enum_variant_index(application);
    (
        module.enum_name(owner.template),
        &module.enum_definition(owner.template).variants[index].name,
        &owner.arguments,
        index as u32,
    )
}

pub(super) fn enum_field_name(module: &Module, field: EnumVariantFieldApplication) -> &str {
    let Type::Enum(owner) = module.types[field.variant.owner] else {
        unreachable!("an enum payload retains its complete enum owner")
    };
    let owner = &module.enum_applications[owner];
    &module.enum_definition(owner.template).variants[module.enum_variant_index(field.variant)]
        .fields[module.enum_field_index(field)]
    .name
}
