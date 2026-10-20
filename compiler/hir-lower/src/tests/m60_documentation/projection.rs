use super::*;
use scoop_doc::{ApiItem, ApiKind, ConeInput, LinkTarget, Visibility};

mod annotations;
mod identity;
mod relations;
mod visibility;

pub(super) fn project(output: &hir::DependencyHirOutput) -> ConeInput {
    let output = output.output();
    scoop_doc::project_cone(
        &output.export,
        output.documentation.as_ref().unwrap(),
        &scoop_identity::ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0").unwrap(),
        scoop_identity::RequestedConeKind::Library,
    )
    .unwrap()
}

pub(super) fn item<'a>(input: &'a ConeInput, kind: ApiKind, name: &str) -> &'a ApiItem {
    input
        .items
        .iter()
        .find(|item| item.kind == kind && item.name == name)
        .unwrap_or_else(|| panic!("missing {kind:?} {name}"))
}

fn signature(item: &ApiItem) -> String {
    item.signature
        .iter()
        .map(|part| part.text.as_str())
        .collect()
}

#[test]
fn m60_documentation_projection_preserves_binder_names_source_aliases_and_markdown() {
    let source = "package `one.two`.api\r\n\
        public class Cell<Element>(public val value: Element) {\r\n\
          /** Reads [Element]. */\r\n\
          public fun read() = value\r\n\
        }\r\n\
        public typealias Count = Int\r\n\
        /** 文档 [Cell] and [Missing].\r\n\
         * @return Number of items.\r\n\
         */\r\n\
        public fun count(value: Count = 2) = value\r\n";
    let input = project(&lower_documented(source).unwrap());
    assert_eq!(input.sources.len(), 1);
    assert_eq!(input.sources[0].text, source);
    let read = item(&input, ApiKind::Function, "read");
    assert_eq!(signature(read), "public fun read(): Element");
    assert_eq!(read.package, ["one.two", "api"]);
    let count = item(&input, ApiKind::Function, "count");
    assert_eq!(signature(count), "public fun count(value: Count = 2): Int");
    assert_eq!(
        count
            .signature
            .iter()
            .find(|part| part.text == "Count")
            .unwrap()
            .link,
        Some(item(&input, ApiKind::TypeAlias, "Count").key.clone())
    );
    let cell = item(&input, ApiKind::Class, "Cell");
    assert_eq!(
        cell.signature
            .iter()
            .find(|part| part.text == "Element" && part.link.is_some())
            .unwrap()
            .link,
        Some(item(&input, ApiKind::TypeParameter, "Element").key.clone())
    );
    assert_eq!(count.location.line, 10);
    assert_eq!(input.warnings.len(), 1);
    let warning = &input.warnings[0];
    assert_eq!(warning.location.line, 7);
    assert!(
        source[warning.location.range.start as usize..warning.location.range.end as usize]
            .contains("Missing")
    );
    let comment = count.comment.as_ref().unwrap();
    assert_eq!(comment.tags[0].body.text, "Number of items.");
    assert_eq!(
        comment.body.references[0].target,
        LinkTarget::Symbols(vec![item(&input, ApiKind::Class, "Cell").key.clone()])
    );
    assert!(matches!(
        comment.body.references[1].target,
        LinkTarget::Unresolved
    ));
    assert_eq!(
        ConeInput::from_json(&input.to_json().unwrap()).unwrap(),
        input
    );
}

#[test]
fn m60_documentation_projection_retains_context_and_m55_contracts() {
    let input = project(
        &lower_documented(
            r#"
        /** Uses [request] and [T].
         * @param request Active request.
         * @param T Element.
         */
        context(request: String)
        public fun <T> Int.inspect(value: T): Int
            where requires(x: T) { x.read(): Int } = 1
        public val inferred = 42
        public val Int.successor: Int get() = 1
        "#,
        )
        .unwrap(),
    );
    let inspect = item(&input, ApiKind::Function, "inspect");
    assert_eq!(inspect.receiver.as_ref().unwrap().text, "Int");
    assert!(inspect.receiver.as_ref().unwrap().link.is_some());
    let property = item(&input, ApiKind::Property, "successor");
    assert_eq!(property.receiver, inspect.receiver);
    let signature = signature(inspect);
    assert!(signature.contains("context(request: String)"));
    assert!(signature.contains("fun <T> Int.inspect(value: T): Int"));
    assert!(signature.contains("where requires(x: T) { x.read(): Int }"));
    let references = &inspect.comment.as_ref().unwrap().body.references;
    assert_eq!(
        references[0].target,
        LinkTarget::Symbols(vec![
            item(&input, ApiKind::ContextParameter, "request")
                .key
                .clone()
        ])
    );
    assert_eq!(
        references[1].target,
        LinkTarget::Symbols(vec![item(&input, ApiKind::TypeParameter, "T").key.clone()])
    );
    assert_eq!(
        super::projection::signature(item(&input, ApiKind::Property, "inferred")),
        "public val inferred: Int"
    );
}
