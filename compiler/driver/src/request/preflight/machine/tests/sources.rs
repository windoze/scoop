use scoop_ast::{
    AllParsedSources, CurrentConeParsedSources, CurrentSourceDiagnosticContext, CurrentSourceText,
    IdentifiedParsedSource, NonEmptyVec,
};
use scoop_identity::{ConeIdentity, NormalizedSourcePath, SourceIdentity};

pub(super) fn core_sources() -> CurrentConeParsedSources {
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
    let fixtures = crate::workspace_root().join("tests/fixtures/core-library");
    for name in ["dependency-calls", "dependency-locals"] {
        sources.push((
            format!("src/user-{name}.scoop"),
            std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap(),
        ));
    }
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

pub(super) fn snapshot(stage: &str, dump: &str) {
    let mut sections = Vec::<String>::new();
    for line in dump.lines() {
        if line.starts_with("  ")
            && !line.starts_with("   ")
            && !(stage == "lir" && line.starts_with("  block "))
        {
            sections.push(String::new());
        }
        if let Some(section) = sections.last_mut() {
            section.push_str(line);
            section.push('\n');
        }
    }
    let result = sections
        .into_iter()
        .filter(|section| {
            section.contains("userCore")
                || section.contains("LocalTools")
                || section.contains("external-fn")
        })
        .collect::<String>();
    assert!(
        !result.is_empty(),
        "the golden must contain the fixture functions"
    );
    let path = crate::workspace_root().join(format!(
        "tests/fixtures/core-library/dependency-calls.driver-{stage}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_CORE_DEPENDENCY_SNAPSHOTS").is_some() {
        std::fs::write(&path, &result).unwrap();
    }
    assert_eq!(result, std::fs::read_to_string(path).unwrap());
}
