use super::*;
use scoop_hir::{IntrinsicTypeKind, NominalSourceShapeV1};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn pointer_source_owner(
        &mut self,
        current_protocols: &CoreBootstrapInterfaceSectionV1,
        function: bool,
    ) -> Result<SourceNominalId, TypeError> {
        let mut selected = None;
        let providers = std::iter::once((self.current, current_protocols)).chain(
            self.dependencies
                .iter()
                .map(|provider| (provider.identity, provider.core)),
        );
        for (provider, section) in providers {
            let Some(definitions) = section.compiler_protocol_definitions() else {
                continue;
            };
            let protocols = definitions.compiler_protocols();
            let declaration = if function {
                protocols.function_pointer_source_type()
            } else {
                protocols.pointer_source_type()
            };
            if let Some((first, previous)) = selected
                && previous != declaration
            {
                return Err(TypeError::ConflictingPointerProtocols {
                    first,
                    second: provider,
                });
            }
            selected = Some((provider, declaration));
        }
        let (_, declaration) = selected.ok_or(TypeError::MissingPointerProtocol)?;
        let declaration = SourceNominalId::GenericTemplate(declaration);
        let (record, _) = self.visibility_nominal(declaration)?;
        let expected = if function {
            IntrinsicTypeKind::FunPtr
        } else {
            IntrinsicTypeKind::Ptr
        };
        if !matches!(record.source_shape(), NominalSourceShapeV1::Intrinsic(shape) if shape.family() == expected)
        {
            return Err(TypeError::PointerRepresentation {
                declaration,
                expected,
            });
        }
        Ok(declaration)
    }
}
