use super::view::LocalView;
use super::*;

pub(super) enum SelectionInput {
    Producer,
    Reader(Vec<MirTypeBridgeDependencyV1>),
}
pub(super) struct SectionInput<'a> {
    pub authority: MirTypeBridgeLocalAuthorityV1<'a>,
    pub exports: MirTypeBridgeExportConstituentsV1,
    pub dependencies: Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>>,
    pub selection: SelectionInput,
}

impl<'a> CrossConeMirTypeBridgeSectionV1<'a> {
    pub fn try_new<E>(
        authority: MirTypeBridgeLocalAuthorityV1<'a>,
        exports: MirTypeBridgeExportConstituentsV1,
        dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
        source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
        graph: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeSectionError<E>> {
        let dependencies = dependencies::complete(authority.provider(), dependencies, meter)?;
        complete(
            SectionInput {
                authority,
                exports,
                dependencies,
                selection: SelectionInput::Producer,
            },
            source,
            graph,
            meter,
        )
    }
}

pub(super) fn complete<'a, E>(
    input: SectionInput<'a>,
    source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
    graph: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError<E>> {
    let provider = input.authority.provider();
    input.authority.validate(meter)?;
    validate_selection_input(provider, &input.selection, meter)?;
    input
        .exports
        .validate_sources(provider, graph, source, meter)?;
    let types = dependencies::types(input.exports.types(), &input.dependencies, meter)?;
    replay::exports(
        input.authority,
        &input.exports,
        &input.dependencies,
        graph,
        &types,
        meter,
    )?;
    let units = units::build(input.authority, source, graph, &types, meter)?;
    let legacy_callables = input.authority.legacy_callables(meter)?;
    let local = LocalView {
        provider,
        exports: &input.exports,
        units: &units,
        legacy: &legacy_callables,
    };
    let committed = source
        .committed_external_uses()
        .map_err(MirTypeBridgeSectionError::Source)?;
    let entries = closure::close(local, &input.dependencies, committed, graph, &types, meter)?;
    if let SelectionInput::Reader(expected) = input.selection {
        meter.charge_work(expected.len() as u64, &WirePath::root())?;
        if !expected
            .into_iter()
            .eq(entries.iter().map(|entry| entry.relation))
        {
            return Err(MirTypeBridgeSectionError::SelectedClosure);
        }
    }
    let selected = SelectedDependencyMirTypeSetV1::from_closed(provider, entries)?;
    Ok(CrossConeMirTypeBridgeSectionV1 {
        authority: input.authority,
        exports: input.exports,
        units,
        legacy_callables,
        dependencies: input.dependencies,
        selected,
    })
}

fn validate_selection_input<E>(
    provider: ConeIdentity,
    selection: &SelectionInput,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSectionError<E>> {
    if let SelectionInput::Reader(records) = selection {
        meter.check_table_entries(records.len() as u64, &WirePath::root())?;
        meter.charge_work(records.len() as u64, &WirePath::root())?;
        if let Some(index) = records.windows(2).position(|pair| pair[0] >= pair[1]) {
            return Err(MirTypeBridgeSectionError::NonCanonicalSelected { index: index + 1 });
        }
        if records.iter().any(|record| record.provider() == provider) {
            return Err(MirTypeBridgeSectionError::SelectedCurrentProvider);
        }
    }
    Ok(())
}
