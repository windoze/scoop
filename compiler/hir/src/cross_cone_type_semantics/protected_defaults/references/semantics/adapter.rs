use super::*;
use crate::{
    CheckedProtectedDefaultWitnessSourceV1, ProtectedDefaultAccessWitnessV1,
    ProtectedDefaultReferenceBodySemanticAuthority, ProtectedDefaultTemplateKeyV1,
};

pub(super) struct Adapter<'t, 's, 'g, 'a, 'i, 'v, A> {
    pub template: &'t ProtectedDefaultTemplateV1,
    pub owner: CheckedProtectedDefaultOwnerProfileV1<'s>,
    pub graph: &'g CheckedNominalInheritanceGraphV1<'a>,
    pub inheritance: CheckedNominalInheritanceInterfacesV1<'i>,
    pub authority: &'v mut A,
}
impl<A: ProtectedDefaultReferenceAccessSemanticAuthority<E>, E>
    ProtectedDefaultReferenceBodySemanticAuthority<ProtectedDefaultReferenceAccessError<E>>
    for Adapter<'_, '_, '_, '_, '_, '_, A>
{
    fn validate_default_reference_occurrence(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        witness: &ProtectedDefaultAccessWitnessV1,
        receiver: ProtectedDefaultReferenceReceiverV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ProtectedDefaultReferenceAccessError<E>> {
        use ProtectedDefaultReferenceAccessError as Error;
        if key != self.owner.key() {
            return Err(Error::TemplateKey);
        }
        meter.charge_nodes(1, path).map_err(Error::Resource)?;
        let origin_size =
            scoop_wire::encoded_length(occurrence.definition_origin).map_err(Error::Encoding)?;
        meter
            .charge_work(origin_size, path)
            .map_err(Error::Resource)?;
        occurrence
            .definition_origin
            .validate_semantics(self.authority)
            .map_err(Error::Origin)?;
        let checked = self
            .owner
            .validate_witness(witness)
            .map_err(Error::Witness)?;
        let source_use = ProtectedDefaultReferenceSourceUseV1 {
            template: self.template,
            owner: self.owner,
            occurrence,
            receiver,
        };
        match checked {
            CheckedProtectedDefaultWitnessSourceV1::ParamFree(checked) => {
                let target = self
                    .authority
                    .replay_param_free_default_reference(source_use, self.graph, meter, path)
                    .map_err(Error::Source)?;
                checked
                    .validate_domains(self.graph, self.inheritance, &target, self.authority, meter)
                    .map_err(Error::Coverage)?;
            }
            CheckedProtectedDefaultWitnessSourceV1::GenericSourceMetadata(_) => {
                self.authority
                    .validate_generic_default_reference(source_use, meter, path)
                    .map_err(Error::Source)?;
            }
        }
        Ok(())
    }
}
