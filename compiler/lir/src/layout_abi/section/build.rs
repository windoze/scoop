use super::*;

pub(super) enum SelectionInput {
    Producer,
    Reader(Vec<LayoutAbiDependencyV1>),
}

pub(super) fn producer<'a, E>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
    physical_imports: Vec<crate::ExternalShapeLinkImportV1<'a>>,
    source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
    meter: &mut BudgetMeter,
) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
    let dependencies = dependencies::complete(
        exports.provider(),
        exports.target_profile(),
        dependencies,
        meter,
    )?;
    let physical_imports =
        crate::CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports, meter)?;
    complete(
        exports,
        dependencies,
        physical_imports,
        SelectionInput::Producer,
        source,
        meter,
    )
}

pub(super) fn complete<'a, E>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: Vec<&'a CrossConeLayoutAbiSectionV1<'a>>,
    physical_imports: crate::CanonicalExternalShapeLinkImportsV1<'a>,
    selection: SelectionInput,
    source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
    meter: &mut BudgetMeter,
) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
    source
        .validate_local_exports(&exports, meter)
        .map_err(LayoutAbiSectionError::Source)?;
    dispatch_inventory::validate(&exports, meter)?;
    source
        .validate_physical_imports(physical_imports.records(), meter)
        .map_err(LayoutAbiSectionError::Source)?;
    validate_selection_input(exports.provider(), &selection, meter)?;
    let roots = source
        .committed_semantic_roots()
        .map_err(LayoutAbiSectionError::Source)?;
    let mut dependency_exports = reserve(dependencies.len(), meter)?;
    dependency_exports.extend(dependencies.iter().map(|section| &section.exports));
    let relations = semantic_closure::close(
        exports.provider(),
        &exports,
        &dependency_exports,
        roots,
        meter,
    )?;
    if let SelectionInput::Reader(expected) = selection {
        meter.charge_work(expected.len() as u64, &WirePath::root())?;
        if expected != relations {
            return Err(LayoutAbiSectionError::SelectedClosure);
        }
    }
    let mut semantic = reserve(relations.len(), meter)?;
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
        meter,
    )?;
    let selected = SelectedDependencyLayoutAbiSetV1::from_closed(
        exports.provider(),
        exports.target_profile(),
        semantic,
        physical_imports,
    )?;
    Ok(CrossConeLayoutAbiSectionV1 {
        exports,
        dependencies,
        selected,
    })
}

fn validate_selection_input<E>(
    consumer: ConeIdentity,
    input: &SelectionInput,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSectionError<E>> {
    if let SelectionInput::Reader(relations) = input {
        meter.check_table_entries(relations.len() as u64, &WirePath::root())?;
        meter.charge_work(relations.len() as u64, &WirePath::root())?;
        if let Some(index) = relations.windows(2).position(|pair| pair[0] >= pair[1]) {
            return Err(LayoutAbiSectionError::NonCanonicalSelected { index: index + 1 });
        }
        if relations
            .iter()
            .any(|relation| relation.provider() == consumer)
        {
            return Err(LayoutAbiSectionError::SelectedCurrentProvider);
        }
    }
    Ok(())
}

fn validate_physical<E>(
    consumer: ConeIdentity,
    dependencies: &[&CrossConeLayoutAbiSectionV1<'_>],
    semantic: &[SelectedLayoutAbiEntryV1<'_>],
    physical: &crate::CanonicalExternalShapeLinkImportsV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSectionError<E>> {
    meter.charge_work(physical.records().len() as u64, &WirePath::root())?;
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
            meter,
        )?;
        let Some(target) = semantic_target(import.subject(), terminal)
            .map_err(LayoutAbiSectionError::MissingPhysicalSubject)?
        else {
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

fn semantic_target(
    subject: crate::ExternalStrongShapeSubjectV1,
    terminal: &CrossConeLayoutAbiSectionV1<'_>,
) -> Result<Option<LayoutAbiSemanticTargetV1>, crate::ExternalStrongShapeSubjectV1> {
    use crate::ExternalStrongShapeSubjectV1 as Subject;
    Ok(match subject {
        Subject::Callable(target) => Some(LayoutAbiSemanticTargetV1::Callable(target)),
        Subject::Layout(layout) => Some(LayoutAbiSemanticTargetV1::Layout(layout)),
        Subject::Scan(scan) => Some(
            terminal
                .layouts()
                .records()
                .iter()
                .find(|record| record.scan() == scan)
                .map(|record| LayoutAbiSemanticTargetV1::Layout(record.identity().layout()))
                .ok_or(subject)?,
        ),
        Subject::TypeDescriptor(exact) | Subject::TypeRegistration(exact) => {
            Some(LayoutAbiSemanticTargetV1::Descriptor(exact))
        }
        Subject::DispatchTable(table) => Some(LayoutAbiSemanticTargetV1::Dispatch(table)),
        Subject::StaticStorage(_)
        | Subject::StaticStorageRegistration(_)
        | Subject::InitializationCell(_)
        | Subject::InitializationDescriptor(_) => None,
    })
}
