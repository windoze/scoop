use super::*;
use hir::{EditingPropertyAccess as Access, SemanticProperty as Target};

#[test]
fn property_reads_keep_logical_declarations_and_actual_name_ranges() {
    for text in [
        "const val property: Boolean = true\nfun editing(): Boolean = property",
        "val property: Boolean = true\nfun editing(): Boolean = property",
        "val property: Boolean get() = true\nfun editing(): Boolean = property",
        "class Value(val property: Boolean)\nfun editing(value: Value): Boolean = value.property",
        "struct Value(val property: Boolean)\nfun editing(value: Value): Boolean = value.property",
        "object Value { val property: Boolean = true }\nfun editing(): Boolean = Value.property",
        "class Value<T>(val property: T)\nfun editing(value: Value<Boolean>): Boolean = value.property",
        "class Value<T>(val item: T)\nval <T> Value<T>.property: T get() = item\nfun editing(value: Value<Boolean>): Boolean = value.property",
        "class Value(val property: Boolean) { fun editing(): Boolean = property }",
    ] {
        let mut analysis = analyze(text);
        let function = function(&analysis, "editing");
        let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
            panic!("{text}: {:?}", analysis.diagnostics());
        };
        assert_eq!(facts.properties.len(), 1, "{text}: {:?}", facts.properties);
        let reference = facts.properties[0].clone();
        assert_eq!(
            reference.span.start as usize,
            text.rfind("property").unwrap(),
            "{text}"
        );
        assert_eq!(reference.span.end - reference.span.start, 8);
        assert_eq!(
            reference.definition.as_ref().unwrap().span.start as usize,
            text.find("property").unwrap(),
            "{text}"
        );
        assert_eq!(
            reference.definition.as_ref().unwrap().span.end
                - reference.definition.as_ref().unwrap().span.start,
            8
        );
        assert_eq!(reference.access, Access::Read);
        assert!(matches!(reference.target, Target::Current(_)));
        let V::Known(ty) = analysis
            .describe_body_type(function, reference.context, reference.value_type)
            .unwrap()
        else {
            panic!("a checked property has its applied type");
        };
        assert_eq!(ty.name, "Boolean", "{text}");
    }
}

#[test]
fn property_writes_and_updates_use_actual_access_roles() {
    let text = "var property: Int = 0\nfun editing(): Int { property = 1; property += 2; property++; return property }";
    let mut analysis = analyze(text);
    let function = function(&analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("{:?}", analysis.diagnostics());
    };
    assert_eq!(
        facts
            .properties
            .iter()
            .map(|reference| reference.access)
            .collect::<Vec<_>>(),
        [
            Access::Write,
            Access::ReadWrite,
            Access::ReadWrite,
            Access::Read
        ]
    );
    let target = facts.properties[0].target;
    for reference in &facts.properties {
        assert_eq!(reference.target, target);
        assert!(reference.mutable);
        assert_eq!(
            &text[reference.span.start as usize..reference.span.end as usize],
            "property"
        );
    }
}

#[test]
fn local_shadowing_does_not_become_a_property_reference() {
    let text = "class Value(var property: Boolean)\nfun editing(value: Value): Boolean { val property = true; value.property = property; return value.property }";
    let mut analysis = analyze(text);
    let function = function(&analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("{:?}", analysis.diagnostics());
    };
    assert_eq!(facts.properties.len(), 2);
    assert_eq!(facts.properties[0].access, Access::Write);
    assert_eq!(facts.properties[1].access, Access::Read);
    assert_eq!(facts.properties[0].target, facts.properties[1].target);
    assert!(facts.locals.iter().any(
        |local| local.reference.name == "property" && local.role == hir::EditingLocalRole::Read
    ));
}

#[test]
fn compound_assign_operator_does_not_write_the_property_it_reads() {
    let text = "class Counter { operator fun plusAssign(value: Int) {} }\nval property: Counter = Counter()\nfun editing() { property += 1 }";
    let mut analysis = analyze(text);
    let function = function(&analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("{:?}", analysis.diagnostics());
    };
    assert_eq!(facts.properties.len(), 1);
    assert_eq!(facts.properties[0].access, Access::Read);
    assert!(!facts.properties[0].mutable);
}

#[test]
fn current_extension_and_direct_interface_accessors_keep_the_property_target() {
    for text in [
        "class Value<T>(var item: T)\nvar <T> Value<T>.property: T get() = item\nset(value) { item = value }\nfun editing(value: Value<Boolean>): Boolean { value.property = true; return value.property }",
        "interface Value { var property: Boolean get() = true\nset(value) {} }\nclass Concrete: Value { fun editing(): Boolean { super<Value>.property = false; return super<Value>.property } }",
    ] {
        let mut analysis = analyze(text);
        let function = function(&analysis, "editing");
        let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
            panic!("{text}: {:?}", analysis.diagnostics());
        };
        assert_eq!(facts.properties.len(), 2, "{text}: {:?}", facts.properties);
        assert_eq!(facts.properties[0].access, Access::Write);
        assert_eq!(facts.properties[1].access, Access::Read);
        assert_eq!(facts.properties[0].target, facts.properties[1].target);
        assert_eq!(
            facts.properties[0].definition.as_ref().unwrap().span.start as usize,
            text.find("property").unwrap()
        );
    }
}

#[test]
fn failed_property_access_does_not_publish_successful_body_facts() {
    for text in [
        "class Value(private val property: Boolean)\nfun editing(value: Value): Boolean = value.property",
        "val property: Boolean = true\nfun editing() { property = false }",
    ] {
        let mut analysis = analyze(text);
        let function = function(&analysis, "editing");
        assert!(
            matches!(
                analysis.function_facts(function, || false).unwrap(),
                V::Unavailable(U::BodyFailure)
            ),
            "{text}"
        );
    }
}
