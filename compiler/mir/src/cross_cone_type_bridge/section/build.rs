use super::view::LocalView;
use super::*;

impl<'a> CrossConeMirTypeBridgeSectionV1<'a> {
    pub fn try_new(
        local: MirTypeBridgeLocalInputV1<'_>,
        exports: MirTypeBridgeExportConstituentsV1,
        units: Vec<MirTypeBridgeInitializationUnitV1>,
        dependencies: &[MirTypeBridgeDependencyViewV1<'a>],
        committed: &[MirTypeBridgeDependencyV1],
        graph: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeSectionError> {
        local.validate()?;
        let provider = local.provider();
        if exports.shapes().provider() != provider {
            return Err(MirTypeBridgeSectionError::ProviderContext);
        }
        if units
            .windows(2)
            .any(|pair| pair[0].unit() >= pair[1].unit())
        {
            return Err(MirTypeBridgeSectionError::NonCanonicalUnitInventory);
        }
        let dependencies = dependencies::complete(provider, dependencies)?;
        let types = dependencies::types(exports.types(), &dependencies)?;
        let entries = closure::close(
            LocalView {
                provider,
                exports: &exports,
                units: &units,
                direct: local.ordinary,
            },
            &dependencies,
            committed,
            graph,
            &types,
        )?;
        let selected = SelectedDependencyMirTypeSetV1::from_closed(provider, entries);
        Ok(Self {
            provider,
            exports,
            units,
            direct_callables: local.ordinary.clone(),
            selected,
        })
    }
}

pub(super) fn validate_selected_records(
    provider: ConeIdentity,
    records: &[MirTypeBridgeDependencyV1],
) -> Result<(), MirTypeBridgeSectionError> {
    if let Some(index) = records.windows(2).position(|pair| pair[0] >= pair[1]) {
        return Err(MirTypeBridgeSectionError::NonCanonicalSelected { index: index + 1 });
    }
    if records.iter().any(|record| record.provider() == provider) {
        return Err(MirTypeBridgeSectionError::SelectedCurrentProvider);
    }
    Ok(())
}
