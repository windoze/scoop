use super::*;

#[test]
fn expanded_local_descriptors_must_agree_on_the_immutable_declaration() {
    with_sources(SOURCE, |output, _, _, _| {
        let original = output.output().export.module();
        let local = original
            .local_functions
            .iter()
            .find(|(_, local)| local.function_type != local.declaration_function_type)
            .unwrap()
            .0;
        check(original).unwrap();
        for change in 0..4 {
            let mut module = original.clone();
            let descriptor = &mut module.local_functions[local];
            let function = descriptor.function;
            match change {
                0 => descriptor.declaration_function_type = descriptor.function_type,
                1 => descriptor
                    .owner_type_arguments
                    .push(descriptor.captures[0].ty),
                2 => descriptor.origin.span.start += 1,
                3 => descriptor.captures[0].binding = hir::BindingId::from_raw(u32::MAX),
                _ => unreachable!(),
            }
            assert!(
                matches!(check(&module), Err(hir::HirFunctionIdentityError::ConflictingClaim {
                function: actual,
                first: hir::FunctionIdentityRelation::LocalFunction,
                second: hir::FunctionIdentityRelation::LocalFunction,
            }) if actual == function.into_raw().into_u32()),
                "change {change}"
            );
        }
    });
}

fn check(
    module: &hir::Module,
) -> Result<hir::HirFunctionIdentities, hir::HirFunctionIdentityError> {
    hir::HirFunctionIdentities::checked(
        hir::HirFunctionIdentityInputs {
            functions: &module.functions,
            lambdas: &module.lambdas,
            anonymous_functions: &module.anonymous_functions,
            local_functions: &module.local_functions,
            property_getters: &module.property_getters,
            property_setters: &module.property_setters,
            property_accessor_identities: &module.property_accessor_identities,
            initialization_units: &module.initialization_units,
            initialization_unit_identities: &module.initialization_unit_identities,
            derived_equality_applications: &module.derived_equality_applications,
            structs: &module.structs,
            enums: &module.enums,
            type_identities: &module.type_identities,
            struct_constructors: &module.struct_constructors,
            class_constructors: &module.class_constructors,
            constructor_identities: &module.constructor_identities,
            enum_member_identities: &module.enum_member_identities,
        },
        module
            .function_identities
            .iter()
            .map(|(_, identity)| identity.clone())
            .collect(),
    )
}
