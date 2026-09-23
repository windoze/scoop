//! Provider parameter contracts reconstructed from the shared declaration tables.

use scoop_hir::{DefaultTemplateContractViewV1, ExportDefaultTemplateV1};
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod declarations;
mod errors;
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
        let mut shapes = DefaultNominalShapes::new(providers, self.meter, &path)?;
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
        shapes: &mut DefaultNominalShapes,
        path: &WirePath,
    ) -> Result<(), Error> {
        let provider = template.definition_origin().origin().source().cone();
        let interface = self
            .provider_interface(provider)
            .map_err(|error| Error::Provider(Box::new(error)))?;
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
        DefaultTemplateContractViewV1::from(template)
            .validate(&publisher, &original, shapes, self.meter, path)
            .map_err(|error| Error::Contract(Box::new(error)))
    }
}
