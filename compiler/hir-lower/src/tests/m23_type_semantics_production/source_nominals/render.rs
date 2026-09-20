use super::*;

pub(super) fn render(export: &hir::ExportHir, table: &Table) -> String {
    let sources = sources(export);
    let mut functions = BTreeMap::new();
    let mut generics = BTreeMap::new();
    for (id, _) in export.functions.iter() {
        if let hir::HirFunctionIdentity::Source(source) = &export.function_identities[id] {
            match source {
                hir::HirSourceFunctionIdentity::Plain(record) => {
                    functions.insert(record.id(), name(source.declaration()));
                }
                hir::HirSourceFunctionIdentity::Generic(record) => {
                    generics.insert(record.id(), name(source.declaration()));
                }
            }
        }
    }
    let properties = export
        .properties
        .iter()
        .filter_map(|(id, _)| match &export.property_identities[id] {
            hir::HirPropertyIdentity::Ordinary(record) => Some((record.id(), name(record.key()))),
            hir::HirPropertyIdentity::Extension(_) => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut records = table.records().iter().map(|record| {
        let mut members = record.members().values().iter().map(|member| match member {
            hir::NestedSourceMemberRefV1::Function(id) => format!("fn {}", functions[id]),
            hir::NestedSourceMemberRefV1::GenericFunction(id) => format!("generic {}", generics[id]),
            hir::NestedSourceMemberRefV1::Property(id) => format!("property {}", properties[id]),
        }).collect::<Vec<_>>();
        members.sort();
        let mut children = record.children().values().iter().map(|id| name(sources[id])).collect::<Vec<_>>();
        children.sort();
        let shape = match record.source_shape() {
            hir::NominalSourceShapeV1::Struct(shape) => format!("fields={}", shape.fields().len()),
            hir::NominalSourceShapeV1::Enum(shape) => shape.variants().iter().map(|variant| format!("{:?}:{}", variant.style(), variant.fields().len())).collect::<Vec<_>>().join(", "),
            hir::NominalSourceShapeV1::Class => "class".into(),
            hir::NominalSourceShapeV1::Interface => "interface".into(),
            hir::NominalSourceShapeV1::Object(_) => "singleton".into(),
        };
        format!("{} {:?}/{:?} binders={} parents={} constructors={}\n  members: {}\n  children: {}\n  shape: {}\n", name(sources[&record.owner()]), record.kind(), record.modality(), record.type_parameters().len_u32(), record.supertypes().values().len(), record.constructors().values().len(), members.join(", "), children.join(", "), shape)
    }).collect::<Vec<_>>();
    records.sort();
    records.concat()
}
