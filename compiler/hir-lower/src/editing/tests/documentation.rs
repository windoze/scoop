use super::*;
use hir::{DocumentationLink as Link, DocumentationSymbol as Symbol};

fn documented(
    text: &str,
    is_cancelled: impl Fn() -> bool,
) -> Result<DeclarationAnalysis, EditingAnalysisError> {
    let core = crate::tests::m23_ordinary_core_only::support::trusted_core();
    let identity = crate::tests::test_source_identity("src/docs.scoop");
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let comments = parsed.ast().map_or_else(
        || {
            parsed
                .declaration_data()
                .source
                .as_ref()
                .unwrap()
                .documentation
                .as_slice()
        },
        |source| source.documentation.as_slice(),
    );
    let documentation = scoop_doc::prepare_comments(comments, text);
    let mut input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    input.documentation = Some(&documentation);
    let world = core.world(identity.cone());
    let protocols = core
        .foundation
        .import_core_inputs(&core.interface)
        .unwrap()
        .into();
    analyze_declarations(&[input], &protocols, &world, &[], is_cancelled)
}

#[test]
fn property_declarations_keep_their_own_comment_beside_constructor_parameters() {
    let text = "/// @param value The constructor input.\nclass Cell(\n/// The stored property.\nval value: Boolean)\n/// Top-level state.\nval state: Boolean = true\nfun broken(): Boolean = missing";
    let mut analysis = documented(text, || false).unwrap();
    let identity = crate::tests::test_source_identity("src/docs.scoop");
    for (needle, name, expected) in [
        ("value: Boolean", "value", "The stored property."),
        ("state:", "state", "Top-level state."),
    ] {
        let start = text.find(needle).unwrap() as u32;
        let V::Known(Some(declaration)) = analysis
            .property_declaration(
                &identity,
                ast::Span::new(start, start + name.len() as u32),
                || false,
            )
            .unwrap()
        else {
            panic!("complete property declaration")
        };
        let hir::SemanticProperty::Current(property) = declaration.property else {
            panic!("source property")
        };
        let symbol = Symbol::Current(hir::SourceAnnotationTarget::Property(property).into());
        let docs = analysis.documentation().unwrap();
        let rendered = scoop_doc::hover_markdown(docs.get(symbol).unwrap(), |_| None);
        assert_eq!(rendered, expected);
    }
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn variant_arguments_read_parameter_tags_without_borrowing_payload_comments() {
    let text = "enum Event {\n/// @param value The constructor argument.\nData(/** The stored payload. */ val value: Boolean)\n}\nfun editing() { val event = Event.Data(value = true) }";
    let mut analysis = documented(text, || false).unwrap();
    let function = function(&analysis, "editing");
    let start = text.find("value = true").unwrap() as u32;
    let V::Known(Some(argument)) = analysis
        .named_argument(function, ast::Span::new(start, start + 5), || false)
        .unwrap()
    else {
        panic!("complete variant argument")
    };
    let Some(hir::EditingCallee::CurrentVariant(variant)) = argument.callee else {
        panic!("the actual source variant")
    };
    let field = hir::EnumVariantFieldRef::checked(&analysis.lowerer.enums, variant, 0).unwrap();
    let docs = analysis.documentation().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    for (target, expected) in [
        (
            hir::SourceAnnotationTarget::Parameter {
                callable: hir::SourceAnnotationCallable::VariantConstructor(variant),
                index: 0,
            },
            "The constructor argument.",
        ),
        (
            hir::SourceAnnotationTarget::VariantField(field),
            "The stored payload.",
        ),
    ] {
        assert_eq!(
            scoop_doc::hover_markdown(docs.get(Symbol::Current(target.into())).unwrap(), |_| None),
            expected
        );
    }
}

#[test]
fn comments_bind_in_the_declaration_scope_without_checking_bodies() {
    let text = "/// Returns [value]; see [later].\n/// @param value The input.\nfun pass(value: Boolean): Boolean = value\nfun later(): Boolean = missing\nfun broken(value: ) {}";
    let analysis = documented(text, || false).unwrap();
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.reference_index.is_none());
    let docs = analysis.documentation().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let function = function(&analysis, "pass");
    let target = hir::DocumentationTarget::from(hir::SourceAnnotationTarget::Callable(
        hir::SourceAnnotationCallable::Function(function),
    ));
    let comment = docs
        .declarations
        .iter()
        .find(|entry| entry.target == target)
        .unwrap()
        .comment
        .as_ref()
        .unwrap();
    assert_eq!(comment.body.references.len(), 2);
    let Link::Symbols(symbols) = &comment.body.references[0].resolution else {
        panic!("the real parameter must bind")
    };
    let parameter = *symbols.first();
    assert_eq!(
        parameter,
        Symbol::Current(
            hir::SourceAnnotationTarget::Parameter {
                callable: hir::SourceAnnotationCallable::Function(function),
                index: 0,
            }
            .into()
        )
    );
    let hir::DocumentationContent::Markdown(description) = docs.get(parameter).unwrap() else {
        panic!("the parameter uses its bound @param description")
    };
    assert_eq!(description.text.text, "The input.");
    let Link::Symbols(symbols) = &comment.body.references[1].resolution else {
        panic!("body errors do not hide a declaration")
    };
    assert_eq!(
        &text[docs.source_locations[symbols.first()].span.start as usize..][..9],
        "fun later"
    );
}

