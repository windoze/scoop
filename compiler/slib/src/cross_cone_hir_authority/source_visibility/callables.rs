use super::*;
use crate::cross_cone_hir_authority::{
    CrossConeHirDefaultCallableAccessError as CallableError,
    CrossConeHirDefaultTypeAccessError as TypeError,
};
use scoop_hir::SourceAccessDomainV1;
use scoop_identity::CoreBuiltinNominal;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(in crate::cross_cone_hir_authority) fn source_equality_access_domain(
        &mut self,
        owner: &SignatureTypeKey,
        protocols: &CoreBootstrapInterfaceSectionV1,
        path: &WirePath,
    ) -> Result<SourceAccessDomainV1, CallableError> {
        let unit = CoreBuiltinNominal::Unit.identity_record();
        let bytes = scoop_wire::encoded_length(unit.key())
            .map_err(|error| CallableError::Encoding(error.to_string()))?;
        self.meter.charge_sha256(bytes, path)?;
        self.meter.charge_work(bytes.saturating_add(65), path)?;
        let (declaration, arity) = match owner {
            SignatureTypeKey::Nominal(id) if *id == unit.id() => {
                return self
                    .source_type_access_domain(owner, protocols, path)
                    .map_err(Into::into);
            }
            SignatureTypeKey::Tuple(_) => {
                return self
                    .source_type_access_domain(owner, protocols, path)
                    .map_err(Into::into);
            }
            SignatureTypeKey::Nominal(id) => (SourceNominalId::Concrete(*id), 0),
            SignatureTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                arguments.as_slice().len(),
            ),
            _ => return Err(CallableError::EqualityShape),
        };
        let (record, key) = self.visibility_nominal(declaration)?;
        if !matches!(
            key.declaration_kind(),
            SourceDeclarationKind::Struct | SourceDeclarationKind::Enum
        ) {
            return Err(CallableError::EqualityKind(declaration));
        }
        let expected = record.type_parameters().len_u32();
        if usize::try_from(expected).ok() != Some(arity) {
            return Err(TypeError::Arity {
                declaration,
                expected,
                actual: arity,
            }
            .into());
        }
        let constraints = self.visibility_domain(
            &key,
            nominal_subject(declaration),
            record.declaration_details().declared_visibility(),
        )?;
        SourceAccessDomainV1::from_constraints(constraints, self.meter, path).map_err(Into::into)
    }
}
