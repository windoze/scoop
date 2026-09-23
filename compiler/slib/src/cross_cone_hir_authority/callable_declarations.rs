//! Required callable support borrows its source origin from the shared foundation.

use super::*;
use scoop_hir::DeclaredVisibilityV1;
use scoop_identity::{DefinitionOriginSubject, SourceDeclarationKind};
use scoop_wire::WirePath;

type Error = CrossConeHirNominalAuthorityError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_support_callable_origins(&mut self) -> Result<(), Error> {
        for record in self
            .current_interface
            .callable_interfaces()
            .support_records()
        {
            let declaration = record.declaration();
            let (key, subject) = self.support_callable_origin(declaration)?;
            let origin = self
                .current_foundation
                .definition_origin(subject)
                .ok_or(Error::MissingDefinitionOrigin { subject })?;
            let path = WirePath::root().field(3).field(2);
            self.meter
                .charge_work(
                    key.owners().owners().len() as u64
                        + origin.origin().source().logical_path().as_str().len() as u64
                        + 1,
                    &path,
                )
                .map_err(Error::Resource)?;
            if key.origin() != self.current
                || origin.origin().source().cone() != self.current
                || key
                    .scope()
                    .source()
                    .is_some_and(|source| source != origin.origin().source())
            {
                return Err(invalid(
                    declaration,
                    "source origin differs from its typed declaration",
                ));
            }
            let PublicDeclarationOwnerV1::Nominal(owner) = record.owner() else {
                return Err(invalid(
                    declaration,
                    "necessary support requires a nominal owner",
                ));
            };
            let owner_key = self.source_nominal_key(owner)?;
            let expected_kind = match declaration {
                CallableTemplateOrigin::Constructor(_) => matches!(
                    owner_key.declaration_kind(),
                    SourceDeclarationKind::Struct | SourceDeclarationKind::Class
                ),
                CallableTemplateOrigin::VariantConstructor(_) => {
                    owner_key.declaration_kind() == SourceDeclarationKind::Enum
                }
                CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_) => true,
                CallableTemplateOrigin::Accessor(_) => false,
            };
            if !expected_kind {
                return Err(invalid(
                    declaration,
                    "source declaration has an incompatible owner kind",
                ));
            }
            if matches!(declaration, CallableTemplateOrigin::VariantConstructor(_)) {
                if record.declared_visibility() != DeclaredVisibilityV1::Public {
                    return Err(invalid(
                        declaration,
                        "variant visibility must follow its enum owner",
                    ));
                }
            } else if key.origin() != owner_key.origin()
                || key.package() != owner_key.package()
                || key
                    .owners()
                    .owners()
                    .split_last()
                    .map(|(_, parents)| parents)
                    != Some(owner_key.owners().owners())
            {
                return Err(invalid(
                    declaration,
                    "source callable has a different lexical owner chain",
                ));
            }
            if record.declared_visibility() == DeclaredVisibilityV1::Protected
                && owner_key.declaration_kind() != SourceDeclarationKind::Class
            {
                return Err(invalid(
                    declaration,
                    "protected callable requires a class owner",
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
            CallableTemplateOrigin::Accessor(_) => {
                return Err(invalid(
                    declaration,
                    "accessor is not a declared nominal function or constructor",
                ));
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
