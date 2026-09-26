//! Actual parameter protocols join the complete shared source declarations.

use super::*;
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
    checked.with_inheritance_graph(&[core], |_| ()).unwrap();
    let records = checked.metadata().public.source_interfaces().records();
    let mut output = String::new();
    for record in records {
        output.push_str(&format!(
            "{:?} parameters={}\n",
            record.owner(),
            record.parameters().parameters().len()
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