#[test]
fn direct_parameter_comments_and_unresolved_links_follow_m60_rules() {
    let text = "/// See [Missing] and `[not_a_reference]`.\n/// @param value Tagged description.\nfun pass(\n/// Direct description.\nvalue: Boolean): Boolean = value";
    let analysis = documented(text, || false).unwrap();
    let docs = analysis.documentation().unwrap();
    assert!(
        docs.warnings
            .iter()
            .any(|warning| warning.message.contains("Missing"))
    );
    assert!(
        docs.warnings
            .iter()
            .any(|warning| warning.message.contains("duplicate documentation"))
    );
    let direct = docs
        .declarations
        .iter()
        .find_map(|entry| {
            let hir::DocumentationTarget::Declaration(hir::SourceAnnotationTarget::Parameter {
                ..
            }) = entry.target
            else {
                return None;
            };
            entry.comment.as_ref()
        })
        .unwrap();
    assert_eq!(direct.body.text.text, "Direct description.");
    let caller = docs
        .declarations
        .iter()
        .filter_map(|entry| entry.comment.as_ref())
        .find(|comment| !comment.body.references.is_empty())
        .unwrap();
    assert_eq!(caller.body.references.len(), 1);
    assert!(matches!(
        caller.body.references[0].resolution,
        Link::Unresolved
    ));
    assert!(caller.tags.is_empty());
    let rendered = scoop_doc::hover_markdown(hir::DocumentationContent::Comment(caller), |_| None);
    assert_eq!(rendered, "See Missing and `[not_a_reference]`.");
}

#[test]
fn hover_markdown_preserves_markup_and_does_not_choose_an_overload() {
    let text = "/// See [**choice**][pick] and [`value`]. Visit [guide][site].\n///\n/// [site]: https://example.org/guide\nfun documented(value: Boolean): Boolean = value\nfun pick(value: Boolean): Boolean = value\nfun pick(value: String): String = value";
    let analysis = documented(text, || false).unwrap();
    let docs = analysis.documentation().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let comment = docs
        .declarations
        .iter()
        .find_map(|entry| entry.comment.as_ref())
        .unwrap();
    let mut resolved = Vec::new();
    let rendered =
        scoop_doc::hover_markdown(hir::DocumentationContent::Comment(comment), |target| {
            resolved.push(target);
            Some("file:///workspace/source.scoop#L5".into())
        });
    assert_eq!(resolved.len(), 1);
    assert!(
        rendered.contains("See **choice** and [`value`](<file:///workspace/source.scoop#L5>)"),
        "{rendered}"
    );
    assert!(rendered.contains("Visit [guide][site]"));
    assert!(rendered.contains("[site]: https://example.org/guide"));
}

#[test]
fn documentation_binding_can_be_cancelled_without_publishing_a_view() {
    let text = "/// First [second].\nfun first(): Boolean = true\n/// Second [first].\nfun second(): Boolean = false";
    let checks = std::cell::Cell::new(0);
    documented(text, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    let total = checks.get();
    checks.set(0);
    assert!(matches!(
        documented(text, || {
            checks.set(checks.get() + 1);
            checks.get() == total - 1
        }),
        Err(EditingAnalysisError::Cancelled)
    ));
    assert!(
        documented(text, || false)
            .unwrap()
            .documentation()
            .is_some()
    );
}

#[test]
fn recovered_function_bodies_keep_their_actual_document_attachment() {
    let text = "/// Uses [value].\n/// @param value The input.\nfun editing(value: Boolean): Boolean { value. }";
    let analysis = documented(text, || false).unwrap();
    let function = function(&analysis, "editing");
    let V::Known(_) = analysis.signature(function) else {
        panic!("the signature is complete")
    };
    let docs = analysis.documentation().unwrap();
    let target = Symbol::Current(
        hir::SourceAnnotationTarget::Callable(hir::SourceAnnotationCallable::Function(function))
            .into(),
    );
    let hir::DocumentationContent::Comment(comment) = docs.get(target).unwrap() else {
        panic!("the grammar attached a direct comment")
    };
    assert_eq!(comment.body.text.text, "Uses [value].");
    assert!(matches!(
        comment.body.references[0].resolution,
        Link::Symbols(_)
    ));
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn type_descriptions_use_their_original_m60_declaration_and_parameter_targets() {
    let text = "/// A documented box.\n/// @param Item The stored type.\nstruct Box<Item>(val value: Item)\n/// A documented alias.\n/// @param Element The wrapped type.\ntypealias Wrapped<Element> = Box<Element>\nfun use(value: Wrapped<Boolean>): Box<Boolean> = missing";
    let mut analysis = documented(text, || false).unwrap();
    let identity = crate::tests::test_source_identity("src/docs.scoop");
    for (needle, name, expected) in [
        ("Box<Item>", "Box", "A documented box."),
        ("Item>", "Item", "The stored type."),
        ("Wrapped<Boolean>", "Wrapped", "A documented alias."),
        ("Element>", "Element", "The wrapped type."),
    ] {
        let start = text.find(needle).unwrap() as u32;
        let V::Known(Some(description)) = analysis
            .type_description(
                &identity,
                ast::Span::new(start, start + name.len() as u32),
                || false,
            )
            .unwrap()
        else {
            panic!("complete type description")
        };
        let target = description
            .documentation
            .expect("original documentation target");
        let docs = analysis.documentation().unwrap();
        let content = docs.get(target).unwrap_or_else(|| {
            panic!(
                "missing M60 comment for {needle}: {target:?}; {:?}",
                docs.warnings
            )
        });
        let rendered = scoop_doc::hover_markdown(content, |_| None);
        assert!(rendered.contains(expected), "{rendered}");
    }
    assert!(analysis.body_facts.is_empty());
}
