use super::*;

pub(super) fn nominals(
    table: &hir::CanonicalNominalInterfacesV1,
    identities: &ValidatedIdentityGraph,
) -> String {
    let mut records = table.all_records().map(|record| {
        let mut members = record.declaration_details().members().values().iter().map(|member| match member {
            hir::NestedSourceMemberRefV1::Function(id) => format!("fn {}", name(*id, identities)),
            hir::NestedSourceMemberRefV1::GenericFunction(id) => format!("generic {}", name(*id, identities)),
            hir::NestedSourceMemberRefV1::Property(id) => format!("property {}", name(*id, identities)),
        }).collect::<Vec<_>>();
        members.sort();
        let mut children = record.declaration_details().children().values().iter().map(|id| declaration_dump::nominal(*id, identities)).collect::<Vec<_>>();
        children.sort();
        let shape = match record.source_shape() {
            hir::NominalSourceShapeV1::Struct(shape) => format!("fields={}", shape.fields().len()),
            hir::NominalSourceShapeV1::Enum(shape) => shape.variants().iter().map(|variant| format!("{:?}:{}", variant.style(), variant.fields().len())).collect::<Vec<_>>().join(", "),
            hir::NominalSourceShapeV1::Class(_) => "class".into(),
            hir::NominalSourceShapeV1::Interface => "interface".into(),
            hir::NominalSourceShapeV1::Object(_) => "singleton".into(),
            hir::NominalSourceShapeV1::Intrinsic(representation) => format!("intrinsic={:?}", representation.family()),
        };
        format!("{} {:?}/{:?} binders={} parents={} constructors={}\n  members: {}\n  children: {}\n  shape: {}\n", declaration_dump::nominal(record.declaration(), identities), record.kind(), record.declaration_details().modality(), record.type_parameters().len_u32(), record.exact_supertypes().values().len(), record.declaration_details().constructors().values().len(), members.join(", "), children.join(", "), shape)
    }).collect::<Vec<_>>();
    records.sort();
    records
        .concat()
        .lines()
        .map(|line| format!("{}\n", line.trim_end()))
        .collect()
}
