use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext, CurrentSourceText,
    IdentifiedParsedSource, NonEmptyVec,
};
use scoop_identity::{ConeIdentity, NormalizedSourcePath, SourceIdentity};

pub(super) fn core_sources() -> CurrentConeParsedSources {
    let fixtures = crate::workspace_root().join("tests/fixtures/core-library");
    let additional = ["dependency-calls", "dependency-locals"].map(|name| {
        (
            format!("src/user-{name}.scoop"),
            std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap(),
        )
    });
    core_sources_with(
        &additional
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect::<Vec<_>>(),
    )
}

pub(super) fn core_sources_with(additional: &[(&str, &str)]) -> CurrentConeParsedSources {
    let root = crate::workspace_root().join("sysroot/lib/scoop.core/src");
    let mut sources = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "scoop")
        })
        .map(|path| {
            (
                format!("src/{}", path.file_name().unwrap().to_str().unwrap()),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    sources.extend(
        additional
            .iter()
            .map(|(path, text)| (path.to_string(), text.to_string())),
    );
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut diagnostics = Vec::new();
    for (path, text) in sources {
        let identity = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new(&path).unwrap(),
        )
        .unwrap();
        parsed.push(IdentifiedParsedSource::new(
            identity.clone(),
            scoop_parser::parse(&text).unwrap(),
        ));
        texts.push(CurrentSourceText::new(identity.clone(), text));
        diagnostics.push(CurrentSourceDiagnosticContext::new(identity, path.into()));
    }
    CurrentConeParsedSources::try_new(
        AllParsedSources::try_new(NonEmptyVec::new(parsed.remove(0), parsed)).unwrap(),
        NonEmptyVec::new(texts.remove(0), texts),
        NonEmptyVec::new(diagnostics.remove(0), diagnostics),
    )
    .unwrap()
}
