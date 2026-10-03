use std::collections::BTreeSet;

use scoop_identity::*;
use scoop_wire::{decode_canonical, encode};

use super::super::*;
use super::fixture::{ProviderFixture, coordinate, empty_alias_expansions, import_foundation};
use crate::{
    HirNativeBoundaryTypeDefinitionError as Error, NativeBoundaryNominalOwner as Owner,
    NativeBoundaryNominalShape as Shape, NativeBoundaryTypeDefinitionRecord as Record,
};

mod rejection;
mod support;
use support::*;

#[test]
fn native_producer_uses_private_reference_declarations_without_public_lookup() {
    let mut fixture = ProviderFixture::with_nominals(
        coordinate("private-reference"),
        PackagePath::root(),
        "Hidden",
        None,
    );
    fixture.interface = ProviderFixture::empty(fixture.coordinate.clone()).interface;
    fixture
        .foundation
        .set_native_boundary_types(vec![])
        .unwrap();
    with_world(&[], &[&fixture], |world| {
        assert!(world.nominal(fixture.outer.unwrap()).is_none());
        let records = close(world, owner(&fixture)).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].owner(), owner(&fixture));
        assert_eq!(records[0].shape(), &Shape::Reference);
    });
}

#[test]
fn native_producer_uses_actual_core_and_ordinary_source_shapes() {
    for coordinate in [
        ConeCoordinate::reserved_core(),
        coordinate("ordinary-native"),
    ] {
        let fixture = structure(
            coordinate,
            "Payload",
            false,
            |owner| vec![pointer(nominal(owner))],
            policy(),
        );
        with_world(&[&fixture], &[], |world| {
            let record = world
                .native_boundary_type_definition(owner(&fixture), Ok)
                .unwrap();
            let Shape::Struct { c_layout, fields } = record.shape() else {
                panic!("struct witness")
            };
            assert_eq!(*c_layout, policy().into());
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].ty(), &pointer(nominal(owner(&fixture))));
            assert_eq!(close(world, owner(&fixture)).unwrap(), vec![record]);
        });
    }
}

#[test]
fn native_producer_closes_mixed_direct_and_support_signature_dependencies() {
    let leaf = structure(
        ConeCoordinate::reserved_core(),
        "Leaf",
        false,
        |_| vec![],
        crate::NominalCLayoutPolicyV1::Ordinary,
    );
    let generic = structure(
        coordinate("generic-native"),
        "Box",
        true,
        |_| vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        crate::NominalCLayoutPolicyV1::Ordinary,
    );
    let Owner::GenericTemplate(origin) = owner(&generic) else {
        panic!("generic owner")
    };
    let application = SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(nominal(owner(&leaf)), []),
    };
    let choice = enumeration(
        coordinate("enum-native"),
        vec![pointer(SignatureTypeKey::Tuple(NonEmptyVec::from_first(
            nominal(owner(&leaf)),
            [application],
        )))],
    );
    let root = structure(
        coordinate("root-native"),
        "Root",
        false,
        |_| {
            vec![
                pointer(nominal(owner(&choice))),
                pointer(nominal(owner(&leaf))),
            ]
        },
        policy(),
    );
    with_world(&[&root, &leaf], &[&choice, &generic], |world| {
        let records = close(world, owner(&root)).unwrap();
        let reordered = with_world(&[&leaf, &root], &[&generic, &choice], |world| {
            close(world, owner(&root)).unwrap()
        });
        assert_eq!(
            records
                .iter()
                .map(|record| encode(record).unwrap())
                .collect::<Vec<_>>(),
            reordered
                .iter()
                .map(|record| encode(record).unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(records.len(), 4);
        assert!(
            records
                .windows(2)
                .all(|pair| pair[0].owner().compare_sort_key(pair[1].owner()).is_lt())
        );
        assert_eq!(
            records.iter().map(Record::owner).collect::<BTreeSet<_>>(),
            [&root, &leaf, &choice, &generic].map(owner).into()
        );
        let mut dump = records
            .iter()
            .map(|record| {
                let label = [&root, &leaf, &choice, &generic]
                    .into_iter()
                    .find(|fixture| owner(fixture) == record.owner())
                    .unwrap();
                let DeclarationName::Named(name) = label.outer_key.as_ref().unwrap().name() else {
                    panic!("named source")
                };
                match record.shape() {
                    Shape::Struct { c_layout, fields } => format!(
                        "{name} binders={} struct {c_layout:?} fields={}\n",
                        record.type_parameter_count(),
                        fields.len()
                    ),
                    Shape::Enum { variants } => format!(
                        "{name} binders={} enum fields={:?}\n",
                        record.type_parameter_count(),
                        variants
                            .iter()
                            .map(|v| v.fields().len())
                            .collect::<Vec<_>>()
                    ),
                    Shape::Reference | Shape::Intrinsic(_) => panic!("declared value fixture"),
                }
            })
            .collect::<Vec<_>>();
        dump.sort();
        assert_eq!(
            dump.concat(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-native-boundary/shared-closure.snap"
            ))
        );
        let mut pending = PendingIdentityValidation::new();
        let decoded = [&root, &leaf, &choice, &generic].map(|fixture| {
            pending.register_authority(fixture.identity()).unwrap();
            decode_canonical::<crate::DecodedHirFoundation>(&encode(&fixture.foundation).unwrap())
                .unwrap()
        });
        for foundation in &decoded {
            foundation.register_identities(&mut pending).unwrap();
        }
        for foundation in &decoded {
            foundation.resolve_identities(&mut pending).unwrap();
        }
        let mut graph = pending.finish().unwrap();
        for record in records {
            let bytes = encode(&record).unwrap();
            let decoded: crate::DecodedNativeBoundaryTypeDefinitionRecord =
                decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
            assert_eq!(decoded.resolve(&mut graph).unwrap(), record);
        }
        assert_eq!(world.direct_packages().package_count(), 0);
    });
}

#[test]
fn native_producer_missing_transitive_provider_does_not_publish_a_partial_closure() {
    let dependency = structure(
        coordinate("absent-native"),
        "Missing",
        false,
        |_| vec![],
        crate::NominalCLayoutPolicyV1::Ordinary,
    );
    let root = structure(
        coordinate("root-native"),
        "Root",
        false,
        |_| vec![pointer(nominal(owner(&dependency)))],
        policy(),
    );
    with_world(&[&root], &[], |world| {
        assert!(
            matches!(close(world, owner(&root)), Err(Error::MissingSourceNominal { owner: missing }) if missing == owner(&dependency))
        );
    });
}

fn close(world: &ImportedSemanticWorld<'_>, root: Owner) -> Result<Vec<Record>, Error> {
    crate::persistent_native_boundary::closure::close([root].into(), |owner| {
        world.native_boundary_type_definition(owner, Ok)
    })
}
