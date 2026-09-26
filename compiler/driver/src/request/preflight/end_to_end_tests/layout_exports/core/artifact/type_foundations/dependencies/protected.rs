//! Complete source-only and concrete protected surfaces survive artifact bytes.

use super::*;
pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    let name = "protected-sources";
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
    checked.with_inheritance_graph(&[core], |_| ()).unwrap();
    let metadata = checked.metadata();
    let public = metadata.public;
    let mut rows = Vec::new();
    for nominal in public.nominal_interfaces().all_records() {
        let details = nominal.declaration_details();
        rows.push(format!(
            "nominal {:?} {:?} {:?} binders={} constructors={} members={} children={}\n",
            nominal.declaration(),
            nominal.kind(),
            details.declared_visibility(),
            nominal.type_parameters().len_u32(),
            details.constructors().values().len(),
            details.members().values().len(),
            details.children().values().len()
        ));
        for id in details.constructors().values() {
            let source = public
                .callable_interfaces()
                .declaration(scoop_identity::CallableTemplateOrigin::Constructor(*id))
                .unwrap();
            assert_eq!(source.owner().nominal_owner(), Some(nominal.declaration()));
        }
    }
    for callable in public.callable_interfaces().all_declarations() {
        rows.push(format!(
            "callable {:?} {:?} parameters={} binders={}\n",
            callable.declaration(),
            callable.declared_visibility(),
            callable.parameters().parameters().len(),
            callable.type_parameters().len_u32()
        ));
    }
    for property in public.property_interfaces().all_declarations() {
        rows.push(format!(
            "property {:?} {:?} {:?}\n",
            property.declaration(),
            property.declared_visibility(),
            property.representation()
        ));
    }
    rows.sort();
    let output = rows.concat();
    let snapshot = fixtures.join(format!("{name}.snap"));
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
        std::fs::write(&snapshot, &output).unwrap();
    }
    assert_eq!(output, std::fs::read_to_string(snapshot).unwrap());
}
