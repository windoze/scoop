use super::super::*;

pub(in crate::tests) fn lower_with_sysroot(
    source: &str,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/src/scoop.core/src");
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "scoop")
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut files = paths
        .into_iter()
        .map(|path| scoop_parser::parse(&std::fs::read_to_string(path).unwrap()).unwrap())
        .collect::<Vec<_>>();
    files.push(scoop_parser::parse(source).unwrap());
    lower(&files)
}

// Compile this structural fixture as one real core Cone, including its coroutine helpers.
pub(in crate::tests) fn bootstrap_sysroot(
    source: &str,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/src/scoop.core/src");
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "scoop"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut text = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text.push_str(source);
    let identity = core_source_identity("src/core.scoop");
    let parsed = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(identity.clone(), scoop_parser::parse(&text).unwrap()),
            Vec::new(),
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(identity.clone(), text),
            Vec::new(),
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(identity, std::path::PathBuf::from("<core>")),
            Vec::new(),
        ),
    )
    .unwrap();
    crate::lower_core_bootstrap(&parsed)
}
