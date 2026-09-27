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
            use ExportConstructorInitializationKindV1 as Kind;
            match constructor.kind() {
                Kind::StructPrimary => {}
                Kind::StructSecondary { delegation, body }
                | Kind::ClassSecondaryThis { delegation, body } => {
                    delegation
                        .arguments
                        .visit_definition_sources(visitor, path)?;
                    body.visit_definition_sources(visitor, path)?;
                }
                Kind::ClassPrimary { base, .. } => {
                    if let Some(base) = base {
                        base.arguments.visit_definition_sources(visitor, path)?;
                    }
                }
                Kind::ClassSecondaryTerminal { base, body } => {
                    if let Some(base) = base {
                        base.arguments.visit_definition_sources(visitor, path)?;
                    }
                    body.visit_definition_sources(visitor, path)?;
                }
            }
        }
        for step in self.common() {
            match step {
                ExportCommonInitializationStepV1::Field { value, .. } => {
                    value.visit_definition_sources(visitor, path)?
                }
                ExportCommonInitializationStepV1::Body(body) => {
                    body.visit_definition_sources(visitor, path)?
                }
            }
        }
        Ok(())
    }
}
