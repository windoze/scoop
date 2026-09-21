use scoop_identity::{ConeCoordinate, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, certificate, coordinate, empty_alias_expansions, import_foundation,
};

#[test]
fn support_exact_lookup_covers_object_values_and_enum_variants() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let support = ProviderFixture::with_value_shapes(coordinate("value-support"));
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 9);
    let support_foundation = import_foundation(&mut session, &support, 10);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("value-current").identity().unwrap(),
        vec![DirectImportedProviderInput::from_validated(
            certificate(&core.coordinate, 9),
            &core_foundation,
            &core.interface,
            &aliases,
        )],
        vec![SupportImportedProviderInput::from_validated(
            certificate(&support.coordinate, 10),
            &support_foundation,
            &support.interface,
            &aliases,
        )],
    )
    .unwrap();

    let object = support.object_value.unwrap();
    let variant = support.enum_variant.unwrap();
    let typed = world.support_provider(support.identity()).unwrap().typed();
    let imported_object = typed.object_value(object).unwrap();
    assert_eq!(imported_object.identity().persistent(), object);
    assert_eq!(
        imported_object.owner().persistent(),
        imported_object.owner_record().declaration()
    );
    let imported_variant = typed.enum_variant(variant).unwrap();
    assert_eq!(imported_variant.identity().persistent(), variant);
    assert_eq!(imported_variant.record().variant(), variant);
    assert_eq!(
        imported_variant.owner().persistent(),
        imported_variant.owner_record().declaration()
    );

    assert_eq!(
        world.object_value(object).unwrap().identity().persistent(),
        object
    );
    assert_eq!(
        world.enum_variant(variant).unwrap().identity().persistent(),
        variant
    );
    assert_eq!(world.direct_packages().package_count(), 0);
}
