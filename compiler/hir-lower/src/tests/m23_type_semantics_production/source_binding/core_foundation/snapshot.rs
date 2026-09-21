use super::*;

pub(super) fn render(output: &hir::Output) -> String {
    let source = Production::from_core_bootstrap(output, &mut meter()).unwrap();
    let transcript = source.source_transcript(&mut meter()).unwrap();
    let entries = transcript.entries();
    let mut rows = Vec::new();
    for owner in source.source_roots() {
        let key = source.nominal_declaration_key(*owner).unwrap();
        let shape = match owner {
            hir::SourceNominalId::GenericTemplate(_) => "generic",
            hir::SourceNominalId::Concrete(id) => {
                use hir::NominalRepresentationShapeV1 as Shape;
                match entries.representations.get(*id).unwrap().shape() {
                    Shape::Struct { .. } => "struct",
                    Shape::Enum { .. } => "enum",
                    Shape::Class { .. } => "class",
                    Shape::Object { .. } => "object",
                    Shape::Interface => "interface",
                    Shape::Intrinsic { .. } => "intrinsic",
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
