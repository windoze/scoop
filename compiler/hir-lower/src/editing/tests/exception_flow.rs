use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn complete_try_bodies_provide_the_actual_throw_path_to_handlers() {
    let text = "fun editing(value: Any, failure: Throwable) { try { if (value !is String) return; throw failure } catch (error: Throwable) { value. } }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}

#[test]
fn unknown_try_bodies_drop_mutable_facts_but_keep_immutable_values() {
    for (text, expected) in [
        (
            "fun editing(value: Any) { var local: Any = value; if (local is String) { try { local. } catch (error: Throwable) { local. } finally { local. } } }",
            "Any",
        ),
        (
            "fun editing(value: Any) { if (value is String) { try { value. } catch (error: Throwable) { value. } finally { value. } } }",
            "String",
        ),
    ] {
        let mut analysis = analyze(text);
        let id = function(&analysis, "editing");
        let facts = analysis.receivers(id, || false).unwrap().to_vec();
        assert_eq!(facts.len(), 3);
        assert_eq!(known_type(&mut analysis, &facts[0]), "String");
        assert_eq!(known_type(&mut analysis, &facts[1]), expected, "{text}");
        assert_eq!(known_type(&mut analysis, &facts[2]), expected, "{text}");
    }
}

#[test]
fn finally_joins_complete_region_exits_and_handler_writes() {
    for (handler, expected) in [("", "String"), ("local = true", "Any")] {
        let text = format!(
            "fun editing(value: Any, flag: Boolean, failure: Throwable) {{ var local: Any = value; if (local is String) {{ try {{ if (flag) throw failure }} catch (error: Throwable) {{ {handler} }} finally {{ local. }} }} }}"
        );
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
    let mut analysis = analyze(
        "fun editing(value: Any) { try { if (value !is String) return } finally { value. } }",
    );
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "Any");
}

#[test]
fn finally_and_nested_regions_keep_the_original_loop_targets() {
    for text in [
        "fun editing(value: String) { cycle@ while (true) { try { break@cycle } finally { value. } } }",
        "fun editing(value: String) { try {} catch (error: Throwable) { try {} finally { value. } } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn finally_signatures_use_preserved_complete_flow_and_unknown_boolean_aliases_do_not_narrow() {
    let candidates = call_candidates(
        "fun choose(value: String, next: Boolean): Boolean = next\nfun choose(value: Boolean, next: Boolean): Boolean = next\nfun editing(value: Any) { var local: Any = value; if (local is String) { try {} finally { choose(local,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
    let text = "fun editing(value: Any) { var local: Any = value; val isText = local is String; try { local. } catch (error: Throwable) { if (isText) { local. } } }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(known_type(&mut analysis, &facts[1]), "Any");
}
