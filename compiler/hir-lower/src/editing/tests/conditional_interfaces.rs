use super::*;

mod imported;

#[test]
fn conditional_member_completion_uses_the_original_owner_conditions() {
    let mut analysis = analyze(
        "interface Flag { fun enabled(): Boolean }\n\
         class Holder<T> {\n\
             override interface Flag where T : ref {\n\
                 public override fun enabled(): Boolean = true\n\
             }\n\
         }\n\
         fun known(value: Holder<String>) { value. }\n\
         fun disabled(value: Holder<Boolean>) { value. }\n\
         fun <T> unknown(value: Holder<T>) { value. }",
    );
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    for (name, available) in [("known", true), ("disabled", false), ("unknown", false)] {
        let function = function(&analysis, name);
        let inputs = analysis.receivers(function, || false).unwrap();
        assert_eq!(inputs.len(), 1);
        let V::Known(members) = &inputs[0].members else {
            panic!("{name}: the recovered receiver has a known member set: {inputs:?}")
        };
        assert_eq!(
            members
                .candidates
                .iter()
                .any(|member| member.name == "enabled"),
            available,
            "{name}: {:?}",
            members.candidates
        );
    }
}

#[test]
fn invalid_interface_conditions_do_not_poison_independent_declarations() {
    let analysis = analyze(
        "class Broken<T> {\n\
             override interface Missing where T : ref {\n\
                 public override fun enabled(): Boolean = true\n\
             }\n\
         }\n\
         fun broken(value: Broken<String>) {}\n\
         fun healthy(value: Boolean): Boolean = value",
    );
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Missing"))
    );
    assert!(matches!(
        analysis.signature(function(&analysis, "healthy")),
        V::Known(_)
    ));
    let signature = analysis.signature(function(&analysis, "broken"));
    assert!(
        matches!(signature, V::Unavailable(U::SignatureFailure)),
        "{signature:?}"
    );
}
