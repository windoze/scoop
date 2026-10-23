use super::imports::{analyze_files, at};
use super::*;
use hir::EditingReferenceTarget as Target;

mod failures;
mod imported;

const FILE: &str = "src/constants.scoop";

fn property(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    name: &str,
) -> hir::EditingPropertyDeclaration {
    let origin = at(FILE, text, needle, name.len());
    match analysis
        .property_declaration(&origin.source, origin.span, || false)
        .unwrap()
    {
        V::Known(Some(property)) => property,
        value => panic!("{needle}: {value:?}; {:?}", analysis.diagnostics()),
    }
}

fn alias(index: &hir::EditingReferenceIndex, name: &str) -> Target {
    Target::ImportAlias(
        index
            .aliases
            .iter()
            .find(|(_, alias)| alias.name == name)
            .unwrap()
            .0,
    )
}

#[test]
fn forward_constants_retain_each_definition_site_without_rechecking_bodies() {
    let text = "const val first: Boolean = later\nconst val later: Boolean = leaf\nconst val leaf: Boolean = true\nval image: Boolean = first && !later\nfun unrelated(): Boolean = missing";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert_eq!(analysis.constant_references.len(), 4);
    let declaration = property(&mut analysis, text, "later:", "later");
    for needle in ["later\n", "later\nfun"] {
        let reference = property(&mut analysis, text, needle, "later");
        assert_eq!(reference.property, declaration.property);
        assert_eq!(reference.origin, Some(at(FILE, text, "later:", 5)));
        assert_eq!(
            analysis
                .describe_declaration_type(&reference.binders, reference.value_type)
                .unwrap()
                .name,
            "Boolean"
        );
    }
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.lowerer.pending_runtime_initializers.is_empty());
    assert!(
        analysis
            .source_bodies()
            .iter()
            .all(|body| body.callable().is_some())
    );
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at(FILE, text, "later:", 5))[0];
    assert!(matches!(target, Target::Property(_)));
    assert_eq!(index.references_to(target).count(), 3);
    assert_eq!(index.targets_at(&at(FILE, text, "later\n", 5)), &[target]);
    let leaf = index.targets_at(&at(FILE, text, "leaf:", 4))[0];
    assert_eq!(index.references_to(leaf).count(), 2);
}

#[test]
fn exact_aliases_and_singleton_member_shadowing_keep_the_selected_lookup_layer() {
    let provider = "package constapi\nconst val flag: Boolean = true\nobject Values { const val flag: Boolean = false; const val copy = flag }";
    let text = "import constapi.flag as selected\nimport constapi.Values.flag as member\nimport constapi.Values\nconst val one = selected\nval image: Boolean = member\nconst val qualified = Values.flag\nobject Shadow { const val selected = false; const val local = selected }";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), (FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let original = property(&mut analysis, text, "selected\nval", "selected");
    let member = property(&mut analysis, text, "member\nconst", "member");
    assert_ne!(original.property, member.property);
    assert_eq!(
        member.property,
        property(&mut analysis, text, "flag\nobject", "flag").property
    );
    let shadowed = property(&mut analysis, text, "selected }", "selected");
    assert_ne!(shadowed.property, original.property);
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.references_to(alias(index, "selected")).count(), 2);
    assert_eq!(index.references_to(alias(index, "member")).count(), 2);
    let shadowed = index.targets_at(&at(FILE, text, "selected = false", 8))[0];
    assert_eq!(
        index.targets_at(&at(FILE, text, "selected }", 8)),
        &[shadowed]
    );
    let nested = index.targets_at(&at("src/api.scoop", provider, "flag: Boolean = false", 4))[0];
    assert!(
        index
            .references_to(nested)
            .any(|reference| { reference.origin == at("src/api.scoop", provider, "flag }", 4) })
    );
}

#[test]
fn failed_static_trials_leave_only_the_actual_runtime_reference() {
    let text = "const val flag: Boolean = true\nfun choose(): Boolean = true\nval runtime: Boolean = flag && choose()\nval image: Boolean = flag\nval wrong: String = flag";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert_eq!(analysis.constant_references.len(), 1);
    assert_eq!(
        analysis.constant_references[0].origin,
        at(FILE, text, "flag\nval wrong", 4)
    );
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at(FILE, text, "flag:", 4))[0];
    assert_eq!(index.references_to(target).count(), 3);
    assert_eq!(index.targets_at(&at(FILE, text, "flag &&", 4)), &[target]);
    assert!(
        index
            .targets_at(&at(FILE, text, "flag", 4))
            .contains(&target)
    );
    assert!(!index.unavailable.is_empty());
}

#[test]
fn numeric_intrinsic_operands_retain_their_constant_references() {
    let text = "const val number: Int = 3\nconst val distance: Long = 1L\nconst val shifted: Int = number shl distance\nval image: Int = number.shl(distance)";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert_eq!(analysis.constant_references.len(), 4);
    let original = property(&mut analysis, text, "number:", "number");
    assert_eq!(
        property(&mut analysis, text, "number shl", "number").property,
        original.property
    );
    assert_eq!(
        property(&mut analysis, text, "number.shl", "number").property,
        original.property
    );
    let index = analysis.reference_index(|| false).unwrap();
    for name in ["number", "distance"] {
        let target = index.targets_at(&at(FILE, text, name, name.len()))[0];
        assert_eq!(index.references_to(target).count(), 3);
    }
    assert!(analysis.body_facts.is_empty());
}
