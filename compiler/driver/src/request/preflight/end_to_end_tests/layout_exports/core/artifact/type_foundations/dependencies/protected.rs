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
    for nominal in public.nominal_interfaces().all_records() {
        let details = nominal.declaration_details();
        for id in details.constructors().values() {
            let source = public
                .callable_interfaces()
                .declaration(scoop_identity::CallableTemplateOrigin::Constructor(*id))
                .unwrap();
            assert_eq!(source.owner().nominal_owner(), Some(nominal.declaration()));
        }
    }
}
