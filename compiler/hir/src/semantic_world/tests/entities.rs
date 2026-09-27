use scoop_identity::{ConeCoordinate, SemanticIdentitySession};

use super::super::*;
use super::fixture::{ProviderFixture, coordinate, empty_alias_expansions, import_foundation};

#[test]
fn support_exact_lookup_covers_object_values_and_enum_variants() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let support = ProviderFixture::with_value_shapes(coordinate("value-support"));
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 9);
    let support_foundation = import_foundation(&mut session, &support, 10);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_dependencies(
        coordinate("value-current").identity().unwrap(),
        vec![ImportedProviderInput {
            foundation: &core_foundation,
            interface: &core.interface,
            alias_expansions: &aliases,
        }],
        vec![ImportedProviderInput {
            foundation: &support_foundation,
            interface: &support.interface,
            alias_expansions: &aliases,
        }],
    )
    .unwrap();

    let object = support.object_value.unwrap();
    let variant = support.enum_variant.unwrap();
    let imported_object = world.object_value(object).unwrap();
    assert_eq!(imported_object.identity().persistent(), object);
    assert_eq!(
        imported_object.owner().persistent(),
        imported_object.owner_record().declaration()
    );
    let imported_variant = world.enum_variant(variant).unwrap();
    assert_eq!(imported_variant.identity().persistent(), variant);
    assert_eq!(imported_variant.record().variant(), variant);
    assert_eq!(
        imported_variant.owner().persistent(),
        imported_variant.owner_record().declaration()
    );

    assert_eq!(world.direct_packages().package_count(), 0);
}
