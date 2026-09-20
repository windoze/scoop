use scoop_wire::{BudgetMeter, WirePath};

use super::super::{DefaultSourceTemplateResolutionError, DefaultSourceTemplateV1};
use super::DecodedDefaultSourceTemplateV1;
use crate::{DefaultSourceReferenceResolver, DefaultStatementReferenceResolver};

impl DecodedDefaultSourceTemplateV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceTemplateV1, DefaultSourceTemplateResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + DefaultSourceReferenceResolver<E>,
    {
        use DefaultSourceTemplateResolutionError as Error;
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
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
        self.definition_origin
            .charge_resolution_at(meter, &path, 2)
            .map_err(Error::Resource)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::DefinitionOrigin)?;
        let template = DefaultSourceTemplateV1 {
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
        };
        template.validate_shape(meter).map_err(Error::Record)?;
        Ok(template)
    }
}
