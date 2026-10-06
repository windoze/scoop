use super::super::*;

pub(in crate::tests) fn lower_with_sysroot(
    source: &str,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/lib/scoop.core/src");
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
