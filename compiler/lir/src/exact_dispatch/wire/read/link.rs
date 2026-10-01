use super::*;
use crate::{
    CallableAbiReceiverInputV1, CanonicalExactCallableAbiExportsV1, CanonicalExactLayoutExportsV1,
    ConeLirFoundation, CrossConeLirBridgeSectionV1, DispatchCallableAbiV1, ExactLayoutExportV1,
    LayoutAbiExportConstituentsV1, LinkDataError, LirTargetProfile, link_data::link_error,
};
use scoop_identity::{
    CallableDefinitionOwner, ConeIdentity, PersistentIdResolver, RepresentationRole,
    ValidatedIdentityGraph,
};

mod implementation;
mod provider;
use provider::AbiReader;

impl DecodedCanonicalExactDispatchExportsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn read_link(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        layouts: &CanonicalExactLayoutExportsV1,
        callables: &CanonicalExactCallableAbiExportsV1,
        ordinary: &CrossConeLirBridgeSectionV1,
        dependencies: &[&LayoutAbiExportConstituentsV1],
    ) -> Result<CanonicalExactDispatchExportsV1, LinkDataError> {
        let reader = AbiReader {
            target,
            layouts,
            callables,
            ordinary,
            dependencies,
        };
        let mut records = Vec::with_capacity(self.records.len());
        let mut previous = None;
        for raw in self.records {
            let table = identities.resolve(raw.semantic.table).map_err(link_error)?;
            if previous.is_some_and(|id| id >= table) {
                return Err(LinkDataError("noncanonical dispatch table order".into()));
            }
            previous = Some(table);
            let mut inputs = Vec::with_capacity(raw.semantic.entries.len());
            for entry in &raw.semantic.entries {
                let implementation = entry.implementation.read_link(identities)?;
                let exact = entry
                    .slot_signature
                    .exact
                    .clone()
                    .resolve(identities)
                    .map_err(link_error)?;
                let slot_receiver_layout = match implementation.receiver_adaptation() {
                    ExactDispatchReceiverAdaptationV1::Identity => None,
                    ExactDispatchReceiverAdaptationV1::ReferenceDispatch => {
                        Some(reader.layout(exact.receiver().into_option().ok_or_else(|| {
                            LinkDataError("dispatch adaptation has no receiver".into())
                        })?)?)
                    }
                };
                let provider = match entry.abi.resolve(identities).map_err(link_error)? {
                    crate::StrongTypeDispatchCallableRefV2::Local(_) => foundation.producer(),
                    crate::StrongTypeDispatchCallableRefV2::DependencyExternal {
                        provider, ..
                    } => provider,
                    crate::StrongTypeDispatchCallableRefV2::Runtime(_) => {
                        return Err(LinkDataError(
                            "exact dispatch requires a concrete callable body".into(),
                        ));
                    }
                };
                inputs.push(ExactDispatchEntryInputV1 {
                    position: ExactDispatchPositionV1::from_u32(entry.position),
                    slot: identities.resolve(entry.slot).map_err(link_error)?,
                    slot_signature: ExactDispatchSlotSignatureV1::new(
                        exact,
                        entry.slot_signature.gc_effect,
                    ),
                    implementation,
                    abi: reader.callable(provider, implementation.target())?,
                    slot_receiver_layout,
                });
            }
            let expected = ExactDispatchExportV1::replay_from_schema(
                target,
                &identities.canonical_record(table).map_err(link_error)?,
                &inputs,
                foundation,
            )
            .map_err(link_error)?;
            records.push(raw.validate_against(&expected).map_err(link_error)?);
        }
        CanonicalExactDispatchExportsV1::try_new(target, foundation, records).map_err(link_error)
    }
}
