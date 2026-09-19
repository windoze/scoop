use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::{
    DispatchSlotKey, PersistentDispatchSlotId, SignatureTypeKey, SourceNominalKind,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};
mod variants;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
impl NominalSupportCallableSemanticAuthority<&'static str> for Fixture {
    fn source_enum_variant_key(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Result<&scoop_identity::EnumVariantIdentityKey, &'static str> {
        self.variants
            .get(&variant)
            .map(|value| &value.0)
            .ok_or("unknown variant")
    }
    fn source_enum_variant_shape(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Result<&EnumSourceVariantV1, &'static str> {
        self.variants
            .get(&variant)
            .map(|value| &value.1)
            .ok_or("unknown variant")
    }
    fn source_enum_variant_field_key(
        &self,
        field: scoop_identity::PersistentEnumVariantFieldId,
    ) -> Result<&scoop_identity::EnumVariantFieldKey, &'static str> {
        self.variant_fields.get(&field).ok_or("unknown field")
    }
    fn source_enum_variant_origin(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.graph
            .origins
            .get(
                &self
                    .variants
                    .get(&variant)
                    .ok_or("unknown variant")?
                    .0
                    .source_owner()
                    .ok_or("generated variant")?,
            )
            .ok_or("unknown owner")
    }
    fn source_nominal_modality(
        &self,
        owner: SourceNominalId,
    ) -> Result<NominalInheritanceModalityV1, &'static str> {
        if let Some(source) = self.nominal_sources.get(&owner) {
            return Ok(source.modality());
        }
        let exact = self.graph.exacts.iter().find_map(|(id, key)| matches!((key, owner), (scoop_identity::ExactTypeKey::Nominal(actual), SourceNominalId::Concrete(expected)) if *actual == expected).then_some(*id)).ok_or("unknown source modality")?;
        Ok(self.graph.records[&exact].modality())
    }
}

#[test]
fn nested_support_accepts_interface_default_without_widening_protected_callable() {
    let mut fixture = Fixture::default();
    let owner = fixture
        .graph
        .add("Protocol", SourceNominalKind::Interface, &[]);
    let declaration = fixture.function(owner, "method", false, vec![]);
    let CallableTemplateOrigin::Function(function) = declaration else {
        unreachable!()
    };
    let slot =
        PersistentDispatchSlotId::from_key(&DispatchSlotKey::interface_method(function)).unwrap();
    fixture.slots.push(slot);
    let mut source = fixture
        .payload(
            owner,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        )
        .source_signature;
    source.modality = CallableModalityV1::InterfaceDefault;
    source.slot_relations = CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    let record = NominalSupportCallableInterfaceV1::try_new(
        declaration,
        fixture.access(owner, DeclaredVisibilityV1::Public),
        source.clone(),
    )
    .unwrap();
    assert!(matches!(
        ProtectedCallablePayloadV1::from_source_signature(source),
        Err(ProtectedCallableInterfaceBuildError::Modality)
    ));
    let bytes = encode(&record).unwrap();
    let decoded: DecodedNominalSupportCallableInterfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let checked = record
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    assert_eq!(
        checked.declaration_access().source().declared_visibility(),
        DeclaredVisibilityV1::Public
    );
    assert_eq!(
        checked.payload().modality(),
        CallableModalityV1::InterfaceDefault
    );
}

#[test]
fn nested_support_preserves_private_interface_helpers_and_rejects_generic_dispatch() {
    let mut fixture = Fixture::default();
    let owner = fixture
        .graph
        .add("Protocol", SourceNominalKind::Interface, &[]);
    let declaration = fixture.function(owner, "helper", false, vec![]);
    let payload = fixture
        .payload(
            owner,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        )
        .source_signature;
    let record = NominalSupportCallableInterfaceV1::try_new(
        declaration,
        fixture.access(owner, DeclaredVisibilityV1::Private),
        payload,
    )
    .unwrap();
    let generic = fixture.function(owner, "generic", true, vec![]);
    let generic_payload = fixture
        .payload(
            owner,
            generic,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        )
        .source_signature;
    let generic_record = NominalSupportCallableInterfaceV1::try_new(
        generic,
        fixture.access(owner, DeclaredVisibilityV1::Private),
        generic_payload,
    )
    .unwrap();
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    record
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    assert!(matches!(
        generic_record.validate_source(&graph, &mut fixture, &mut meter()),
        Err(NominalSupportCallableSemanticError::Modality)
    ));
    let mut payload = record.payload().clone();
    payload.modality = CallableModalityV1::Abstract;
    assert!(matches!(
        NominalSupportCallableInterfaceV1::try_new(
            declaration,
            record.declaration_access().clone(),
            payload
        ),
        Err(ProtectedCallableInterfaceBuildError::Modality)
    ));
}

#[test]
fn nested_support_constructor_has_typed_owner_result_and_no_public_lookup_cast() {
    let mut fixture = Fixture::default();
    let owner = fixture.graph.add("Value", SourceNominalKind::Struct, &[]);
    let constructor = fixture.constructor(owner);
    let payload = fixture
        .payload(
            owner,
            CallableTemplateOrigin::Constructor(constructor),
            vec![],
            SignatureTypeKey::Nominal(nominal(owner)),
        )
        .source_signature;
    let record = NominalSupportConstructorInterfaceV1::try_new(
        constructor,
        fixture.access(owner, DeclaredVisibilityV1::Public),
        payload,
    )
    .unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedNominalSupportConstructorInterfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    record
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    let mut bad = record;
    bad.payload.result = SignatureTypeKey::Nominal(nominal(fixture.unit));
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture, &mut meter()),
        Err(NominalSupportCallableSemanticError::Signature(
            ProtectedCallableSemanticError::Result
        ))
    ));
}
