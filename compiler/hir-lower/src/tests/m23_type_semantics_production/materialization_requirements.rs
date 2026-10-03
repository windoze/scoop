use super::*;
use hir::{
    NominalMaterializationClosureError as Error, NominalMaterializationRequirementV1 as Requirement,
};
use scoop_identity::{PersistentTypeId, ValidatedIdentityGraph};
use scoop_wire::{decode_canonical, encode};

mod dispatch;
mod validation;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-nominal-requirements/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-nominal-requirements/combined.scoop"
));

#[test]
fn shared_machine_requirements_preserve_roles_and_positions_after_wire_round_trip() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        source_dispatch::with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let mut identities = source_inventory::identity_closure(output);
            let nominals: hir::DecodedCanonicalNominalInterfacesV1 =
                decode_canonical(&encode(public.nominal_interfaces()).unwrap()).unwrap();
            let nominals = nominals.resolve(&mut identities).unwrap();
            let callables: hir::DecodedCanonicalCallableInterfacesV1 =
                decode_canonical(&encode(public.callable_interfaces()).unwrap()).unwrap();
            let callables = callables.resolve(&mut identities).unwrap();
            assert_eq!(&nominals, public.nominal_interfaces());
            assert_eq!(&callables, public.callable_interfaces());
            let counts = check_requirements(&nominals, &callables);
            assert_eq!(
                counts,
                if case == "combined" {
                    [3, 0, 6, 6, 6]
                } else {
                    [4, 2, 0, 4, 0]
                }
            );
        });
    }
}

fn check_requirements(
    nominals: &hir::CanonicalNominalInterfacesV1,
    callables: &hir::CanonicalCallableInterfacesV1,
) -> [usize; 5] {
    let mut counts = [0; 5];
    let mut fields = BTreeSet::new();
    let mut variant_fields = BTreeSet::new();
    nominals
        .visit_materialization_requirements::<Error>(callables, |requirement| {
            let owner = nominals
                .declaration(hir::SourceNominalId::Concrete(requirement.owner()))
                .unwrap();
            match requirement {
                Requirement::Field { field, .. } => {
                    assert!(fields.insert(field.field()));
                    assert!(owner.source_shape().declared_fields().contains(field));
                    counts[0] += 1;
                }
                Requirement::EnumVariantField { field, .. } => {
                    assert!(variant_fields.insert(field.field()));
                    let hir::NominalSourceShapeV1::Enum(shape) = owner.source_shape() else {
                        panic!("enum field owner")
                    };
                    assert!(
                        shape
                            .variants()
                            .iter()
                            .any(|variant| variant.fields().contains(field))
                    );
                    counts[1] += 1;
                }
                Requirement::Inheritance { parent, .. } => {
                    assert!(owner.exact_supertypes().values().contains(parent));
                    counts[2] += 1;
                }
                Requirement::Constructor { callable, .. } | Requirement::Slot { callable, .. } => {
                    assert!(std::ptr::eq(
                        callable,
                        callables.declaration(callable.declaration()).unwrap()
                    ));
                    assert_eq!(
                        callable.owner(),
                        hir::PublicDeclarationOwnerV1::Nominal(owner.declaration())
                    );
                    counts[if matches!(requirement, Requirement::Constructor { .. }) {
                        3
                    } else {
                        4
                    }] += 1;
                }
            }
            Ok(())
        })
        .unwrap();
    let closure =
        hir::NominalMaterializationClosure::from_declarations(nominals, callables).unwrap();
    for nominal in nominals.all_records() {
        if let hir::SourceNominalId::Concrete(owner) = nominal.declaration() {
            assert!(closure.contains(owner));
        }
    }
    counts
}

fn name(owner: PersistentTypeId, identities: &ValidatedIdentityGraph) -> String {
    declaration_dump::nominal(hir::SourceNominalId::Concrete(owner), identities)
}
