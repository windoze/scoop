use super::*;

#[test]
fn compiler_exception_constructors_cannot_be_unsafe() {
    for name in ["Throwable", "ArithmeticException"] {
        let mut core = complete_core_file();
        let declaration = core
            .declarations
            .iter_mut()
            .find_map(|declaration| match declaration {
                ast::Decl::Class(class) if class.name.text == name => Some(class),
                _ => None,
            })
            .unwrap();
        let ast::ClassConstructorDecl::Declared(constructor) = &mut declaration.constructor else {
            panic!("explicit core constructor");
        };
        constructor.annotations.push(ast::Annotation {
            name: ident("Unsafe"),
            args: vec![],
            span: declaration.span,
        });
        let errors = lower(&[core, scoop_parser::parse("fun main() {}").unwrap()]).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.message
                    == format!("compiler exception constructor `{name}` must be safe")),
            "{errors:?}"
        );
    }
}
