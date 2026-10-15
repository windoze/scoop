use super::*;
use crate::link_data::link_error;
use crate::*;
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedStrongStaticStorageRegistrationPlanV1 {
    pub(in crate::production::registration_production) fn read(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        layouts: &[&CanonicalExactLayoutExportsV1],
    ) -> Result<StrongStaticStorageSemanticPlanV1, LinkDataError> {
        let data = self.semantic;
        let layout_provider = identities
            .resolve(data.layout_provider)
            .map_err(link_error)?;
        let layout_id = identities.resolve(data.layout).map_err(link_error)?;
        let scan = identities.resolve(data.scan).map_err(link_error)?;
        let layout = if layout_provider == foundation.producer() {
            let layout = foundation
                .layouts()
                .iter()
                .find(|record| record.id() == layout_id)
                .ok_or_else(|| link_error(format!("missing storage layout {layout_id}")))?;
            let scan = foundation
                .scans()
                .iter()
                .find(|record| record.id() == scan)
                .ok_or_else(|| link_error(format!("missing storage scan {scan}")))?;
            if scan.key().layout() != layout.id() {
                return Err(link_error("storage scan references a different layout"));
            }
            LayoutIdentity::from_records(layout.clone(), scan.clone())
                .map(StaticStorageLayout::from)
                .map_err(link_error)?
        } else {
            let value = layouts
                .iter()
                .find(|exports| exports.provider() == layout_provider)
                .and_then(|exports| exports.get(layout_id))
                .and_then(|layout| layout.value_handle())
                .ok_or_else(|| {
                    link_error(format!("missing external storage layout {layout_id}"))
                })?;
            if value.scan() != scan {
                return Err(link_error("external storage scan differs from its layout"));
            }
            StaticStorageLayout::External(value)
        };
        let scan_kind = match data.scan_kind {
            0 => StaticStorageScanKindV1::None,
            1 => StaticStorageScanKindV1::Recursive,
            tag => return Err(link_error(format!("invalid storage scan kind {tag}"))),
        };
        let initial_state = match data.initial_state {
            DecodedStrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit => {
                StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit
            }
            DecodedStrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                initial_template,
                immortal_relocations,
            } => StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                initial_template,
                immortal_relocations: immortal_relocations
                    .into_iter()
                    .map(|relocation| {
                        Ok(StaticImmortalRelocationPlanV1::from_artifact(
                            relocation.pointer_offset,
                            identities.resolve(relocation.target).map_err(link_error)?,
                        ))
                    })
                    .collect::<Result<_, LinkDataError>>()?,
            },
        };
        let semantic = StrongStaticStorageSemanticPlanV1::from_artifact(
            identities.resolve(data.storage).map_err(link_error)?,
            data.storage_symbol
                .resolve(identities)
                .map_err(link_error)?,
            layout,
            layout_provider,
            data.scan_program.read()?,
            scan_kind,
            data.byte_size,
            data.allocation_extent,
            data.required_alignment,
            initial_state,
        );
        Ok(semantic)
    }
}
