//! Actual parameter protocols join the complete shared source declarations.

use super::*;
use hir::{
    CanonicalProtectedCallableSourceInterfacesV1, CanonicalProtectedSourceParametersV1,
    ProtectedCallableSourceInterfaceV1, ProtectedParameterCallingV1, ProtectedSourceParameterV1,
};
use scoop_identity::{CallableTemplateOrigin, DeclarationName, SourceDeclarationKey};

mod parameters;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    let name = "source-protocols";
    let root = sysroot.join(name);
    write_manifest_cone(
        &root,
        "dev.example",
        name,
        "library",
        &std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap(),
    );
    let provider = lower(sysroot, target, &root, vec![], &[core]);
    let checked = provider.check(&[core]).unwrap();
    checked
        .with_inheritance_graph(&[core], &mut meter(), |_, _| ())
        .unwrap();
    let records = checked.section().protected_source_interfaces().records();
    let mut missing = records.to_vec();
    missing.remove(index(checked, "none"));
    assert!(matches!(
        reject(checked, core, missing),
        Error::SourceProtocolInventory(_)
    ));
    let shared = checked.metadata().public;
    let extra = shared
        .source_interfaces()
        .records()
        .iter()
        .find(|record| {
            !records
                .iter()
                .any(|source| source.owner() == record.owner())
                && record.parameters().is_empty()
                && matches!(record.owner(), CallableTemplateOrigin::Function(_))
        })
        .unwrap();
    let mut surplus = records.to_vec();
    surplus.push(
        ProtectedCallableSourceInterfaceV1::try_new(
            extra.owner(),
            CanonicalProtectedSourceParametersV1::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        reject(checked, core, surplus),
        Error::SourceProtocolInventory(_)
    ));
    parameters::check(checked, core);

    let mut output = String::new();
    for record in records {
        output.push_str(&format!(
            "{:?} parameters={}\n",
            record.owner(),
            record.parameters().len_u32()
        ));
        for (position, parameter) in record.parameters().parameters().iter().enumerate() {
            output.push_str(&format!(
                "  {position} {} {:?} {:?}\n",
                parameter.name().as_str(),
                parameter.value_type(),
                parameter.calling(),
            ));
        }
    }
    let snapshot = fixtures.join(format!("{name}.snap"));
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
        std::fs::write(&snapshot, &output).unwrap();
    }
    assert_eq!(output, std::fs::read_to_string(snapshot).unwrap());
}

fn index(checked: CheckedSharedTypeFoundationV1<'_>, name: &str) -> usize {
    checked
        .section()
        .protected_source_interfaces()
        .records()
        .iter()
        .position(|record| {
            let key = match record.owner() {
                CallableTemplateOrigin::Function(id) => checked
                    .metadata()
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap(),
                _ => return false,
            };
            matches!(key.name(), DeclarationName::Named(actual) if actual.as_str() == name)
        })
        .unwrap()
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<ProtectedCallableSourceInterfaceV1>,
) -> Error {
    let source = checked.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        source.inheritance().clone(),
        source.protected_declarations().clone(),
        CanonicalProtectedCallableSourceInterfacesV1::try_new(records).unwrap(),
        source.protected_defaults().clone(),
        source.definition_sources().clone(),
        source.selected().clone(),
    );
    candidate
        .validate_shared_foundation(checked.metadata(), &[core], &mut meter())
        .unwrap()
        .with_inheritance_graph(&[core], &mut meter(), |_, _| ())
        .expect_err("source parameters must match the complete shared declarations")
}
