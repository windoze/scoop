use super::*;
use scoop_wire::{WireError, WirePath};

impl ExportGenericNominalInitializationV1 {
    pub fn visit_definition_sources<V, E>(&self, visitor: &mut V, path: &WirePath) -> Result<(), E>
    where
        V: FnMut(&ExportDefinitionSourceV1) -> Result<(), E>,
        E: From<WireError>,
    {
        for constructor in self.constructors() {
            visitor(constructor.definition_origin())?;
            for input in constructor.inputs().records() {
                if let crate::TemplateLocalDefinitionV1::Source(origin) = input.definition() {
                    visitor(origin)?;
                }
            }
        }
        for fragment in self.fragments() {
            fragment.visit_definition_sources(visitor, path)?;
        }

        Ok(())
    }
    pub(crate) fn fragments(&self) -> impl Iterator<Item = &crate::ExportTemplateFragmentV1> {
        self.constructors()
            .iter()
            .flat_map(|constructor| {
                use ExportConstructorInitializationKindV1 as Kind;
                match constructor.kind() {
                    Kind::StructPrimary => [None, None],
                    Kind::StructSecondary { delegation, body }
                    | Kind::ClassSecondaryThis { delegation, body } => {
                        [Some(&delegation.arguments), Some(body)]
                    }
                    Kind::ClassPrimary { base, .. } => {
                        [base.as_ref().map(|base| &base.arguments), None]
                    }
                    Kind::ClassSecondaryTerminal { base, body } => {
                        [base.as_ref().map(|base| &base.arguments), Some(body)]
                    }
                }
                .into_iter()
                .flatten()
            })
            .chain(self.common().iter().map(|step| match step {
                ExportCommonInitializationStepV1::Field { value, .. } => value,
                ExportCommonInitializationStepV1::Body(body) => body,
            }))
    }
}
