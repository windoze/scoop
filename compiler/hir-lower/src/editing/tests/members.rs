use super::*;

pub(super) fn members(analysis: &mut DeclarationAnalysis) -> hir::EditingMembers {
    let id = function(analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap();
    assert_eq!(facts.len(), 1);
    let V::Known(members) = &facts[0].members else {
        panic!("receiver members are unavailable: {facts:?}");
    };
    members.clone()
}

#[test]
fn nominal_completion_obeys_visibility_and_override_selection() {
    let text = "open class Parent { open fun member(): Boolean = true\nprivate fun hidden(): Boolean = false\nprotected fun restricted(): Boolean = true\nval property: Boolean = true }\nclass Child : Parent() { override fun member(): Boolean = false }\nfun editing(value: Child) { value. }";
    let mut analysis = analyze(text);
    let members = members(&mut analysis);
    let selected = members
        .candidates
        .iter()
        .filter(|member| member.name == "member")
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1, "{members:?}");
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "property")
    );
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| matches!(member.name.as_str(), "hidden" | "restricted"))
    );
    let hir::EditingMemberTarget::Callable(hir::SemanticCallable::Current(target)) =
        selected[0].target
    else {
        panic!("source method");
    };
    let origin = analysis
        .lowerer
        .editing_callable_origin(hir::SemanticCallable::Current(target))
        .unwrap();
    assert_eq!(origin.span.start as usize, text.rfind("member").unwrap());
}

#[test]
fn extension_candidates_use_receiver_constraints_without_missing_call_arguments() {
    let text = "fun Boolean.onlyBoolean(required: String): Boolean = this\nfun String.onlyString(): String = this\nfun <T> T.generic(required: T): T = required\nval Boolean.property: Boolean get() = this\nfun editing(value: Boolean) { value. }";
    let mut analysis = analyze(text);
    let members = members(&mut analysis);
    for name in ["onlyBoolean", "generic", "property"] {
        assert!(
            members
                .candidates
                .iter()
                .any(|member| member.name == name && member.is_extension),
            "{members:?}"
        );
    }
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| member.name == "onlyString")
    );
}

#[test]
fn incomplete_extension_candidates_remain_explicitly_unavailable() {
    let mut analysis = analyze(
        "fun Boolean.known(): Boolean = this\nfun editing(value: Boolean) { value. }\nfun Boolean.broken(value:",
    );
    let members = members(&mut analysis);
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "known")
    );
    assert!(
        members
            .unavailable
            .iter()
            .any(|member| member.name == "broken"),
        "{members:?}"
    );
}

#[test]
fn safe_navigation_checks_the_option_payload() {
    let mut analysis = analyze(
        "fun Boolean.payload(): Boolean = this\nfun editing(value: Option<Boolean>) { value?. }",
    );
    let members = members(&mut analysis);
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "payload"),
        "{members:?}"
    );
}

#[test]
fn generic_interface_receivers_keep_the_ordinary_member_restriction() {
    let mut analysis = analyze(
        "interface Contract { fun available(): Boolean\nfun <T> generic(value: T): T }\nfun editing(value: Contract) { value. }",
    );
    let members = members(&mut analysis);
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "available")
    );
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| member.name == "generic")
    );
}
