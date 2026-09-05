//! M12 typed annotations, lexical safety and `@NoGC` verification.

use scoop_ast as ast;
use scoop_hir as hir;

use super::*;

fn marker(name: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident(name),
        args: Vec::new(),
        span: sp(),
    }
}

fn string_annotation(name: &str, value: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident(name),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(value.to_string()),
            span: sp(),
        }],
        span: sp(),
    }
}

fn annotate(mut decl: Decl, annotations: Vec<ast::Annotation>) -> Decl {
    let Decl::Function(function) = &mut decl else {
        panic!("expected function declaration");
    };
    function.annotations = annotations;
    decl
}

fn annotate_method(mut method: FunctionDecl, annotations: Vec<ast::Annotation>) -> FunctionDecl {
    method.annotations = annotations;
    method
}

fn annotate_struct(mut decl: Decl, annotations: Vec<ast::Annotation>) -> Decl {
    let Decl::Struct(strukt) = &mut decl else {
        panic!("expected struct declaration");
    };
    strukt.annotations = annotations;
    decl
}

fn annotate_enum(mut decl: Decl, annotations: Vec<ast::Annotation>) -> Decl {
    let Decl::Enum(enumeration) = &mut decl else {
        panic!("expected enum declaration");
    };
    enumeration.annotations = annotations;
    decl
}

fn c_layout(aligned: i64, packed: i64) -> ast::Annotation {
    ast::Annotation {
        name: ident("CLayout"),
        args: vec![
            ast::AnnotationArg {
                name: Some(ident("aligned")),
                value: ast::AnnotationLiteral::Int(aligned),
                span: sp(),
            },
            ast::AnnotationArg {
                name: Some(ident("packed")),
                value: ast::AnnotationLiteral::Int(packed),
                span: sp(),
            },
        ],
        span: sp(),
    }
}

fn extern_annotation(lib: &str, name: &str, abi: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident("Extern"),
        args: [("lib", lib), ("name", name), ("abi", abi)]
            .into_iter()
            .map(|(parameter, value)| ast::AnnotationArg {
                name: Some(ident(parameter)),
                value: ast::AnnotationLiteral::String(value.to_string()),
                span: sp(),
            })
            .collect(),
        span: sp(),
    }
}

fn extern_fun(
    name: &str,
    params: Vec<(&str, ast::TypeRef)>,
    return_ty: Option<ast::TypeRef>,
    annotation: ast::Annotation,
) -> Decl {
    let mut declaration = fun_sig(name, vec![], params, return_ty, vec![]);
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.annotations = vec![annotation];
    function.body = ast::FunctionBody::None;
    declaration
}

fn with_kind(mut decl: Decl, kind: ast::TypeParamKindBound) -> Decl {
    let type_params = match &mut decl {
        Decl::Function(decl) => &mut decl.type_params,
        Decl::Struct(decl) => &mut decl.type_params,
        Decl::Enum(decl) => &mut decl.type_params,
        Decl::Interface(decl) => &mut decl.type_params,
        Decl::Class(decl) => &mut decl.type_params,
        Decl::Object(_) => panic!("objects have no type parameters"),
        Decl::Global(_) => panic!("globals have no type parameters"),
        Decl::TypeAlias(_) => panic!("M22 typealiases have no type parameters"),
    };
    type_params[0].inline_bound = Some(ast::TypeBound::Kind(kind));
    decl
}

fn safety_block(mode: ast::SafetyMode, statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::SafetyBlock {
            mode,
            block: block(statements),
        },
        span: sp(),
    }
}

fn messages(decls: Vec<Decl>) -> Vec<String> {
    lower_user(file(decls))
        .expect_err("program must be rejected")
        .into_iter()
        .map(|error| error.message)
        .collect()
}

mod annotations;
mod externs;
mod ffi_types;
mod generic_bounds;
mod interior_mutability;
mod no_gc;
mod no_gc_generics;
mod safety;
