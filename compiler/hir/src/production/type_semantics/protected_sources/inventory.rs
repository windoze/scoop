use super::*;

impl CanonicalProtectedDeclarationRefsV1 {
    /// Selects required protected declarations before any candidate is built.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let roots = CanonicalSourceNominalIdsV1::from_export_hir(output, meter)?;
        let export = output.module();
        let mut members =
            super::super::inheritance::source_inventory::protected_members(export, meter)?;
        let mut records = Vec::new();
        for owner in roots.values() {
            work(meter, members.len())?;
            if let Some(required) = members.remove(owner) {
                for declaration in required {
                    resources::push(&mut records, declaration, meter)?;
                }
            }
        }
        for (id, constructor) in export.class_constructors.iter() {
            work(meter, roots.values().len())?;
            if constructor.access.declared != DeclaredVisibility::Protected
                || constructor.identity_kind != ClassConstructorIdentityKind::Source
                || !selected(
                    export.nominal_identities[constructor.owner].source(),
                    &roots,
                )
            {
                continue;
            }
            let source = export.constructor_identities[id]
                .source_record()
                .ok_or_else(|| invalid("protected source constructor has no sealed identity"))?;
            resources::push(
                &mut records,
                ProtectedDeclarationRefV1::Constructor(source.id()),
                meter,
            )?;
        }
        for (id, constructor) in export.struct_constructors.iter() {
            work(meter, roots.values().len())?;
            if constructor.access.declared == DeclaredVisibility::Protected
                && selected(
                    export.nominal_identities[constructor.owner].source(),
                    &roots,
                )
            {
                resources::push(
                    &mut records,
                    ProtectedDeclarationRefV1::Constructor(export.constructor_identities[id].id()),
                    meter,
                )?;
            }
        }
        resources::canonical(records.len(), meter)?;
        Self::try_new(records).map_err(invalid)
    }
}
fn selected(
    source: Option<&HirSourceNominalIdentity>,
    roots: &CanonicalSourceNominalIdsV1,
) -> bool {
    let owner = match source {
        Some(HirSourceNominalIdentity::Concrete(record)) => SourceNominalId::Concrete(record.id()),
        Some(HirSourceNominalIdentity::Generic(record)) => {
            SourceNominalId::GenericTemplate(record.id())
        }
        None => return false,
    };
    roots.values().binary_search(&owner).is_ok()
}
