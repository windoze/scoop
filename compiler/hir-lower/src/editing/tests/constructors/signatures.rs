use super::*;
use hir::{EditingConstructorDeclaration as Declaration, EditingConstructorForm as Form};

fn signatures(analysis: &mut DeclarationAnalysis) -> Vec<hir::EditingConstructorSignature> {
    analysis
        .lowerer
        .editing_source_constructors()
        .into_iter()
        .filter_map(|(callee, _)| analysis.constructor_signature(callee).unwrap())
        .collect()
}

#[test]
fn constructor_signatures_keep_unchecked_defaults_and_declared_vararg_types() {
    let text = "class Choice<T>(value: T, other: T = missing, vararg flags: Boolean = missing)\nclass Secondary { constructor(enabled: Boolean = missing) {} }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    let signatures = signatures(&mut analysis);
    let primary = signatures
        .iter()
        .find(|signature| signature.parameters.len() == 3)
        .unwrap();
    assert_eq!(primary.binders.owner.len(), 1);
    assert_eq!(
        analysis
            .describe_declaration_type(&primary.binders, primary.result)
            .unwrap()
            .name,
        "Choice<T>"
    );
    assert_eq!(
        analysis
            .describe_declaration_type(&primary.binders, primary.parameters[0].value_type)
            .unwrap()
            .name,
        "T"
    );
    let element = primary.parameters[2].vararg_element_type.unwrap();
    assert_eq!(
        analysis
            .describe_declaration_type(&primary.binders, element)
            .unwrap()
            .name,
        "Boolean"
    );
    assert!(primary.parameters[0].declared_default.is_none());
    for parameter in &primary.parameters[1..] {
        let origin = parameter.declared_default.as_ref().unwrap();
        assert_eq!(origin.source, primary.origin.source);
        assert_eq!(
            &text[origin.span.start as usize..origin.span.end as usize],
            "missing"
        );
    }
    let secondary = signatures
        .iter()
        .find(|signature| signature.parameters.len() == 1)
        .unwrap();
    assert!(secondary.parameters[0].declared_default.is_some());
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
}

#[test]
fn constructor_and_parameter_declarations_select_their_original_typed_owner() {
    let text = "class Choice<T>(value: T)\nclass Secondary { constructor(enabled: Boolean) {} }\nstruct Pair(val item: Boolean)";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    for signature in signatures(&mut analysis) {
        let origin = &signature.origin;
        let V::Known(Some(Declaration::Constructor(callee))) = analysis
            .constructor_declaration_at(&origin.source, origin.span, || false)
            .unwrap()
        else {
            panic!("a constructor declaration retains its original range")
        };
        assert_eq!(callee, signature.callee);
        for (index, parameter) in signature.parameters.iter().enumerate() {
            let origin = &parameter.origin;
            let V::Known(Some(Declaration::Parameter {
                callee,
                index: selected,
            })) = analysis
                .constructor_declaration_at(&origin.source, origin.span, || false)
                .unwrap()
            else {
                panic!("a source parameter retains its constructor owner")
            };
            assert_eq!((callee, selected), (signature.callee, index));
        }
    }
}

#[test]
fn variant_signatures_keep_their_four_source_calling_forms() {
    let text = "enum Event<T> { Empty, Tuple(T), Named { item: T }, Item(val value: T, val enabled: Boolean = true) }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let signatures = signatures(&mut analysis);
    assert_eq!(signatures.len(), 4);
    for (name, style, count) in [
        ("Empty", hir::VariantStyle::Unit, 0),
        ("Tuple", hir::VariantStyle::Positional, 1),
        ("Named", hir::VariantStyle::Named, 1),
        ("Item", hir::VariantStyle::Constructor, 2),
    ] {
        let signature = signatures.iter().find(|signature| matches!(&signature.form, Form::Variant { name: candidate, .. } if candidate == name)).unwrap();
        assert!(matches!(&signature.form, Form::Variant { style: actual, .. } if *actual == style));
        assert_eq!(signature.parameters.len(), count);
        assert_eq!(
            analysis
                .describe_declaration_type(&signature.binders, signature.result)
                .unwrap()
                .name,
            "Event<T>"
        );
    }
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
}

#[test]
fn failed_local_regions_and_cancellation_do_not_expose_constructor_signatures() {
    let text = "fun editing() { class Local(value: Boolean); val broken = missing }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    let parameter = at("src/constructors.scoop", text, "value: Boolean", 5);
    assert!(
        analysis
            .constructor_declaration_at(&parameter.source, parameter.span, || true)
            .is_err()
    );
    assert!(matches!(
        analysis
            .constructor_declaration_at(&parameter.source, parameter.span, || false)
            .unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    assert!(signatures(&mut analysis).is_empty());
}
