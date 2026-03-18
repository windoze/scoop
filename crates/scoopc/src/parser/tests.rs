use super::*;

#[test]
fn parse_minimal_file() {
    let src = SourceFile::new_virtual("<mem>", "package a.b\n\nfun main() { val x = 1 }");
    let ast = parse_file(&src).unwrap();
    assert!(ast.package.is_some());
    assert_eq!(ast.imports.len(), 0);
    assert_eq!(ast.items.len(), 1);
}

#[test]
fn parse_type_decls() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "open class A(x: Int) : B(x)\nstruct P(val x: Int)\nenum E { A, B }\nfun main() {}",
    );
    let ast = parse_file(&src).unwrap();
    assert_eq!(ast.items.len(), 4);
}

