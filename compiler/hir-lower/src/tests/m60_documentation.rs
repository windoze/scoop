use super::m23_ordinary_core_only::support;
use super::*;
use hir::{
    DocumentationKind as Kind, DocumentationLink as Link, DocumentationSignaturePart as Part,
    DocumentationSymbol as Symbol, DocumentationTagKind as Tag, DocumentationTarget as Target,
    SourceAnnotationCallable as Callable, SourceAnnotationTarget as Source,
};

mod combinations;
mod declarations;
mod dependencies;
mod links;
mod scopes;
mod sysroot;
mod warnings;

fn lower_documented(source: &str) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    let core = support::trusted_core();
    lower_with_core(&core, source, true)
}

fn lower_with_core(
    core: &support::TrustedCoreFixture,
    source: &str,
    documentation: bool,
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    let parsed = support::parsed_ordinary_text(source);
    let world = core.world(parsed.cone());
    let mut sources = crate::CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    if documentation {
        let file = scoop_parser::parse(source).unwrap();
        sources = sources
            .with_documentation(vec![ast::IdentifiedSourceDocumentation {
                source: test_source_identity("src/main.scoop"),
                documentation: scoop_doc::prepare_source(&file, source),
            }])
            .unwrap();
    }
    crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &sources)
}

fn named<'a>(
    output: &'a hir::DependencyHirOutput,
    kind: Kind,
    name: &str,
) -> &'a hir::DocumentationDeclaration {
    output
        .output()
        .documentation
        .as_ref()
        .unwrap()
        .declarations
        .iter()
        .find(|entry| entry.kind == kind && entry.name == name)
        .unwrap_or_else(|| panic!("missing {kind:?} {name}"))
}

fn comment<'a>(
    output: &'a hir::DependencyHirOutput,
    kind: Kind,
    name: &str,
) -> &'a hir::DocumentationComment {
    named(output, kind, name).comment.as_ref().unwrap()
}

fn current(link: &Link) -> Target {
    let Link::Symbols(symbols) = link else {
        panic!("expected a resolved reference");
    };
    assert_eq!(symbols.len(), 1);
    let Symbol::Current(target) = symbols.first() else {
        panic!("expected a current declaration");
    };
    *target
}
