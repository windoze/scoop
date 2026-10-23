use super::*;

#[test]
fn invalid_cycles_leave_independent_functions_and_constructors_available() {
    for declaration in [
        "interface Cycle : Cycle {}",
        "interface Cycle : Other {}\ninterface Other : Cycle {}",
        "open class Cycle : Cycle() {}",
        "open class Cycle : Other() {}\nopen class Other : Cycle() {}",
        "struct Cycle(val next: Cycle)",
        "struct Cycle<T>(val next: Cycle<(T, T)>)",
        "enum Cycle { Item(val next: Cycle) }",
    ] {
        let text = format!(
            "{declaration}\nstruct Independent(val flag: Boolean)\n{HEALTHY}\nfun construct(): Boolean = Independent(true).flag"
        );
        let mut analysis = analyze_files(&[(PATH, &text)]);
        assert!(
            available(&mut analysis, "independent"),
            "{declaration}: {:?}",
            analysis.diagnostics()
        );
        assert!(
            available(&mut analysis, "construct"),
            "{:?}",
            analysis.diagnostics()
        );
        assert_unavailable(&mut analysis, &text, "Cycle");
        let constructors = analysis.lowerer.editing_source_constructors();
        assert_eq!(constructors.len(), 1, "{declaration}");
        let signature = analysis
            .constructor_signature(constructors[0].0)
            .unwrap()
            .unwrap();
        assert_eq!(
            analysis
                .describe_declaration_type(&signature.binders, signature.result)
                .unwrap()
                .name,
            "Independent"
        );
    }
}

#[test]
fn cyclic_dependencies_propagate_to_earlier_owners_and_aliases() {
    let text = format!(
        "typealias Alias = Dependent\ninterface Dependent : Cycle {{}}\ninterface Cycle : Cycle {{}}\n{HEALTHY}"
    );
    let mut analysis = analyze_files(&[(PATH, &text)]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    for name in ["Alias", "Dependent", "Cycle"] {
        assert_unavailable(&mut analysis, &text, name);
    }
    let text = format!(
        "typealias Alias = Dependent\nstruct Dependent(val cycle: Cycle)\nstruct Cycle(val value: Cycle)\n{HEALTHY}"
    );
    let mut analysis = analyze_files(&[(PATH, &text)]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    for name in ["Alias", "Dependent", "Cycle"] {
        assert_unavailable(&mut analysis, &text, name);
    }
}

#[test]
fn valid_f_bounds_and_reference_recursion_remain_usable() {
    let text = "interface RecursiveConstraint<T: RecursiveConstraint<T>> {}\nclass Item : RecursiveConstraint<Item> {}\nclass Node(val next: Node?)\nstruct Value(val node: Node)\nfun independent(value: Value): Node = value.node\nfun bounded(value: Item): RecursiveConstraint<Item> = value";
    let mut analysis = analyze_files(&[(PATH, text)]);
    for name in ["independent", "bounded"] {
        assert!(
            available(&mut analysis, name),
            "{:?}",
            analysis.diagnostics()
        );
    }
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert!(
        analysis
            .lowerer
            .failed_editing_declarations()
            .next()
            .is_none()
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty());
}
