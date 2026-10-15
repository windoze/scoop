use super::*;

pub(super) fn check(replay: &Fixture<'_>, combined: bool) {
    assert_eq!(
        replay.fixture_bindings().count(),
        if combined { 16 } else { 4 }
    );
    let mut private = 0;
    let mut internal = 0;
    for binding in replay.fixture_bindings() {
        let source = replay
            .source
            .public
            .callable_interfaces()
            .declaration(scoop_identity::CallableTemplateOrigin::Constructor(
                declaration(binding),
            ))
            .unwrap();
        private += usize::from(source.declared_visibility() == hir::DeclaredVisibilityV1::Private);
        internal +=
            usize::from(source.declared_visibility() == hir::DeclaredVisibilityV1::Internal);
    }
    assert_eq!(
        (private, internal),
        if combined { (2, 1) } else { (0, 0) },
        "support constructors retain their original source visibility"
    );
}
