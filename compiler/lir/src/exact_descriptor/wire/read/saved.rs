use scoop_identity::{
    CanonicalExactTypeDiagnosticName, PersistentIdResolver, ValidatedIdentityGraph,
};

use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, CanonicalExactLayoutExportsV1, ConeLirFoundation,
    ExactDescriptorAncestryV1, ExactDescriptorDispatchV1, ExactDescriptorExportV1,
    ExactDescriptorItableV1, ExternalStrongShapeSubjectV1, LinkDataError, LirTargetProfile,
    OptionalStrongTypeDescriptorRefV2, StrongShapeDefinitionRefV1, StrongTypeDescriptorRefV2,
    exact_descriptor::model::DescriptorBodyPartsV1, link_data::link_error,
};

impl DecodedExactDescriptorExportV1 {
    pub fn read(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        layouts: &CanonicalExactLayoutExportsV1,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactDescriptorExportV1, LinkDataError> {
        let raw = self.semantic;
        let exact = identities.resolve(raw.exact).map_err(link_error)?;
        let value = identities.resolve(raw.value_layout).map_err(link_error)?;
        let instance = identities
            .resolve(raw.instance_layout)
            .map_err(link_error)?;
        let value_layout = layouts
            .get(value)
            .and_then(crate::ExactLayoutExportV1::value_handle)
            .ok_or_else(|| link_error("missing descriptor value layout"))?;
        let instance_layout = layouts
            .get(instance)
            .and_then(crate::ExactLayoutExportV1::instance_handle)
            .ok_or_else(|| link_error("missing descriptor instance layout"))?;
        let parent = match raw
            .ancestry
            .parent
            .resolve(identities)
            .map_err(link_error)?
        {
            OptionalStrongTypeDescriptorRefV2::Absent => None,
            OptionalStrongTypeDescriptorRefV2::Local(exact) => {
                Some(StrongTypeDescriptorRefV2::Local(exact))
            }
            OptionalStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                Some(StrongTypeDescriptorRefV2::DependencyExternal { provider, exact })
            }
        };
        let ancestry = ExactDescriptorAncestryV1 {
            parent,
            interfaces: raw
                .ancestry
                .interfaces
                .into_iter()
                .map(|value| value.resolve(identities).map_err(link_error))
                .collect::<Result<_, _>>()?,
        };
        let dispatch = ExactDescriptorDispatchV1 {
            vtable: identities
                .resolve(raw.dispatch.vtable)
                .map_err(link_error)?,
            itables: raw
                .dispatch
                .itables
                .into_iter()
                .map(|value| {
                    Ok(ExactDescriptorItableV1 {
                        interface: value.interface.resolve(identities).map_err(link_error)?,
                        table: identities.resolve(value.table).map_err(link_error)?,
                    })
                })
                .collect::<Result<_, LinkDataError>>()?,
        };
        let definition = self.definition.read(identities)?;
        let registration = self.registration.read(identities)?;
        if definition.semantic_id() != exact || registration.semantic_id() != exact {
            return Err(link_error(
                "descriptor definition or registration subject mismatch",
            ));
        }
        let physical = StrongShapeDefinitionRefV1::from_saved(
            ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
            definition.definition_plan(),
            definition.symbol(),
            foundation,
        )?;
        Ok(ExactDescriptorExportV1::from_parts(DescriptorBodyPartsV1 {
            exact: identities.canonical_record(exact).map_err(link_error)?,
            value_layout,
            instance_layout,
            shape: raw.shape.read(target)?,
            object_scan: raw
                .object_scan
                .validate()
                .map_err(link_error)?
                .into_ref_scan(),
            ancestry,
            dispatch,
            diagnostic_name: CanonicalExactTypeDiagnosticName::from_saved(raw.diagnostic_name),
            physical,
            definition,
            registration,
        }))
    }
}

impl DecodedCanonicalExactDescriptorExportsV1 {
    pub fn read(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        layouts: &CanonicalExactLayoutExportsV1,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalExactDescriptorExportsV1, LinkDataError> {
        let records = self
            .records
            .into_iter()
            .map(|record| record.read(target, foundation, layouts, identities))
            .collect::<Result<Vec<_>, _>>()?;
        if records
            .windows(2)
            .any(|pair| pair[0].exact() >= pair[1].exact())
        {
            return Err(link_error("descriptor table is not in canonical order"));
        }
        Ok(CanonicalExactDescriptorExportsV1::from_saved(
            target,
            foundation.producer(),
            records,
        ))
    }
}
