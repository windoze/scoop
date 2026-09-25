//! Actual operations, rather than nominal fanout, choose external shape owners.

use super::*;
use hir::{CanonicalSelectedExternalTypeUsesV1, SelectedExternalTypeUseV1, SelectedTypeUseV1};

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
) {
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-executable-type-sites");
    for case in ["shape-standalone", "shape-combined"] {
        let root = sysroot.join(case);
        write_manifest_cone(
            &root,
            "dev.example",
            case,
            "library",
            &std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap(),
        );
        let artifact = lower(sysroot, target, &root, vec![], &[core]);
        let current = artifact.check(&[core]).unwrap();
        let original = current.section().selected().records();
        let mut shapes = original
            .iter()
            .filter_map(|record| match record.usage() {
                SelectedTypeUseV1::ShapeSupport { exact } => Some((record.provider(), exact)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let names: &[&str] = if case == "shape-standalone" {
            &["Int"]
        } else {
            &["Int", "String", "Unit"]
        };
        let mut expected = names
            .iter()
            .map(|name| (core.provider(), exact_named(core, name)))
            .collect::<Vec<_>>();
        shapes.sort_unstable();
        expected.sort_unstable();
        assert_eq!(shapes, expected);
        super::super::type_uses::check(current, &[core]);
        for (provider, exact) in [
            (current.provider(), exact_named(core, "Int")),
            (core.provider(), exact_named(core, "UInt16")),
        ] {
            let position = original
                .iter()
                .position(|record| matches!(record.usage(), SelectedTypeUseV1::ShapeSupport { .. }))
                .unwrap();
            let mut changed = original.to_vec();
            changed[position] =
                SelectedExternalTypeUseV1::new(provider, SelectedTypeUseV1::ShapeSupport { exact });
            reject(current, core, changed);
        }
        let mut extra = original.to_vec();
        extra.push(SelectedExternalTypeUseV1::new(
            core.provider(),
            SelectedTypeUseV1::ShapeSupport {
                exact: exact_named(core, "UInt"),
            },
        ));
        reject(current, core, extra);
        if case == "shape-combined" {
            let mut local = original.to_vec();
            local.push(SelectedExternalTypeUseV1::new(
                current.provider(),
                SelectedTypeUseV1::ShapeSupport {
                    exact: exact_named(current, "LocalValue"),
                },
            ));
            reject(current, core, local);
            let tuple = current
                .metadata()
                .public
                .external_references()
                .records()
                .iter()
                .flat_map(|reference| reference.type_sites().records())
                .filter_map(|site| site.as_expression())
                .find(|site| {
                    site.role() == hir::HirExpressionTypeRoleV1::BoxedValue
                        && matches!(
                            current
                                .metadata()
                                .identities
                                .canonical_key::<_, ExactTypeKey>(site.exact())
                                .unwrap()
                                .as_ref(),
                            ExactTypeKey::Tuple(_)
                        )
                })
                .unwrap()
                .exact();
            let mut structural = original.to_vec();
            structural.push(SelectedExternalTypeUseV1::new(
                core.provider(),
                SelectedTypeUseV1::ShapeSupport { exact: tuple },
            ));
            reject(current, core, structural);
        }
        budget(current, core);
        let dump = original
            .iter()
            .map(|record| format!("{} {:?}\n", record.provider(), record.usage()))
            .collect::<String>();
        let path = fixtures.join(format!("{case}.selected.snap"));
        if std::env::var_os("SCOOP_UPDATE_EXECUTABLE_TYPE_SITES").is_some() {
            std::fs::write(&path, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(path).unwrap());
    }
}

fn exact_named(checked: CheckedSharedTypeFoundationV1<'_>, name: &str) -> PersistentExactTypeId {
    for fact in checked.facts().records() {
        let identities = checked.metadata().identities;
        let key = identities
            .canonical_key::<_, ExactTypeKey>(fact.exact())
            .unwrap();
        let ExactTypeKey::Nominal(owner) = key.as_ref() else {
            continue;
        };
        let source = identities
            .canonical_key::<_, SourceDeclarationKey>(*owner)
            .unwrap();
        if matches!(source.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name)
        {
            return fact.exact();
        }
    }
    panic!("missing fixture type {name}")
}

fn reject(
    current: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<SelectedExternalTypeUseV1>,
) {
    assert!(matches!(
        current.metadata().validate_materialized_type_uses(
            &CanonicalSelectedExternalTypeUsesV1::try_new(records).unwrap(),
            &[core.metadata()],
            &mut meter(),
        ),
        Err(Error::TypeUseInventory)
    ));
}

fn budget(current: CheckedSharedTypeFoundationV1<'_>, core: CheckedSharedTypeFoundationV1<'_>) {
    let mut measured = meter();
    current
        .validate_materialized_type_uses(&[core], &mut measured)
        .unwrap();
    let mut bounded = scoop_wire::BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    current
        .validate_materialized_type_uses(&[core], &mut bounded)
        .unwrap();
    assert!(matches!(
        current.validate_materialized_type_uses(&[core], &mut bounded),
        Err(Error::Resource(_))
    ));
}
