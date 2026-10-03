use scoop_identity::{ExactTypeKey, SpecializationKey};

use super::*;

fn exact_type_fixture(include_noise: bool) -> hir::Output {
    let mut declarations = Vec::new();
    if include_noise {
        declarations.push(struct_decl("Noise", Vec::new()));
    }
    declarations.extend([
        generic_struct_decl(
            "Pair",
            vec!["T"],
            vec![("first", ty_named("T")), ("second", ty_named("T"))],
        ),
        fun(
            "main",
            vec![
                val("pair", struct_init("Pair", vec![int_lit(1), int_lit(2)])),
                val("tuple", tuple_lit(vec![int_lit(3), str_lit("value")])),
            ],
        ),
    ]);
    lower_user_output(file(declarations))
        .expect("closed nominal applications and structural types must lower")
}

fn concrete_pair_type(module: &hir::concrete::Module) -> hir::concrete::TypeId {
    let pair = module
        .structs
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "Pair").then_some(id))
        .expect("Pair<Int> is materialized");
    module
        .types
        .iter()
        .find_map(|(id, ty)| {
            matches!(ty.kind, hir::concrete::TypeKind::Struct(found) if found == pair).then_some(id)
        })
        .expect("Pair<Int> has one canonical concrete type")
}

#[test]
fn local_concrete_types_have_total_exact_identities() {
    let output = exact_type_fixture(false);
    let module = &output.local;
    assert_eq!(module.exact_type_identities.len(), module.types.len());
    assert!(
        module
            .types
            .iter()
            .all(|(id, _)| module.exact_type_identities.get(id).is_some())
    );

    let pair_type = concrete_pair_type(module);
    let pair = match module.types[pair_type].kind {
        hir::concrete::TypeKind::Struct(pair) => pair,
        _ => unreachable!("the helper returns a struct type"),
    };
    let pair_argument = module.structs[pair].type_arguments[0];
    let export_pair = output
        .export
        .structs
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "Pair").then_some(id))
        .expect("Pair source template exists");
    let origin = output.export.nominal_identities[export_pair]
        .generic_type_id()
        .expect("Pair is a generic nominal template");
    let ExactTypeKey::NominalApplication {
        origin: actual_origin,
        arguments,
    } = module.exact_type_identities[pair_type].key()
    else {
        panic!("Pair<Int> must have a nominal-application identity")
    };
    assert_eq!(*actual_origin, origin);
    assert_eq!(
        arguments.as_slice(),
        &[module.exact_type_identities[pair_argument].id()]
    );
    let specialization = module
        .exact_type_identities
        .nominal_specialization(pair_type)
        .expect("Pair<Int> has one nominal specialization group");
    assert!(matches!(
        specialization.key(),
        SpecializationKey::Nominal {
            origin: specialization_origin,
            arguments: specialization_arguments,
        } if specialization_origin == actual_origin && specialization_arguments == arguments
    ));

    let tuple_type = module
        .types
        .iter()
        .find_map(|(id, ty)| {
            matches!(&ty.kind, hir::concrete::TypeKind::Tuple(elements) if elements.len() == 2)
                .then_some(id)
        })
        .expect("the tuple expression materializes its exact type");
    let hir::concrete::TypeKind::Tuple(elements) = &module.types[tuple_type].kind else {
        unreachable!("the selected concrete type is a tuple")
    };
    let ExactTypeKey::Tuple(identities) = module.exact_type_identities[tuple_type].key() else {
        panic!("the tuple must retain its structural exact-type key")
    };
    assert_eq!(
        identities.as_slice(),
        elements
            .iter()
            .map(|element| module.exact_type_identities[*element].id())
            .collect::<Vec<_>>()
    );
    assert!(
        module
            .exact_type_identities
            .nominal_specialization(tuple_type)
            .is_none(),
        "structural types create an ODR group only when a generated materialization needs one"
    );
    let specialization_records = module
        .exact_type_identities
        .nominal_specialization_records();
    assert!(
        specialization_records
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
    );
    assert!(
        specialization_records
            .iter()
            .any(|record| record == specialization)
    );
    assert_eq!(
        specialization_records.len(),
        module
            .types
            .iter()
            .filter(|(id, _)| matches!(
                module.exact_type_identities[*id].key(),
                ExactTypeKey::NominalApplication { .. }
            ))
            .count()
    );
    assert!(module.types.iter().all(|(id, _)| {
        module
            .exact_type_identities
            .nominal_specialization(id)
            .is_some()
            == matches!(
                module.exact_type_identities[id].key(),
                ExactTypeKey::NominalApplication { .. }
            )
    }));
}

#[test]
fn exact_type_identity_ignores_unrelated_declaration_insertion() {
    let baseline = exact_type_fixture(false);
    let shifted = exact_type_fixture(true);
    assert_eq!(
        baseline.local.exact_type_identities[concrete_pair_type(&baseline.local)].id(),
        shifted.local.exact_type_identities[concrete_pair_type(&shifted.local)].id()
    );
}
