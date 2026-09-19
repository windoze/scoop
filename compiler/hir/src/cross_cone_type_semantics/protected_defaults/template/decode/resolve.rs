use scoop_wire::{BudgetMeter, WirePath, encoded_length};

use super::super::{ProtectedDefaultTemplateResolutionError, ProtectedDefaultTemplateV1};
use super::DecodedProtectedDefaultTemplateV1;
use crate::{DefaultStatementReferenceResolver, ProtectedDefaultReferenceResolver};

impl DecodedProtectedDefaultTemplateV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedDefaultTemplateV1, ProtectedDefaultTemplateResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + ProtectedDefaultReferenceResolver<E>,
    {
        use ProtectedDefaultTemplateResolutionError as Error;
        let path = WirePath::root();
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let count = self.definition_path.segments().len() as u64;
        meter
            .charge_collection_slots(count, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(count, &path).map_err(Error::Resource)?;
        meter.charge_edges(count, &path).map_err(Error::Resource)?;
        meter.charge_work(count, &path).map_err(Error::Resource)?;
        let key = self.key.resolve(resolver).map_err(Error::Key)?;
        let definition_root = self
            .definition_root
            .resolve(resolver)
            .map_err(Error::DefinitionRoot)?;
        self.locals
            .charge_resolution(meter)
            .map_err(Error::Resource)?;
        let mut locals = self.locals.resolve(resolver).map_err(Error::Locals)?;
        self.body
            .charge_resolution(&locals, meter)
            .map_err(Error::Resource)?;
        let body = self
            .body
            .resolve(resolver, &mut locals)
            .map_err(Error::Body)?;
        self.result
            .charge_resolution(meter)
            .map_err(Error::Resource)?;
        let result = self.result.resolve(resolver).map_err(Error::Result)?;
        self.type_parameters
            .charge_resolution(meter)
            .map_err(Error::Resource)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(Error::TypeParameters)?;
        self.receiver
            .charge_resolution(&locals, meter)
            .map_err(Error::Resource)?;
        let receiver = self
            .receiver
            .resolve(resolver, &mut locals)
            .map_err(Error::Receiver)?;
        self.value_parameters
            .charge_resolution(&locals, meter)
            .map_err(Error::Resource)?;
        let value_parameters = self
            .value_parameters
            .resolve(&mut locals)
            .map_err(Error::ValueParameters)?;
        let references = self
            .references
            .resolve(resolver, meter)
            .map_err(Error::References)?;
        let origin_bytes = encoded_length(&self.definition_origin).map_err(Error::Encoding)?;
        meter
            .charge_owned_bytes(origin_bytes, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_work(origin_bytes, &path)
            .map_err(Error::Resource)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::DefinitionOrigin)?;
        ProtectedDefaultTemplateV1 {
            key,
            definition_root,
            definition_path: self.definition_path,
            locals,
            body,
            result,
            allows_suspend: self.allows_suspend,
            type_parameters,
            receiver,
            value_parameters,
            references,
            definition_origin,
        }
        .finish_metered(meter)
    }
}
