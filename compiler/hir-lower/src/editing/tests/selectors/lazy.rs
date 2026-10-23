use super::*;
use std::{cell::Cell, rc::Rc};

#[test]
fn references_leave_members_lazy_and_each_selector_caches_only_completed_queries() {
    let mut analysis = analyze(
        "fun String.onlyString(): String = this\nfun Boolean.onlyBoolean(): Boolean = this\nfun editing(text: String, flag: Boolean) { text.onlyString(); flag.onlyBoolean() }",
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let inputs = selectors(&mut analysis);
    assert_eq!(inputs.len(), 2);
    let function = function(&analysis, "editing");
    for (input, expected) in inputs.iter().zip(["onlyString", "onlyBoolean"]) {
        let before = analysis.lowerer.types.len();
        let checks = Rc::new(Cell::new(0));
        let cancelled = checks.clone();
        assert!(matches!(
            analysis.selector_members(function, input.selector, move || {
                cancelled.set(cancelled.get() + 1);
                cancelled.get() > 1
            }),
            Err(EditingCancelled)
        ));
        assert_eq!(analysis.lowerer.types.len(), before);
        let candidates = members(&mut analysis, input);
        assert!(
            candidates
                .candidates
                .iter()
                .any(|member| member.name == expected)
        );
        let checks = Rc::new(Cell::new(0));
        let cancelled = checks.clone();
        assert!(matches!(
            analysis.selector_members(function, input.selector, move || {
                cancelled.set(cancelled.get() + 1);
                cancelled.get() > 1
            }),
            Ok(V::Known(Some(_)))
        ));
        assert_eq!(checks.get(), 1, "cached members do not repeat enumeration");
    }
}

#[test]
fn deferred_members_keep_generic_owner_visibility_after_other_bodies_run() {
    let mut analysis = analyze(
        "class Box<T>(val value: T) { private fun hidden(): T = value; fun inside(): T = this.hidden() }\nfun <T> editing(box: Box<T>): T = box.value",
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let inputs = selectors(&mut analysis);
    let candidates = members(&mut analysis, &inputs[0]);
    assert!(
        candidates
            .candidates
            .iter()
            .any(|member| member.name == "value")
    );
    assert!(
        !candidates
            .candidates
            .iter()
            .any(|member| member.name == "hidden")
    );
    let inside = function(&analysis, "inside");
    let V::Known(inputs) = analysis.selectors(inside, || false).unwrap() else {
        panic!("known member body")
    };
    let selector = inputs[0].selector;
    let V::Known(Some(candidates)) = analysis
        .selector_members(inside, selector, || false)
        .unwrap()
    else {
        panic!("known private receiver")
    };
    assert!(
        candidates
            .candidates
            .iter()
            .any(|member| member.name == "hidden")
    );
}

#[test]
fn deferred_safe_navigation_uses_the_original_option_inner_type() {
    let mut analysis = analyze(
        "fun Boolean.payload(): Boolean = this\nfun editing(value: Option<Boolean>) { value?.payload() }",
    );
    let inputs = selectors(&mut analysis);
    assert_eq!(inputs.len(), 1);
    let candidates = members(&mut analysis, &inputs[0]);
    assert!(
        candidates
            .candidates
            .iter()
            .any(|member| member.name == "payload")
    );
    let function = function(&analysis, "editing");
    assert!(matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Known(_)
    ));
}
