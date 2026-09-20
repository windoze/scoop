//! Complete lexical subtree projection used before nested candidate assembly.
use super::*;
use scoop_identity::DefinitionOriginSubject;

pub(in crate::production) struct NestedSourceNode {
    pub contract: NominalSourceContractV1,
    pub access: DeclarationAccessSourceV1,
}
pub(in crate::production) fn project(
    output: &ExportHirOutput,
    root: SourceNominalId,
    meter: &mut BudgetMeter,
) -> Result<Vec<NestedSourceNode>, Error> {
    let export = output.module();
    let path = WirePath::root();
    let mut index = roots::Index::new(export, meter)?;
    work(meter, index.nodes.len())?;
    if index
        .nodes
        .get(&root)
        .is_none_or(|node| node.parent.is_none())
    {
        return Err(invalid(
            "nested source root is absent or has no lexical owner",
        ));
    }
    let mut pending = Vec::new();
    push(&mut pending, (root, 1_u64), meter)?;
    let mut records = Vec::new();
    while let Some((owner, depth)) = pending.pop() {
        meter.check_semantic_depth(depth, &path).map_err(resource)?;
        work(meter, index.nodes.len())?;
        let node = index
            .nodes
            .remove(&owner)
            .ok_or_else(|| invalid("nested source ownership is missing, repeated or cyclic"))?;
        let source = node
            .local
            .identity(export)
            .and_then(HirNominalIdentity::source)
            .ok_or_else(|| invalid("nested source lacks its sealed nominal identity"))?;
        let contract = projection::project(export, node.local, source, meter)?;
        for child in contract.children().values() {
            push(&mut pending, (*child, depth + 1), meter)?;
        }
        let subject = match owner {
            SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
        };
        let owners = source.declaration().owners().owners().len() as u64;
        meter
            .charge_collection_slots(owners, &path)
            .map_err(resource)?;
        meter.charge_work(owners, &path).map_err(resource)?;
        work(meter, export.export_definition_origins.records().len())?;
        let origin = export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;
        resources::name(origin.origin().source().logical_path().as_str(), meter)?;
        let access = crate::production::type_semantics::declaration_access_for_subject(
            export,
            source.declaration(),
            subject,
            node.visibility.into(),
        )?;
        push(&mut records, NestedSourceNode { contract, access }, meter)?;
    }
    Ok(records)
}
