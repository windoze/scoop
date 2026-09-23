//! Provider parameter contracts reconstructed from the shared declaration tables.

use scoop_hir::{DefaultTemplateContractViewV1, ExportDefaultTemplateV1};
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod declarations;
mod errors;
mod local_signatures;
mod shapes;
use declarations::ParameterSelection;
pub use errors::CrossConeHirDefaultProviderContractError;
pub use shapes::DefaultMetadataNominalError;
use shapes::DefaultNominalShapes;
type Error = CrossConeHirDefaultProviderContractError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_provider_contracts(&mut self) -> Result<(), Error> {
        let templates = self.current_interface.default_templates().records();
        if templates.is_empty() {
            return Ok(());
        }
        let path = WirePath::root().field(7);
        let providers = std::iter::once((self.current, self.current_interface)).chain(
            self.dependencies
                .iter()
                .map(|provider| (provider.identity, provider.interface)),
        );
        let mut shapes = DefaultNominalShapes::new(self.identities, providers, self.meter, &path)?;
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        for (index, template) in templates.iter().enumerate() {
            self.validate_default_contract(
                template,
                &mut shapes,
                &path.clone().index(index as u64),
            )
            .map_err(|source| Error::Template {
                index,
                key: template.key(),
                source: Box::new(source),
            })?;
        }
        Ok(())
    }

    fn validate_default_contract(
        &mut self,
        template: &ExportDefaultTemplateV1,
        shapes: &mut DefaultNominalShapes<'_>,
        path: &WirePath,
    ) -> Result<(), Error> {
        let provider = template.definition_origin().origin().source().cone();
        let interface = self
            .provider_interface(provider)
            .map_err(|error| Error::Provider(Box::new(error)))?;
        self.meter.charge_work(
            u64::from(
                self.current_interface
                    .callable_interfaces()
                    .records()
                    .len()
                    .max(1)
                    .ilog2(),
            ) + 1,
            path,
        )?;
        if let Some(public) = self
            .current_interface
            .callable_interfaces()
            .get(template.key().owner())
        {
            template
                .references()
                .validate_public_access(public, self.meter, path)
                .map_err(|error| Error::PublicWitness(Box::new(error)))?;
        }
        let publisher = declarations::contract(
            self.current_interface,
            template.key().owner(),
            ParameterSelection::Position(template.key().parameter_position()),
            shapes,
            self.meter,
            path,
        )?;
        let original = declarations::contract(
            interface,
            template.definition_root().declaration(),
            ParameterSelection::from_path(template.definition_path())?,
            shapes,
            self.meter,
            path,
        )?;
        let view = DefaultTemplateContractViewV1::from(template);
        view.validate(&publisher, &original, shapes, self.meter, path)
            .map_err(|error| Error::Contract(Box::new(error)))?;
        view.validate_provider_types(original.shape(), shapes, self.meter, path)
            .map_err(|error| Error::Envelope(Box::new(error)))?;
        template
            .validate_source_reference_closure(self.meter, path)
            .map_err(|error| Error::ReferenceClosure(Box::new(error)))
    }
}
