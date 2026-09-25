use super::*;

pub(super) fn render(output: &hir::Output) -> String {
    let source = Production::from_hir(output).unwrap();
    let transcript = source.source_transcript().unwrap();
    let entries = transcript.entries();
    let mut rows = Vec::new();
    for owner in source.source_roots() {
        let key = source.nominal_declaration_key(*owner).unwrap();
        let shape = match owner {
            hir::SourceNominalId::GenericTemplate(_) => "generic",
            hir::SourceNominalId::Concrete(id) => {
                use hir::NominalRepresentationShapeV1 as Shape;
                match entries
                    .representations
                    .get(*id)
                    .map(hir::NominalRepresentationSupportV1::shape)
                {
                    Some(Shape::Struct { .. }) => "struct",
                    Some(Shape::Enum { .. }) => "enum",
                    Some(Shape::Class { .. }) => "class",
                    Some(Shape::Object { .. }) => "object",
                    Some(Shape::Interface) => "interface",
                    Some(Shape::Intrinsic { .. }) => "intrinsic",
                    None => "source-only",
                }
            }
        };
        let scoop_identity::DeclarationName::Named(name) = key.name() else {
            panic!("nominal name")
        };
        rows.push(format!(
            "{}: arity {}; {shape}",
            name.as_str(),
            key.duplicate_signature().type_parameter_count()
        ));
    }
    rows.sort();
    format!(
        "roots {}; representations {}; facts {}; inheritance edges {}; origins {}\n{}\n",
        source.source_roots().len(),
        entries.representations.records().len(),
        entries.fact_shapes.records().len(),
        entries.local_inheritance_edges.records().len(),
        entries.definition_sources.sources().len(),
        rows.join("\n")
    )
}

pub(super) fn check(output: &hir::Output, name: &str) {
    let actual = render(output);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-type-source-defaults/{name}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_SOURCE_ONLY_SNAPSHOTS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
