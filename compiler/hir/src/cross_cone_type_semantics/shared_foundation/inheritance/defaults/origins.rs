use super::*;
use crate::{
    InheritanceCallableDeclarationV1, TypeDefinitionSourceSemanticAuthority,
    TypeDefinitionSourceUseV1,
};
use scoop_identity::CallableTemplateOrigin;

pub(super) struct Replay<'a, 'd> {
    pub metadata: SharedTypeMetadataV1<'a>,
    pub dependencies: &'d [CheckedSharedTypeFoundationV1<'a>],
}

impl TypeDefinitionSourceSemanticAuthority<Error> for Replay<'_, '_> {
    fn validate_type_definition_source_use(
        &mut self,
        source_use: TypeDefinitionSourceUseV1<'_>,
        source: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        let provider = metadata(
            self.metadata,
            self.dependencies,
            source.origin().source().cone(),
            meter,
        )?;
        provider
            .foundation
            .validate_definition_source_location(provider.provider, source, meter, path)
            .map_err(|error| match error {
                crate::DefinitionSourceLocationValidationError::Resource(error) => {
                    Error::Resource(error)
                }
                error => Error::DefinitionSourceLocation(error),
            })?;
        // The other source-bearing fields have just been joined to complete
        // shared declarations or the identical shared default body.
        let declaration = match source_use {
            TypeDefinitionSourceUseV1::SlotDeclaration { slot, .. } => Some(slot.declaration()),
            TypeDefinitionSourceUseV1::SlotImplementation { target, .. } => {
                Some(target.declaration())
            }
            _ => None,
        };
        if let Some(declaration) = declaration {
            let declaration = match declaration {
                InheritanceCallableDeclarationV1::Function(id) => {
                    CallableTemplateOrigin::Function(id)
                }
                InheritanceCallableDeclarationV1::Getter(id)
                | InheritanceCallableDeclarationV1::Setter(id) => {
                    CallableTemplateOrigin::Accessor(id)
                }
            };
            let record = contracts::callable(provider, declaration, meter)?;
            let access = contracts::callable_access(provider, record, meter)?;
            contracts::charge_compare(source, access.definition_origin(), meter)?;
            if source != access.definition_origin() {
                return Err(Error::CallableContract(declaration));
            }
        }
        Ok(())
    }
}
