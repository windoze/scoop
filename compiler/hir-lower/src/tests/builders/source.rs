use super::*;

pub(crate) fn file(declarations: Vec<Decl>) -> SourceFile {
    SourceFile {
        package: ast::PackageSyntax::RootPackage,
        imports: Vec::new(),
        declarations,
        span: Span::new(0, 100),
    }
}
