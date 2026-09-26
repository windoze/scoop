//! Actual source defaults use the shared typed body and declaration references.
use super::*;
use std::fmt::Write;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    for case in ["source-defaults", "default-combinations"] {
        let root = sysroot.join(case);
        write_manifest_cone(
            &root,
            "dev.example",
            case,
            "library",
            &std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap(),
        );
        let provider = lower(sysroot, target, &root, vec![], &[core]);
        let checked = provider.check(&[core]).unwrap();
        checked.with_inheritance_graph(&[core], |_| ()).unwrap();
        let templates = checked.metadata().public.default_templates().records();
        assert!(!templates.is_empty());
        let mut snapshot = String::new();
        let mut inherited = 0;
        for template in templates {
            assert_eq!(template.body().value().result_type(), template.result());
            if template.definition_root().declaration() != template.key().owner() {
                inherited += 1;
            }
            let references = template.references();
            writeln!(snapshot, "default {:?} from {:?}\n  callables={} constructors={} types={} globals={} singletons={} fields={}",
                template.key(), template.definition_root().declaration(), references.callables().len(), references.constructors().len(),
                references.types().len(), references.globals().len(), references.singleton_values().len(), references.fields().len()).unwrap();
        }
        assert_eq!(inherited, 1, "Derived.count retains its base default body");
        let path = fixtures.join(format!("{case}.snap"));
        if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
            std::fs::write(&path, &snapshot).unwrap();
        }
        assert_eq!(snapshot, std::fs::read_to_string(path).unwrap());
    }
}
