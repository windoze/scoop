use scoop_ast::{ClassConstructorDecl, ClassMember, Decl, DocCommentKind, DocCommentTarget, Span};

use crate::parse;

fn target(file: &scoop_ast::SourceFile, text: &str) -> DocCommentTarget {
    file.documentation
        .iter()
        .find(|doc| doc.text == text)
        .unwrap_or_else(|| panic!("missing documentation {text:?}"))
        .target
}

fn attached(file: &scoop_ast::SourceFile, text: &str, span: Span) {
    assert_eq!(target(file, text), DocCommentTarget::Declaration(span));
}

#[test]
fn m60_doc_comments_keep_original_unicode_crlf_and_empty_blocks() {
    let source =
        "/// 中文 α\r\n/// second\r\n\n/** block\r\n * 文档\r\n */\n/**/\npublic fun f(): Int = 1";
    let file = parse(source).unwrap();
    assert_eq!(file.documentation.len(), 4);
    for doc in &file.documentation {
        assert_eq!(
            &source[doc.span.start as usize..doc.span.end as usize],
            doc.text
        );
        assert_eq!(
            doc.target,
            DocCommentTarget::Declaration(file.declarations[0].span())
        );
    }
    assert_eq!(file.documentation[0].kind, DocCommentKind::Line);
    assert_eq!(file.documentation[0].text, "/// 中文 α\r");
    assert_eq!(file.documentation[2].kind, DocCommentKind::Block);
    assert_eq!(file.documentation[3].text, "/**/");
}

#[test]
fn m60_ordinary_comments_and_string_text_are_not_documentation() {
    let source = r#"
//// ordinary
//! ordinary
/*** ordinary */
/* /** not nested */
val plain = "/// ignored /** ignored */"
val raw = """/** ignored */"""
val interpolated = f"/// ${1} /** ignored */"
"#;
    assert!(parse(source).unwrap().documentation.is_empty());
}

#[test]
fn m60_documentation_precedes_complete_annotation_and_modifier_prefix() {
    let source = r#"
/** callable */
// ordinary separator

@A
public inline fun f(): Int = 1
/** type */
@A
public open class C
/** alias */
public typealias Number = Int
/** annotation */
public annotation class Tag(/** argument */ val value: String)
"#;
    let file = parse(source).unwrap();
    for (text, decl) in [
        "/** callable */",
        "/** type */",
        "/** alias */",
        "/** annotation */",
    ]
    .into_iter()
    .zip(&file.declarations)
    {
        attached(&file, text, decl.span());
    }
    let Decl::AnnotationClass(tag) = &file.declarations[3] else {
        panic!()
    };
    attached(&file, "/** argument */", tag.parameters[0].span);
}

#[test]
fn m60_headers_expressions_and_scope_ends_do_not_forward_comments() {
    let source = r#"
/** package */
package sample
/** import */
import other.Type
/** good */
fun f(): Int {
    /** variable */
    val x: /** type use */ Int = /** expression */ 1
    /** statement */
    println(x)
    return x
    /** end of block */
}
fun g(): Int = f"${/** interpolation */ 1}".length
/** eof */
"#;
    let file = parse(source).unwrap();
    attached(&file, "/** good */", file.declarations[0].span());
    for doc in &file.documentation {
        if doc.text != "/** good */" {
            assert_eq!(doc.target, DocCommentTarget::Unattached, "{}", doc.text);
        }
    }
}

#[test]
fn m60_class_constructor_property_and_accessor_ownership_is_exact() {
    let source = r#"
/** class */
class C /** constructor */ constructor(/** field */ val x: Int, /** plain */ seed: Int) {
    /** property */
    var value: Int = seed
        /** getter */
        get() = x
        /** setter */
        private set(/** setter value */ next) {}
    /** secondary */
    constructor(/** parameter */ text: String) : this(1, 2) {}
    /** init is not an API */
    init {}
}
"#;
    let file = parse(source).unwrap();
    let Decl::Class(class) = &file.declarations[0] else {
        panic!()
    };
    attached(&file, "/** class */", class.span);
    let ClassConstructorDecl::Declared(primary) = &class.constructor else {
        panic!()
    };
    attached(&file, "/** constructor */", primary.span);
    attached(&file, "/** field */", primary.parameters[0].span);
    attached(&file, "/** plain */", primary.parameters[1].span);
    let ClassMember::StoredProperty(property) = &class.members[0] else {
        panic!()
    };
    attached(&file, "/** property */", property.span);
    let scoop_ast::PropertyBodySyntax::Initializer { accessors, .. } = &property.body else {
        panic!()
    };
    attached(
        &file,
        "/** getter */",
        accessors.getter.as_ref().unwrap().span,
    );
    let setter = accessors.setter.as_ref().unwrap();
    attached(&file, "/** setter */", setter.span);
    let scoop_ast::SetterParameterSyntax::Named(parameter) = &setter.parameter else {
        panic!()
    };
    attached(&file, "/** setter value */", parameter.span);
    let ClassMember::SecondaryConstructor(secondary) = &class.members[1] else {
        panic!()
    };
    attached(&file, "/** secondary */", secondary.span);
    attached(&file, "/** parameter */", secondary.params[0].span);
    assert_eq!(
        target(&file, "/** init is not an API */"),
        DocCommentTarget::Unattached
    );
}

