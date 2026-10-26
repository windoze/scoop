use scoop_ast::CurrentConeParsedSources;
use scoop_hir::{DefinitionOrigin, TestContainer};
use scoop_slib::{TestCaseListing, TestContainerListing, TestDirectory, TestSourceLocation};

pub(in crate::request::preflight) fn project(
    containers: &[TestContainer],
    sources: &CurrentConeParsedSources,
) -> TestDirectory {
    let sources = sources.iter().collect::<Vec<_>>();
    let location = |origin: DefinitionOrigin| {
        let source = sources[origin.file as usize];
        let prefix = &source.text().text()[..origin.span.start as usize];
        TestSourceLocation {
            path: source
                .source()
                .identity()
                .logical_path()
                .as_str()
                .to_owned(),
            line: u32::try_from(prefix.bytes().filter(|b| *b == b'\n').count() + 1)
                .expect("source line fits its u32 byte span"),
            column: u32::try_from(prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1)
                .expect("source column fits its u32 byte span"),
        }
    };
    TestDirectory {
        containers: containers
            .iter()
            .map(|container| TestContainerListing {
                qualified_name: container.qualified_name.clone(),
                location: location(container.origin),
                cases: container
                    .cases
                    .iter()
                    .map(|case| TestCaseListing {
                        name: case.method.name.clone(),
                        display_name: case.display_name.clone(),
                        location: location(case.method.origin),
                        ignore_reason: case.ignore_reason.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}
