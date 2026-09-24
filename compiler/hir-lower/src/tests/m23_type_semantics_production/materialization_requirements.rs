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
            let nominals: hir::DecodedCanonicalNominalInterfacesV1 = decode_canonical(
                &encode(public.nominal_interfaces()).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            let nominals = nominals.resolve(&mut identities).unwrap();
            let callables: hir::DecodedCanonicalCallableInterfacesV1 = decode_canonical(
                &encode(public.callable_interfaces()).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            let callables = callables.resolve(&mut identities).unwrap();
            assert_eq!(&nominals, public.nominal_interfaces());
            assert_eq!(&callables, public.callable_interfaces());
            let actual = render(&nominals, &callables, &identities);
            assert_eq!(
                actual,
                render(
                    public.nominal_interfaces(),
                    public.callable_interfaces(),
                    &identities
                )
            );
            let snapshot = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/m23-nominal-requirements/{case}.snap"
            ));
            if std::env::var_os("SCOOP_UPDATE_NOMINAL_REQUIREMENT_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, &actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
        });
    }
}

fn render(
    nominals: &hir::CanonicalNominalInterfacesV1,
    callables: &hir::CanonicalCallableInterfacesV1,
    identities: &ValidatedIdentityGraph,
) -> String {
    let mut rows = Vec::new();
    let mut fields = BTreeSet::new();
    let mut variant_fields = BTreeSet::new();
    nominals
        .visit_materialization_requirements::<Error>(callables, &mut meter(), |requirement, _| {
            let owner = name(requirement.owner(), identities);
            let detail = match requirement {
                Requirement::Field { field, .. } => {
                    assert!(fields.insert(field.field()));
                    format!(
                        "field {}",
                        declaration_dump::ty(field.value_type(), identities)
                    )
                }
                Requirement::EnumVariantField { field, .. } => {
                    assert!(variant_fields.insert(field.field()));
                    format!(
                        "enum-field {}",
                        declaration_dump::ty(field.value_type(), identities)
                    )
                }
                Requirement::Inheritance { parent, .. } => {
                    format!("inherits {}", declaration_dump::ty(parent, identities))
                }
                Requirement::Constructor { callable, .. } | Requirement::Slot { callable, .. } => {
                    assert!(std::ptr::eq(
                        callable,
                        callables.declaration(callable.declaration()).unwrap()
                    ));
                    let kind = if matches!(requirement, Requirement::Constructor { .. }) {
                        "constructor"
                    } else {
                        "slot"
                    };
                    let parameters = callable
                        .parameters()
                        .parameters()
                        .iter()
                        .map(|parameter| declaration_dump::ty(parameter.value_type(), identities))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "{kind} {:?} ({parameters}) -> {} {:?} {:?}",
                        callable.declared_visibility(),
                        declaration_dump::ty(callable.result(), identities),
                        callable.effects().execution(),
                        callable.effects().implementation()
                    )
                }
            };
            rows.push(format!("{owner} {detail}\n"));
            Ok(())
        })
        .unwrap();
    let closure =
        hir::NominalMaterializationClosure::from_declarations(nominals, callables, &mut meter())
            .unwrap();
    for nominal in nominals.all_records() {
        if let hir::SourceNominalId::Concrete(owner) = nominal.declaration() {
            rows.push(format!(
                "{} materializable={}\n",
                name(owner, identities),
                closure.contains(owner)
            ));
        }
    }
    rows.sort();
    rows.concat()
}

fn name(owner: PersistentTypeId, identities: &ValidatedIdentityGraph) -> String {
    declaration_dump::nominal(hir::SourceNominalId::Concrete(owner), identities)
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
