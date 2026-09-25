//! Shared declaration fixtures exercise nonempty direct inheritance partitions.

use super::*;
use crate::*;
use scoop_identity::*;
use scoop_wire::{DecodeLimits, ResourceKind, WireErrorKind};

mod fixture;
mod negative;
mod resources;
use fixture::{Artifact, Loaded, coordinate, exact, meter, selected};

#[test]
fn shared_type_uses_rebuild_class_and_interface_edges_for_every_provider() {
    for coordinate in [ConeCoordinate::reserved_core(), coordinate("ordinary")] {
        let mut source = Artifact::new(coordinate);
        let base = source.nominal("Base", SourceNominalKind::Class, &[]);
        let interface = source.nominal("Interface", SourceNominalKind::Interface, &[]);
        let provider = source.load(&[]);
        for (parent, edge) in [
            (
                base,
                SelectedDirectInheritanceEdgeV1::ClassBase { exact: exact(base) },
            ),
            (
                interface,
                SelectedDirectInheritanceEdgeV1::Interface {
                    exact: exact(interface),
                },
            ),
        ] {
            let mut source = Artifact::new(coordinate_for_consumer());
            let derived = source.nominal("Derived", SourceNominalKind::Class, &[parent]);
            let consumer = source.load(&[&provider]);
            let expected = selected(vec![
                SelectedExternalTypeUseV1::new(
                    provider.provider(),
                    SelectedTypeUseV1::Representation {
                        exact: exact(parent),
                    },
                ),
                SelectedExternalTypeUseV1::new(
                    provider.provider(),
                    SelectedTypeUseV1::Inheritance {
                        derived: exact(derived),
                        edge,
                    },
                ),
            ]);
            assert_eq!(consumer.uses(&[&provider]).unwrap(), expected);
            consumer
                .validate(&expected, &[&provider], &mut meter())
                .unwrap();
        }
    }
}

#[test]
fn shared_type_uses_preserve_each_current_derived_and_do_not_claim_transitive_edges() {
    let mut source = Artifact::new(coordinate("ancestor"));
    let ancestor = source.nominal("Ancestor", SourceNominalKind::Class, &[]);
    let ancestor_provider = source.load(&[]);
    let mut source = Artifact::new(coordinate("middle"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[ancestor]);
    let interface = source.nominal("Interface", SourceNominalKind::Interface, &[]);
    let middle = source.load(&[&ancestor_provider]);
    let mut source = Artifact::new(coordinate_for_consumer());
    let first = source.nominal("First", SourceNominalKind::Class, &[base, interface]);
    let second = source.nominal("Second", SourceNominalKind::Class, &[base, interface]);
    source.nominal("LocalChild", SourceNominalKind::Class, &[first]);
    let mut consumer = source.load(&[&ancestor_provider, &middle]);
    let actual = consumer.uses(&[&ancestor_provider, &middle]).unwrap();
    let mut expected = vec![
        SelectedExternalTypeUseV1::new(
            ancestor_provider.provider(),
            SelectedTypeUseV1::Representation {
                exact: exact(ancestor),
            },
        ),
        SelectedExternalTypeUseV1::new(
            middle.provider(),
            SelectedTypeUseV1::Representation { exact: exact(base) },
        ),
        SelectedExternalTypeUseV1::new(
            middle.provider(),
            SelectedTypeUseV1::Representation {
                exact: exact(interface),
            },
        ),
    ];
    for derived in [first, second] {
        for edge in [
            SelectedDirectInheritanceEdgeV1::ClassBase { exact: exact(base) },
            SelectedDirectInheritanceEdgeV1::Interface {
                exact: exact(interface),
            },
        ] {
            expected.push(SelectedExternalTypeUseV1::new(
                middle.provider(),
                SelectedTypeUseV1::Inheritance {
                    derived: exact(derived),
                    edge,
                },
            ));
        }
    }
    assert_eq!(actual, selected(expected));
    let reversed = consumer.uses(&[&middle, &ancestor_provider]).unwrap();
    assert_eq!(actual, reversed);
    let wire = scoop_wire::encode(&actual).unwrap();
    let decoded: DecodedCanonicalSelectedExternalTypeUsesV1 =
        scoop_wire::decode_canonical(&wire, DecodeLimits::default()).unwrap();
    let decoded = decoded
        .resolve(&mut consumer.identities, &mut meter(), &WirePath::root())
        .unwrap();
    assert_eq!(actual, decoded);
    let snapshot = actual
        .records()
        .iter()
        .map(|record| {
            let usage = match record.usage() {
                SelectedTypeUseV1::Representation { exact } => {
                    format!("Representation {exact}")
                }
                SelectedTypeUseV1::Inheritance { derived, edge } => match edge {
                    SelectedDirectInheritanceEdgeV1::ClassBase { exact } => {
                        format!("Inheritance derived={derived} ClassBase {exact}")
                    }
                    SelectedDirectInheritanceEdgeV1::Interface { exact } => {
                        format!("Inheritance derived={derived} Interface {exact}")
                    }
                },
                _ => panic!("the fixture contains only representations and inheritance"),
            };
            format!("{} {usage}\n", record.provider())
        })
        .collect::<String>();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-type-uses/inheritance.snap");
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_USES").is_some() {
        std::fs::write(&path, &snapshot).unwrap();
    }
    assert_eq!(snapshot, std::fs::read_to_string(path).unwrap());
}

fn coordinate_for_consumer() -> ConeCoordinate {
    coordinate("consumer")
}
