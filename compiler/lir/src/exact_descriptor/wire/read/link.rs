use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, CanonicalExactLayoutExportsV1, ConeLirFoundation,
    ConeProductionSectionV2, ExactDescriptorExportV1, LinkDataError, LirTargetProfile,
    link_data::link_error,
};
use scoop_identity::ExactTypeDiagnosticGraph;

impl DecodedCanonicalExactDescriptorExportsV1 {
    pub fn read_link(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        layouts: &CanonicalExactLayoutExportsV1,
        production: &ConeProductionSectionV2,
        diagnostics: &impl ExactTypeDiagnosticGraph,
    ) -> Result<CanonicalExactDescriptorExportsV1, LinkDataError> {
        let mut records = Vec::with_capacity(self.records.len());
        let mut previous = None;
        for raw in self.records {
            let registration = production
                .type_registrations()
                .registrations()
                .iter()
                .find(|record| record.exact_type().as_array() == raw.semantic.exact.as_array())
                .ok_or_else(|| LinkDataError("missing descriptor registration".into()))?;
            let exact = registration.exact_type();
            if previous.is_some_and(|id| id >= exact) {
                return Err(LinkDataError("noncanonical descriptor order".into()));
            }
            previous = Some(exact);
            let expected = ExactDescriptorExportV1::replay(
                target,
                layouts,
                registration,
                diagnostics,
                foundation,
            )
            .map_err(link_error)?;
            records.push(raw.validate_against(&expected).map_err(link_error)?);
        }
        CanonicalExactDescriptorExportsV1::try_new(target, foundation, records).map_err(link_error)
    }
}
