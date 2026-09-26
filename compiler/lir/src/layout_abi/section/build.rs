use super::*;

pub(super) fn producer<'a>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
    physical_imports: Vec<crate::ExternalShapeLinkImportV1>,
    roots: &[LayoutAbiDependencyV1],
) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError> {
    let dependencies =
        dependencies::complete(exports.provider(), exports.target_profile(), dependencies)?;
    let physical_imports =
        crate::CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports)?;
    complete(exports, dependencies, physical_imports, roots)
}

pub(super) fn complete<'a>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: Vec<&'a LayoutAbiExportConstituentsV1>,
    physical_imports: crate::CanonicalExternalShapeLinkImportsV1,
    roots: &[LayoutAbiDependencyV1],
) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError> {
    dispatch_inventory::validate(&exports)?;
    let relations = close_selection(&exports, &dependencies, roots, None)?;
    let mut semantic = reserve(relations.len())?;
    for relation in relations {
        let terminal = dependencies
            .binary_search_by_key(&relation.provider(), |section| section.provider())
            .ok()
            .map(|index| dependencies[index])
            .ok_or(LayoutAbiSectionError::MissingSemanticProvider(
                relation.provider(),
            ))?;
        semantic.push(SelectedLayoutAbiEntryV1 { relation, terminal });
    }
    validate_physical(
        exports.provider(),
        &dependencies,
        &semantic,
        &physical_imports,
    )?;
    let selected = SelectedDependencyLayoutAbiSetV1::from_closed(
        exports.provider(),
        exports.target_profile(),
        semantic,
        physical_imports,
    )?;
    Ok(CrossConeLayoutAbiSectionV1 { exports, selected })
}

pub(super) fn close_selection(
    exports: &LayoutAbiExportConstituentsV1,
    dependencies: &[&LayoutAbiExportConstituentsV1],
    committed: &[LayoutAbiDependencyV1],
    candidate: Option<&[LayoutAbiDependencyV1]>,
) -> Result<Vec<LayoutAbiDependencyV1>, LayoutAbiSectionError> {
    let relations = semantic_closure::close(exports.provider(), exports, dependencies, committed)?;
    if let Some(expected) = candidate {
        if expected != relations {
            return Err(LayoutAbiSectionError::SelectedClosure);
        }
    }
    Ok(relations)
}

pub(super) fn validate_selected_records(
    consumer: ConeIdentity,
    relations: &[LayoutAbiDependencyV1],
) -> Result<(), LayoutAbiSectionError> {
    if let Some(index) = relations.windows(2).position(|pair| pair[0] >= pair[1]) {
        return Err(LayoutAbiSectionError::NonCanonicalSelected { index: index + 1 });
    }
    if relations
        .iter()
        .any(|relation| relation.provider() == consumer)
    {
        return Err(LayoutAbiSectionError::SelectedCurrentProvider);
    }
    Ok(())
}

fn validate_physical(
    consumer: ConeIdentity,
    dependencies: &[&LayoutAbiExportConstituentsV1],
    semantic: &[SelectedLayoutAbiEntryV1<'_>],
    physical: &crate::CanonicalExternalShapeLinkImportsV1,
) -> Result<(), LayoutAbiSectionError> {
    for import in physical.records() {
        if import.provider() == consumer {
            return Err(LayoutAbiSectionError::SelectedCurrentProvider);
        }
        let terminal = dependencies
            .binary_search_by_key(&import.provider(), |section| section.provider())
            .ok()
            .map(|index| dependencies[index])
            .ok_or(LayoutAbiSectionError::MissingPhysicalProvider(
                import.provider(),
            ))?;
        import.validate_semantic_against(
            terminal.layouts(),
            terminal.callables(),
            terminal.descriptors(),
            terminal.dispatch(),
        )?;
        let Some(target) = import.semantic_target(terminal.layouts())? else {
            continue;
        };
        let relation = LayoutAbiDependencyV1::new(import.provider(), target);
        if semantic
            .binary_search_by_key(&relation, |entry| entry.relation)
            .is_err()
        {
            return Err(LayoutAbiSectionError::MissingPhysicalSemantic(relation));
        }
    }
    Ok(())
}
