use scoop_identity::{GeneratedNominalKey, SourceNominalKind};

use super::*;
use crate::SourceNominalId;

#[test]
fn source_object_requires_matching_representation_generated_key_and_exact_key() {
    let mut fixture = Fixture::default();
    let object = fixture.add("Object", SourceNominalKind::Object, &[]);
    let other = fixture.add("Other", SourceNominalKind::Object, &[]);
    let SourceNominalId::Concrete(object_id) = object.source else {
        unreachable!()
    };
    let SourceNominalId::Concrete(other_id) = other.source else {
        unreachable!()
    };
    let own_representation = fixture.representations[&object_id].clone();
    fixture
        .representations
        .insert(object_id, fixture.representations[&other_id].clone());
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::ObjectBacking(_))
    ));
    fixture
        .representations
        .insert(object_id, own_representation);
    let (&backing, _) = fixture
        .generated
        .iter()
        .find(|(_, key)| **key == GeneratedNominalKey::ObjectBackingClass { object: object_id })
        .unwrap();
    let own_key = fixture
        .generated
        .insert(
            backing,
            GeneratedNominalKey::ObjectBackingClass { object: other_id },
        )
        .unwrap();
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::ObjectBacking(_))
    ));
    fixture.generated.insert(backing, own_key);
    let backing_exact =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(backing)).unwrap();
    fixture.exacts.remove(&backing_exact);
    assert!(matches!(
        CheckedNominalInheritanceGraphV1::validate(
            fixture.records.values(),
            &fixture,
            &mut meter()
        ),
        Err(InheritanceGraphError::Foundation("unknown exact"))
    ));
}