#[test]
fn m60_value_type_fields_and_all_variant_payload_forms_are_documented() {
    let source = r#"
struct Pair(/** first */ val a: Int, /** second */ val b: Int) {
    /** secondary */
    constructor(/** scalar */ x: Int) : this(x, x) {}
}
enum Data {
    /** unit */ Empty,
    /** positional */ Value(/** payload */ Int),
    /** named */ Record { /** field */ x: Int },
    /** constructor */ Item(/** item */ val value: Int),
    /** method */ fun size(): Int = 1
    /** nested */ class Nested
    /** companion */ companion object {}
}
"#;
    let file = parse(source).unwrap();
    let Decl::Struct(pair) = &file.declarations[0] else {
        panic!()
    };
    attached(&file, "/** first */", pair.fields[0].span);
    attached(&file, "/** second */", pair.fields[1].span);
    attached(&file, "/** secondary */", pair.members[0].span());
    let Decl::Enum(data) = &file.declarations[1] else {
        panic!()
    };
    attached(&file, "/** unit */", data.variants[0].span);
    attached(&file, "/** positional */", data.variants[1].span);
    let scoop_ast::VariantDeclKind::Positional(fields) = &data.variants[1].kind else {
        panic!()
    };
    attached(&file, "/** payload */", fields[0].span);
    let scoop_ast::VariantDeclKind::Named(fields) = &data.variants[2].kind else {
        panic!()
    };
    attached(&file, "/** field */", fields[0].span);
    let scoop_ast::VariantDeclKind::Constructor(fields) = &data.variants[3].kind else {
        panic!()
    };
    attached(&file, "/** item */", fields[0].span);
    attached(&file, "/** method */", data.methods[0].span);
    attached(&file, "/** nested */", data.nested[0].span());
    attached(
        &file,
        "/** companion */",
        data.companion.as_ref().unwrap().span,
    );
    assert!(
        file.documentation
            .iter()
            .all(|doc| matches!(doc.target, DocCommentTarget::Declaration(_)))
    );
}

#[test]
fn m60_interface_members_and_local_declarations_preserve_ownership() {
    let source = r#"
interface API {
    /** function */ fun call(/** arg */ x: Int): Int
    /** property */ val value: Int
    /** alias */ typealias Number = Int
    /** annotation */ annotation class Tag
    /** object */ object Nested {}
}
fun outer(): Int {
    /** local function */ fun inner(): Int = 1
    /** local type */ class Local
    /** local alias */ typealias Count = Int
    return inner()
}
"#;
    let file = parse(source).unwrap();
    let Decl::Interface(api) = &file.declarations[0] else {
        panic!()
    };
    attached(&file, "/** function */", api.methods[0].span);
    attached(&file, "/** arg */", api.methods[0].params[0].span);
    attached(&file, "/** property */", api.properties[0].span);
    let Decl::Function(outer) = &file.declarations[1] else {
        panic!()
    };
    let scoop_ast::FunctionBody::Block(body) = &outer.body else {
        panic!()
    };
    for (text, statement) in [
        "/** local function */",
        "/** local type */",
        "/** local alias */",
    ]
    .into_iter()
    .zip(&body.statements)
    {
        attached(&file, text, statement.span);
    }
    assert!(
        file.documentation
            .iter()
            .all(|doc| matches!(doc.target, DocCommentTarget::Declaration(_)))
    );
}

#[test]
fn m60_context_and_generic_parameters_are_attached_to_syntax_nodes() {
    let source = "context(/** context */ environment: Env)\npublic fun </** type */ T> f(/** value */ value: T): T = value";
    let file = parse(source).unwrap();
    let Decl::Function(function) = &file.declarations[0] else {
        panic!()
    };
    attached(&file, "/** context */", function.context_parameters[0].span);
    attached(&file, "/** type */", function.type_params[0].span);
    attached(&file, "/** value */", function.params[0].span);
}

#[test]
fn m60_unterminated_doc_blocks_keep_normal_lexical_errors() {
    let errors = parse("fun f(): Int = 1\n/** no end").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message == "unterminated block comment")
    );
}

#[test]
fn m60_bodyless_class_leaves_the_next_declaration_prefix_intact() {
    for prefix in ["public", "@A\npublic", "/** next */\n@A\npublic"] {
        let source = format!("class First\n{prefix} fun next(): Int = 1");
        let file = parse(&source).unwrap();
        assert_eq!(file.declarations.len(), 2);
        let Decl::Function(function) = &file.declarations[1] else {
            panic!()
        };
        assert_eq!(function.name.text, "next");
        if prefix.contains("@A") {
            assert_eq!(function.annotations.len(), 1);
        }
        if prefix.contains("/** next */") {
            attached(&file, "/** next */", function.span);
        }
    }
    let file = parse("class First\n/** primary */ @A private constructor(x: Int)").unwrap();
    let Decl::Class(class) = &file.declarations[0] else {
        panic!()
    };
    let ClassConstructorDecl::Declared(primary) = &class.constructor else {
        panic!()
    };
    attached(&file, "/** primary */", primary.span);
    assert_eq!(primary.annotations.len(), 1);
}
