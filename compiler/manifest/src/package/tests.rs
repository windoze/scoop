use super::*;

fn parse(body: &str) -> Result<PackageDocument, ManifestParseError> {
    parse_package_manifest(body, crate::test_catalog::SOURCE_TARGETS)
}

const CONE: &str = "schema = 1\n[cone]\ngroup = 'dev.example'\nname = 'app'\nversion = '0.1.0'\nkind = 'executable'\n";

#[test]
fn virtual_workspaces_do_not_silently_accept_build_inputs() {
    for fields in [
        "[native]\ncxx = true",
        "[windows]\nsubsystem = 'gui'",
        "[publish]\ninclude = ['src']",
        "[dependencies]\n'org.example:a' = '1.0.0'",
    ] {
        assert!(
            parse(&format!(
                "schema = 1\n[workspace]\nmembers = []\n{fields}\n"
            ))
            .is_err()
        );
    }
}

#[test]
fn windows_configuration_survives_package_binding_and_json_handoff() {
    let document = parse(&format!(
        "{CONE}\n[windows]\nsubsystem = 'gui'\nrc = '../resources/app.rc'\n"
    ))
    .unwrap();
    let manifest = crate::ParsedConeManifest::bind_package(
        &document,
        scoop_identity::ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap(),
        &BTreeMap::new(),
        false,
    )
    .unwrap();
    let decoded: crate::ParsedConeManifest =
        serde_json::from_slice(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        decoded.semantic().windows().subsystem(),
        scoop_identity::WindowsSubsystem::Gui
    );
    assert_eq!(
        decoded.semantic().windows().rc().unwrap().as_str(),
        "../resources/app.rc"
    );
}

#[test]
fn parses_sources_without_selecting_dependency_versions() {
    let doc = parse(&format!("{CONE}\n[dependencies]\n\
        'org.example:json' = '1.4.0'\n\
        'dev.example:model' = {{ path = '../model' }}\n\
        'org.example:http' = {{ github = 'example/http', tag = 'v1.2.0', subdir = 'cones/http', version = '^1.2.0' }}\n\
        'org.vendor:codec' = {{ artifact = '../codec.slib', version = '=2.0.1' }}\n")).unwrap();
    let DependencyDeclaration::Request(http) =
        &doc.dependencies[&"org.example:http".parse().unwrap()]
    else {
        panic!()
    };
    let PackageSource::Git(git) = &http.source else {
        panic!()
    };
    assert_eq!(git.repository, "https://github.com/example/http.git");
    assert_eq!(git.reference, GitReference::Tag("v1.2.0".into()));
    assert_eq!(git.subdir.as_str(), "cones/http");
    assert_eq!(http.requirement.as_ref().unwrap().as_str(), "^1.2.0");
    assert_eq!(doc.dependencies.len(), 4);
}

#[test]
fn workspace_fields_retain_declaration_spans() {
    let text = "schema = 1\n[workspace]\nmembers = ['app']\n[workspace.cone]\ngroup = 'dev.example'\n[workspace.dependencies]\n'org.example:json' = '1.4.0'\n";
    let doc = parse(text).unwrap();
    assert!(doc.cone.is_none());
    let ws = doc.workspace.unwrap();
    assert_eq!(&text[ws.cone["group"].span.clone()], "'dev.example'");
    assert_eq!(ws.members[0].value.as_str(), "app");
    let inherited = parse("schema = 1\n[cone]\ngroup = { workspace = true }\nname = 'app'\nversion = { workspace = true }\nkind = 'library'\n[dependencies]\n'org.example:json' = { workspace = true }\n").unwrap();
    assert!(matches!(
        inherited.cone.unwrap().group,
        InheritableValue::Workspace { .. }
    ));
}

#[test]
fn rejects_invalid_dependency_shapes_at_the_declaration() {
    for value in [
        "{ path = '../a', git = 'https://example.org/a' }",
        "{ workspace = true, version = '1.0.0' }",
        "{ workspace = false }",
        "{ git = 'https://example.org/a', branch = 'main', rev = 'abc' }",
        "{ github = 'example/a', subdir = '../a' }",
        "{ github = 'https://github.com/example/a' }",
        "{ version = '1.0.0', branch = 'main' }",
        "{ artifact = 'a.slib', version = '1.0.0' }",
        "{ registry = 'https://example.org/' }",
        "{ version = '1.0.0', optional = true }",
        "'1.2'",
    ] {
        let text = format!("{CONE}[dependencies]\n'org.example:a' = {value}\n");
        let error = parse(&text).unwrap_err();
        assert_eq!(&text[error.span().unwrap().range()], value, "{error}");
    }
}

#[test]
fn checks_test_declaration_format_but_does_not_access_sources() {
    let text = format!(
        "{CONE}[test-dependencies]\n'org.example:test' = {{ path = '/does-not-exist', version = '1.0.0' }}\n"
    );
    assert_eq!(parse(&text).unwrap().test_dependencies.len(), 1);
    let duplicate = format!(
        "{CONE}[dependencies]\n'org.example:a' = '1.0.0'\n[test-dependencies]\n'org.example:a' = '1.0.0'\n"
    );
    assert!(matches!(
        parse(&duplicate).unwrap_err().kind(),
        ManifestParseErrorKind::DuplicateTestDependency(_)
    ));
}

#[test]
fn normalizes_registry_urls_and_reuses_source_and_native_validation() {
    assert_eq!(
        "HTTPS://EXAMPLE.ORG:443/index"
            .parse::<RegistryUrl>()
            .unwrap()
            .as_str(),
        "https://example.org/index/"
    );
    for url in [
        "file:///index",
        "https://user:password@example.org/",
        "https://example.org/?x",
        "https://example.org/#x",
    ] {
        assert!(url.parse::<RegistryUrl>().is_err());
    }
    let source = format!("{CONE}[[sources]]\npath = '../escape'\n");
    assert!(matches!(
        parse(&source).unwrap_err().kind(),
        ManifestParseErrorKind::InvalidSelection(_)
    ));
    let native = format!("{CONE}[native]\nc_flags = ['-o', 'out']\n");
    assert!(matches!(
        parse(&native).unwrap_err().kind(),
        ManifestParseErrorKind::InvalidNative(_)
    ));
}
