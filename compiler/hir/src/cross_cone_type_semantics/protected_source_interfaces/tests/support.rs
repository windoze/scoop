use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::nominal;
use scoop_identity::{
    CanonicalIdentifier, NonEmptyVec, PersistentGenericTypeId, SourceDeclarationKey,
    SourceNominalKind,
};

pub(super) struct ProtocolAuthority {
    pub owner: CallableTemplateOrigin,
    pub shapes: Vec<SourceParameterShapeV1>,
    pub calling: Vec<ProtectedParameterCallingKindV1>,
    pub origin: ExportDefinitionSourceV1,
    pub array: PersistentGenericTypeId,
}
impl ProtectedSourceProtocolSemanticAuthority<&'static str> for ProtocolAuthority {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, &'static str> {
        Ok(self.array)
    }
    fn source_parameter_shape(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&SourceParameterShapeV1, &'static str> {
        if owner != self.owner {
            return Err("wrong owner");
        }
        self.shapes.get(position as usize).ok_or("wrong parameter")
    }
    fn source_parameter_calling_kind(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, &'static str> {
        if owner != self.owner {
            return Err("wrong owner");
        }
        self.calling
            .get(position as usize)
            .copied()
            .ok_or("wrong parameter")
    }
    fn validate_source_parameter_origin(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if owner != self.owner || position as usize >= self.shapes.len() || origin != &self.origin {
            return Err("wrong parameter origin");
        }
        Ok(())
    }
}

pub(super) fn fixture(
    vararg_default: bool,
) -> (
    Fixture,
    ProtectedCallableInterfaceV1,
    ProtectedCallableSourceInterfaceV1,
    ProtectedDefaultKeyIndexV1,
    ProtocolAuthority,
) {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let key = SourceDeclarationKey::nominal(
        site(&[]),
        CanonicalIdentifier::new("Array").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let array = PersistentGenericTypeId::from_source_declaration(&key).unwrap();
    fixture
        .graph
        .keys
        .insert(SourceNominalId::GenericTemplate(array), key);
    let unit = SignatureTypeKey::Nominal(nominal(fixture.unit));
    let array_type = SignatureTypeKey::NominalApplication {
        origin: array,
        arguments: NonEmptyVec::from_first(unit.clone(), []),
    };
    let types = vec![unit.clone(), unit.clone(), array_type];
    let declaration = fixture.function(owner, "method", false, types.clone());
    let payload = fixture.payload(owner, declaration, types, unit.clone());
    let shapes = payload.parameters().parameters().to_vec();
    let record = fixture.record(owner, declaration, payload);
    let origin = record.declaration_access().definition_origin().clone();
    let key = ProtectedDefaultTemplateKeyV1::try_new(declaration, 1).unwrap();
    let vararg_key = ProtectedDefaultTemplateKeyV1::try_new(declaration, 2).unwrap();
    let calling = vec![
        ProtectedParameterCallingV1::Required,
        ProtectedParameterCallingV1::Default { template: key },
        if vararg_default {
            ProtectedParameterCallingV1::VarargDefault {
                element_type: unit,
                template: vararg_key,
            }
        } else {
            ProtectedParameterCallingV1::VarargEmpty { element_type: unit }
        },
    ];
    let categories = calling
        .iter()
        .map(ProtectedParameterCallingV1::kind)
        .collect();
    let parameters = shapes
        .iter()
        .zip(calling)
        .map(|(shape, calling)| {
            ProtectedSourceParameterV1::new(
                shape.name().clone(),
                shape.value_type().clone(),
                calling,
                origin.clone(),
            )
        })
        .collect();
    let source = ProtectedCallableSourceInterfaceV1::try_new(
        declaration,
        CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
    )
    .unwrap();
    let keys = ProtectedDefaultKeyIndexV1::try_new(if vararg_default {
        vec![vararg_key, key]
    } else {
        vec![key]
    })
    .unwrap();
    let authority = ProtocolAuthority {
        owner: declaration,
        shapes,
        calling: categories,
        origin,
        array,
    };
    (fixture, record, source, keys, authority)
}
