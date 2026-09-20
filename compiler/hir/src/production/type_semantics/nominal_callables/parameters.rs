use super::*;
use scoop_identity::SignatureTypeKey;

impl Projection<'_, '_> {
    pub(super) fn parameters(
        &mut self,
        owner: ExportParameterOwner,
        binders: &[HirSignatureBinder],
        expected: &[SignatureTypeKey],
    ) -> Result<CanonicalSourceParameterShapesV1, Error> {
        let path = WirePath::root();
        self.meter
            .charge_work(
                self.export.source_parameter_interfaces.len() as u64 * 2,
                &path,
            )
            .map_err(resource)?;
        for interface in &self.export.source_parameter_interfaces {
            if interface.owner != owner {
                continue;
            }
            self.meter
                .check_table_entries(interface.parameters.len() as u64, &path)
                .map_err(resource)?;
            self.meter
                .charge_collection_slots(interface.parameters.len() as u64 * 3, &path)
                .map_err(resource)?;
            for parameter in &interface.parameters {
                resources::name(&parameter.name, self.meter)?;
                let ty = match parameter.calling {
                    ExportParameterCalling::Required { value_type }
                    | ExportParameterCalling::Default { value_type, .. } => value_type,
                    ExportParameterCalling::Vararg { parameter_type, .. } => {
                        self.export.export_vararg_parameter_types[parameter_type].array_type
                    }
                };
                resources::ty(self.export, ty, binders.len(), 3, self.meter)?;
            }
        }
        callable_interfaces::source_parameter_shapes(
            self.export,
            &self.signatures,
            owner,
            binders,
            expected,
        )
        .map_err(invalid)
    }
}
