//! Canonical nominal lookup shared by signature and intrinsic type validation.

use super::*;

impl<'a> CanonicalCrossConeHirSurfaceAuthority<'a> {
    pub(super) fn source_nominal_key(
        &self,
        declaration: SourceNominalId,
    ) -> Result<SourceDeclarationKey, CrossConeHirNominalAuthorityError> {
        let key = match declaration {
            NominalDeclarationOwner::Concrete(id) => self
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id),
            NominalDeclarationOwner::GenericTemplate(id) => self
                .identities
                .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id),
        }
        .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        Ok(key.as_ref().clone())
    }

    pub(super) fn nominal_shape(
        &self,
        declaration: SourceNominalId,
    ) -> Result<PublicNominalShapeV1, CrossConeHirNominalAuthorityError> {
        let key = self.source_nominal_key(declaration)?;
        let origin = key.origin();
        let expected_kind =
            PublicNominalKindV1::try_from(key.declaration_kind()).map_err(|_| {
                CrossConeHirNominalAuthorityError::InvalidNominalDeclarationKind {
                    declaration,
                    actual: key.declaration_kind(),
                }
            })?;
        let expected_arity = key.duplicate_signature().type_parameter_count();
        let interface = self.provider_interface(origin)?;
        // Unit and Any are intrinsic language types without source declaration
        // arena entries. All source-defined nominals use the provider table.
        if let SourceNominalId::Concrete(id) = declaration
            && [
                scoop_identity::CoreBuiltinNominal::Unit,
                scoop_identity::CoreBuiltinNominal::Any,
            ]
            .iter()
            .any(|builtin| builtin.identity_record().id() == id)
        {
            return Ok(PublicNominalShapeV1::new(expected_kind, expected_arity));
        }
        let record = Self::checked_nominal_record(interface, declaration, &key)?;
        Ok(PublicNominalShapeV1::new(
            record.kind(),
            record.type_parameters().len_u32(),
        ))
    }

    pub(super) fn checked_nominal_record(
        interface: &'a CrossConeHirInterfaceSectionV1,
        declaration: SourceNominalId,
        key: &SourceDeclarationKey,
    ) -> Result<&'a scoop_hir::NominalInterfaceRecordV1, CrossConeHirNominalAuthorityError> {
        let expected_kind =
            PublicNominalKindV1::try_from(key.declaration_kind()).map_err(|_| {
                CrossConeHirNominalAuthorityError::InvalidNominalDeclarationKind {
                    declaration,
                    actual: key.declaration_kind(),
                }
            })?;
        let expected_arity = key.duplicate_signature().type_parameter_count();
        let record = interface
            .nominal_interfaces()
            .declaration(declaration)
            .ok_or(CrossConeHirNominalAuthorityError::MissingNominalInterface {
                origin: key.origin(),
                declaration,
            })?;
        if record.kind() != expected_kind {
            return Err(CrossConeHirNominalAuthorityError::NominalKindMismatch {
                declaration,
                expected: expected_kind,
                actual: record.kind(),
            });
        }
        let actual_arity = record.type_parameters().len_u32();
        if actual_arity != expected_arity {
            return Err(CrossConeHirNominalAuthorityError::NominalArityMismatch {
                declaration,
                expected: expected_arity,
                actual: actual_arity,
            });
        }
        Ok(record)
    }
}
