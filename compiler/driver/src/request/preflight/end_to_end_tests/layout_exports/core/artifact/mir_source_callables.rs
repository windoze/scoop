//! Representative source callables retain their selected machine partition.
use super::*;
use scoop_identity::DependencyCallableDeclarationId as Declaration;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    if !name.starts_with("shared-callables-") && name != "shared-accessors-combined" {
        return;
    }
    let fixture = Fixture { source };
    if name.starts_with("shared-callables-") {
        assert!(fixture.function("privateHelper").is_some());
    }
    if name == "shared-callables-combined" {
        let declaration =
            Declaration::Function(fixture.function("sharedCallableDeferred").unwrap());
        assert!(ordinary.export(declaration).is_some());
        assert!(
            section
                .callables()
                .get(declaration.implementation())
                .is_none()
        );
    }
    if let Some(keep) = fixture.function("keep") {
        let binding = ordinary.export(Declaration::Function(keep)).unwrap();
        assert_eq!(binding.gc_effect(), mir::GcEffect::NoGc);
        assert!(binding.signature().receiver().into_option().is_some());
        assert_eq!(binding.signature().parameters().len(), 1);
    }
}
struct Fixture<'a> {
    source: hir::CheckedSharedTypeFoundationV1<'a>,
}
impl Fixture<'_> {
    fn function(&self, name: &str) -> Option<scoop_identity::PersistentFunctionId> {
        self.source.metadata().public.callable_interfaces().all_declarations().find_map(|source| {
            let scoop_identity::CallableTemplateOrigin::Function(id) = source.declaration() else { return None };
            let key = self.source.metadata().identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(id).unwrap();
            matches!(key.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name).then_some(id)
        })
    }
}
