//! A structural value crosses an ordinary Any signature as an actual ODR box.

use super::*;
use crate::request::preflight::end_to_end_tests::imported_classes::runtime;
use scoop_identity::{ExactTypeKey, LinkageClass};

pub(super) fn check(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core: &SingleConeProductionSuccess,
) {
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-any-call-signatures");
    let source = std::fs::read_to_string(fixtures.join("structural-box.scoop")).unwrap();
    let root = sysroot.join("structural-box");
    write_manifest_cone(&root, "dev.example", "structural-box", "library", &source);
    let mut outputs = Vec::new();
    for (kind, stage) in [
        (StageDumpKind::Hir, "hir"),
        (StageDumpKind::Mir, "mir"),
        (StageDumpKind::Lir, "lir"),
    ] {
        let mut request = build_manifest_request(
            sysroot,
            target,
            &root,
            &root.join("output.slib"),
            vec![],
            vec![],
        );
        request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
        let output = request.build_and_publish().unwrap();
        let dump = output.emitted_dumps().first().unwrap().text();
        let path = fixtures.join(format!("structural-box.{stage}.snap"));
        if std::env::var_os("SCOOP_UPDATE_ANY_CALLS").is_some() {
            std::fs::write(&path, dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(path).unwrap());
        outputs.push(output);
    }
    std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
    let output = outputs.last().unwrap();
    let library = runtime::build(target, &root.join("runtime"));
    let runtime_fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");
    let closure = runtime::check(
        target,
        &[core, output],
        &library,
        &runtime_fixtures,
        &root.join("run"),
        "structural-box",
    );
    let identity = output.artifact().summary().coordinate().identity().unwrap();
    let (sections, _) = closure.artifact(identity).unwrap();
    let boxed = sections
        .mir_type_bridge()
        .exports()
        .types()
        .records()
        .iter()
        .find(|record| {
            let mir::MirTypeRepresentationV1::BoxedValue { payload } = record.representation()
            else {
                return false;
            };
            matches!(
                sections
                    .identity_graph()
                    .canonical_key::<_, ExactTypeKey>(payload.value)
                    .unwrap()
                    .as_ref(),
                ExactTypeKey::Tuple(_)
            )
        })
        .expect("the Any argument has an actual tuple box");
    let descriptor = sections
        .lir_exports()
        .descriptors()
        .get(boxed.exact())
        .unwrap();
    assert_eq!(
        descriptor.physical_definition().symbol().linkage(),
        LinkageClass::OdrWeak
    );
}
