//! Required callable support borrows its source origin from the shared foundation.

use super::*;
use scoop_hir::DeclaredVisibilityV1;
use scoop_identity::{DefinitionOriginSubject, SourceDeclarationKind};

type Error = CrossConeHirNominalAuthorityError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_support_callable_origins(&mut self) -> Result<(), Error> {
        for record in self
            .current_interface
            .callable_interfaces()
            .all_declarations()
        {
            if self
                .current_interface
                .callable_interfaces()
                .get(record.declaration())
                .is_some()
                && !matches!(record.declaration(), CallableTemplateOrigin::Accessor(_))
            {
                continue;
            }
            let declaration = record.declaration();
            let (key, subject) = self.support_callable_origin(declaration)?;
            let owner = match record.owner() {
                PublicDeclarationOwnerV1::Nominal(owner) => Some(owner),
                PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => None,
            };
            let origin_owner =
                if matches!(declaration, CallableTemplateOrigin::VariantConstructor(_)) {
                    self.source_key_owner("variant enum", &key)?
                } else {
                    record.owner()
                };
            self.validate_declaration_origin(
                &key,
                subject,
                origin_owner,
                record.declared_visibility(),
            )?;
            let owner_kind = owner
                .map(|owner| {
                    self.source_nominal_key(owner)
                        .map(|key| key.declaration_kind())
                })
                .transpose()?;
            let expected_kind = match declaration {
                CallableTemplateOrigin::Constructor(_) => matches!(
                    owner_kind,
                    Some(SourceDeclarationKind::Struct | SourceDeclarationKind::Class)
                ),
                CallableTemplateOrigin::VariantConstructor(_) => {
                    owner_kind == Some(SourceDeclarationKind::Enum)
                }
                CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_) => true,
                CallableTemplateOrigin::Accessor(_) => true,
            };
            if !expected_kind {
                return Err(invalid(
                    declaration,
                    "source declaration has an incompatible owner kind",
                ));
            }
            if matches!(declaration, CallableTemplateOrigin::VariantConstructor(_))
                && record.declared_visibility() != DeclaredVisibilityV1::Public
            {
                return Err(invalid(
                    declaration,
                    "variant visibility must follow its enum owner",
                ));
            }
        }
        Ok(())
    }

    fn support_callable_origin(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<(SourceDeclarationKey, DefinitionOriginSubject), Error> {
        let subject = match declaration {
            CallableTemplateOrigin::Function(id) => DefinitionOriginSubject::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                DefinitionOriginSubject::GenericFunction(id)
            }
            CallableTemplateOrigin::Constructor(id) => DefinitionOriginSubject::Constructor(id),
            CallableTemplateOrigin::VariantConstructor(variant) => {
                let key = self
                    .identities
                    .canonical_key::<_, EnumVariantIdentityKey>(variant)
                    .map_err(Error::Identity)?;
                let owner = key
                    .source_owner()
                    .ok_or(Error::GeneratedEnumVariant { variant })?;
                return Ok((
                    self.source_nominal_key(owner)?,
                    DefinitionOriginSubject::EnumVariant(variant),
                ));
            }
            CallableTemplateOrigin::Accessor(id) => {
                let accessor = self
                    .identities
                    .canonical_key::<_, PropertyAccessorKey>(id)
                    .map_err(Error::Identity)?;
                let property = accessor.owner();
                // Foundation validation already ties the accessor source to this property.
                let subject = DefinitionOriginSubject::PropertyAccessor(id);
                return Ok((self.property_key(property)?, subject));
            }
        };
        Ok((self.function_key(declaration)?, subject))
    }
}

fn invalid(declaration: CallableTemplateOrigin, reason: &'static str) -> Error {
    Error::CallableDeclaration {
        declaration,
        reason,
    }
}
