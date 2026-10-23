use super::*;

#[test]
fn secondary_parameters_keep_comments_without_annotation_or_body_evaluation() {
    let text = "class Box<T>(val value: T) {\n/// @param copy The class input.\nconstructor(copy: T, marker: Boolean): this(value = copy) { val echo = copy }\n}\nstruct Value(val value: Boolean) {\n/// @param seed The struct input.\nconstructor(seed: String): this(value = true) { val echo = seed }\n}";
    let analysis = documented(text, || false).unwrap();
    for (needle, expected) in [
        ("echo = copy", "The class input."),
        ("echo = seed", "The struct input."),
    ] {
        let start = text.find(needle).unwrap() as u32;
        let body = analysis
            .source_bodies()
            .into_iter()
            .find(|body| body.origin.span.start <= start && start < body.origin.span.end)
            .unwrap();
        let callable = match body.callee().unwrap() {
            hir::EditingCallee::Constructor(hir::SemanticConstructor::CurrentClass(id)) => {
                hir::SourceAnnotationCallable::ClassConstructor(id)
            }
            hir::EditingCallee::Constructor(hir::SemanticConstructor::CurrentStruct(id)) => {
                hir::SourceAnnotationCallable::StructConstructor(id)
            }
            _ => panic!("the body retains its actual secondary constructor"),
        };
        let target = hir::SourceAnnotationTarget::Parameter { callable, index: 0 };
        let docs = analysis.documentation().unwrap();
        assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
        assert_eq!(
            scoop_doc::hover_markdown(docs.get(Symbol::Current(target.into())).unwrap(), |_| None),
            expected
        );
    }
    assert!(analysis.body_facts.is_empty());
}
