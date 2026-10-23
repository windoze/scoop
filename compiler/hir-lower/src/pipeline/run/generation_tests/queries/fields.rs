use super::types::resolve;
use super::*;

fn source_text<'a>(source: &'a str, origin: &hir::SemanticSourceLocation) -> &'a str {
    assert_eq!(
        origin.source,
        crate::tests::test_source_identity("src/user.scoop")
    );
    &source[origin.span.start as usize..origin.span.end as usize]
}

#[test]
fn field_queries_keep_declaration_types_identity_order_and_exact_source() {
    let source = r#"
        annotation class Label(val text: String)
        struct Box<T>(@Label("值") val value: T, val fixed: Int, val pair: (T, Int), val apply: (T) -> T)
        typealias Wrapped<T> = Box<T>
        @Label("inspect") fun inspect() {}
    "#;
    let mut observed = None;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let integer = resolve(queries, context, "Int");
        let string = resolve(queries, context, "String");
        let first_type = resolve(queries, context, "Wrapped<Int>");
        let second_type = resolve(queries, context, "Box<String>");
        let first = queries
            .nominal_fields(context, first_type)
            .unwrap()
            .unwrap();
        let second = queries
            .nominal_fields(context, second_type)
            .unwrap()
            .unwrap();
        assert_eq!(first.declaration, second.declaration);
        assert_eq!(
            first
                .fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["value", "fixed", "pair", "apply"]
        );
        for (first, second) in first.fields.iter().zip(&second.fields) {
            assert_eq!(first.identity, second.identity);
            assert_eq!(first.value_type.declaration, second.value_type.declaration);
            assert_eq!(first.storage, hir::NominalFieldStorage::Declared);
            assert!(source_text(source, &first.origin).contains(&format!("val {}:", first.name)));
        }
        assert_eq!(first.fields[0].value_type.applied, integer);
        assert_eq!(second.fields[0].value_type.applied, string);
        assert_ne!(first.fields[0].value_type.declaration, integer);
        assert_eq!(
            first.fields[1].value_type,
            hir::SemanticTypeUse {
                declaration: integer,
                applied: integer
            }
        );
        let pair = resolve(queries, context, "(String, Int)");
        assert_eq!(second.fields[2].value_type.applied, pair);
        let callback = resolve(queries, context, "(String) -> String");
        assert_eq!(second.fields[3].value_type.applied, callback);
        let hir::SemanticAnnotations::Current(annotations) =
            queries.describe_annotations(first.fields[0].annotations)
        else {
            panic!("source field annotations")
        };
        assert_eq!(annotations.len(), 1);
        assert!(queries.nominal_fields(context, pair).unwrap().is_none());
        observed = Some(first.fields[0].identity);
        Ok(Vec::new())
    });
    let module = compile(source, &mut driver).unwrap();
    let (structure, _) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Box")
        .unwrap();
    let field = hir::StructFieldRef::checked(&module.structs, structure, 0).unwrap();
    assert_eq!(
        observed,
        Some(hir::StaticFieldIdentity::Field(
            module.field_identities[field].id()
        ))
    );
}

#[test]
fn variant_queries_preserve_payload_styles_and_distinct_same_named_fields() {
    let source = r#"
        annotation class Label(val text: String)
        enum Choice<T> {
            @Label("empty") Empty,
            Named(@Label("named") val value: T, val count: Int = 4),
            Other(val value: T),
            Pair(T, Int)
        }
        @Label("inspect") fun inspect() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let ty = resolve(queries, context, "Choice<String>");
        let string = resolve(queries, context, "String");
        let fields = queries.nominal_fields(context, ty).unwrap().unwrap();
        assert!(fields.fields.is_empty());
        assert_eq!(
            fields
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            ["Empty", "Named", "Other", "Pair"]
        );
        let variants = &fields.variants;
        assert_eq!(variants[0].style, hir::VariantStyle::Unit);
        assert_eq!(variants[3].style, hir::VariantStyle::Positional);
        assert_eq!(variants[1].fields[0].value_type.applied, string);
        assert_ne!(
            variants[1].fields[0].identity,
            variants[2].fields[0].identity
        );
        assert!(source_text(source, &variants[1].origin).starts_with("Named("));
        assert!(source_text(source, &variants[1].fields[0].origin).contains("val value: T"));
        for target in [variants[0].annotations, variants[1].fields[0].annotations] {
            let hir::SemanticAnnotations::Current(annotations) =
                queries.describe_annotations(target)
            else {
                panic!("source enum annotations")
            };
            assert_eq!(annotations.len(), 1);
        }
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}

#[test]
fn class_storage_queries_keep_base_computed_and_delegate_members_distinct() {
    let source = r#"
        annotation class Mark
        open class Base<T>(val base: T)
        class Row<T>(val value: T, private var count: Int = 0) : Base<T>(value) {
            val computed: Int get() = count
        }
        class Slot<T>(private val value: T) {
            operator fun getValue(thisRef: Holder<T>): T = value
        }
        class Holder<T>(value: T) { val stored: T by Slot<T>(value) }
        class Node(val next: Option<Node>)
        @Mark fun inspect() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let row = resolve(queries, context, "Row<String>");
        let fields = queries
            .nominal_fields(context, row)
            .unwrap()
            .unwrap()
            .fields;
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["value", "count"]
        );
        assert!(
            fields
                .iter()
                .all(|field| matches!(field.storage, hir::NominalFieldStorage::PropertyBacking(_)))
        );
        assert_ne!(fields[0].storage, fields[1].storage);
        let holder = resolve(queries, context, "Holder<String>");
        let fields = queries
            .nominal_fields(context, holder)
            .unwrap()
            .unwrap()
            .fields;
        assert_eq!(fields.len(), 1);
        assert!(matches!(
            fields[0].storage,
            hir::NominalFieldStorage::PropertyDelegate(_)
        ));
        let slot = resolve(queries, context, "Slot<String>");
        assert_eq!(fields[0].value_type.applied, slot);
        let node = resolve(queries, context, "Node");
        let next = resolve(queries, context, "Option<Node>");
        let fields = queries
            .nominal_fields(context, node)
            .unwrap()
            .unwrap()
            .fields;
        assert_eq!(fields[0].value_type.applied, next);
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
