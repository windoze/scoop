use super::*;
use crate::*;

pub(super) fn owner(fixture: &ProviderFixture) -> Owner {
    match fixture.outer.unwrap() {
        SourceNominalId::Concrete(id) => Owner::Concrete(id),
        SourceNominalId::GenericTemplate(id) => Owner::GenericTemplate(id),
    }
}
pub(super) fn nominal(owner: Owner) -> SignatureTypeKey {
    match owner {
        Owner::Concrete(id) => SignatureTypeKey::Nominal(id),
        _ => panic!("concrete fixture reference"),
    }
}
pub(super) fn pointer(pointee: SignatureTypeKey) -> SignatureTypeKey {
    SignatureTypeKey::RawPointer(Box::new(pointee))
}
pub(super) fn policy() -> NominalCLayoutPolicyV1 {
    NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A8,
            packed: HirCLayoutValue::A1,
        },
    }
}
fn base(
    coordinate: ConeCoordinate,
    name: &str,
    kind: SourceNominalKind,
    generic: bool,
) -> ProviderFixture {
    let mut fixture = ProviderFixture::empty(coordinate);
    let key = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            fixture.identity(),
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        u32::from(generic),
    );
    if generic {
        let record = CborIdentityRecord::from_key(key.clone()).unwrap();
        fixture.outer = Some(SourceNominalId::GenericTemplate(record.id()));
        fixture.foundation.set_generic_types(vec![record]).unwrap();
    } else {
        let record = CborIdentityRecord::from_key(key.clone()).unwrap();
        fixture.outer = Some(SourceNominalId::Concrete(record.id()));
        fixture.foundation.set_types(vec![record]).unwrap();
    }
    fixture.outer_key = Some(key);
    fixture
}
pub(super) fn set_shape(fixture: &mut ProviderFixture, shape: NominalSourceShapeV1) {
    let binders = (0..fixture
        .outer_key
        .as_ref()
        .unwrap()
        .duplicate_signature()
        .type_parameter_count())
        .map(|index| {
            TypeParameterBinderV1::new(
                CanonicalIdentifier::new(&format!("T{index}")).unwrap(),
                TypeParameterBoundsV1::Unconstrained,
            )
        })
        .collect();
    let nominal = NominalInterfaceRecordV1::try_new(
        fixture.outer.unwrap(),
        shape.kind(),
        CanonicalBinderListV1::try_new(binders).unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::empty(),
        CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::empty(),
        shape,
    )
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::default(),
        CanonicalNominalInterfacesV1::try_new(vec![nominal]).unwrap(),
        CanonicalCallableInterfacesV1::default(),
        CanonicalPropertyInterfacesV1::default(),
        CanonicalTypeAliasInterfacesV1::default(),
        CanonicalCallableSourceInterfacesV1::default(),
        CanonicalExportDefaultTemplatesV1::default(),
        CanonicalExportConstValuesV1::default(),
        CanonicalExportDefinitionSourcesV1::default(),
        CanonicalExternalHirReferencesV1::default(),
    );
}
pub(super) fn structure(
    coordinate: ConeCoordinate,
    name: &str,
    generic: bool,
    types: impl FnOnce(Owner) -> Vec<SignatureTypeKey>,
    policy: NominalCLayoutPolicyV1,
) -> ProviderFixture {
    let mut fixture = base(coordinate, name, SourceNominalKind::Struct, generic);
    let mut fields = Vec::new();
    let shape = types(owner(&fixture))
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            let field = CborIdentityRecord::from_key(
                FieldIdentityKey::source_declared(
                    fixture.outer_key.as_ref().unwrap(),
                    CanonicalIdentifier::new(&format!("field{index}")).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let shape = StructSourceFieldV1::new(field.id(), ty);
            fields.push(field);
            shape
        })
        .collect();
    fixture.foundation.set_fields(fields).unwrap();
    set_shape(
        &mut fixture,
        NominalSourceShapeV1::Struct(StructSourceShapeV1::try_new(shape, policy).unwrap()),
    );
    fixture
}
pub(super) fn enumeration(
    coordinate: ConeCoordinate,
    types: Vec<SignatureTypeKey>,
) -> ProviderFixture {
    let mut fixture = base(coordinate, "Choice", SourceNominalKind::Enum, false);
    let key = fixture.outer_key.as_ref().unwrap();
    let full = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(key, CanonicalIdentifier::new("Full").unwrap()).unwrap(),
    )
    .unwrap();
    let empty = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(key, CanonicalIdentifier::new("Empty").unwrap()).unwrap(),
    )
    .unwrap();
    let mut fields = Vec::new();
    let shape = types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            let field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
                full.id(),
                EnumVariantFieldSelector::Positional {
                    declaration_index: index as u32,
                },
            ))
            .unwrap();
            let shape = EnumSourceFieldV1::new(field.id(), ty);
            fields.push(field);
            shape
        })
        .collect();
    let shape = EnumSourceShapeV1::try_new(vec![
        EnumSourceVariantV1::try_new(full.id(), EnumSourceVariantStyleV1::Positional, shape)
            .unwrap(),
        EnumSourceVariantV1::try_new(empty.id(), EnumSourceVariantStyleV1::Unit, vec![]).unwrap(),
    ])
    .unwrap();
    fixture
        .foundation
        .set_enum_variants(vec![full, empty])
        .unwrap();
    fixture.foundation.set_enum_variant_fields(fields).unwrap();
    set_shape(&mut fixture, NominalSourceShapeV1::Enum(shape));
    fixture
}
pub(super) fn with_world<T>(
    direct: &[&ProviderFixture],
    support: &[&ProviderFixture],
    run: impl FnOnce(&ImportedSemanticWorld<'_>) -> T,
) -> T {
    try_with_world(direct, support, run).unwrap()
}

pub(super) fn try_with_world<T>(
    direct: &[&ProviderFixture],
    support: &[&ProviderFixture],
    run: impl FnOnce(&ImportedSemanticWorld<'_>) -> T,
) -> Result<T, ImportedSemanticWorldBuildError> {
    let mut session = SemanticIdentitySession::new();
    let foundations = direct
        .iter()
        .chain(support)
        .enumerate()
        .map(|(index, fixture)| import_foundation(&mut session, fixture, index as u8 + 1))
        .collect::<Vec<_>>();
    let aliases = empty_alias_expansions();
    let direct_inputs = direct
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            DirectImportedProviderInput::from_validated(
                certificate(&fixture.coordinate, index as u8 + 1),
                &foundations[index],
                &fixture.interface,
                &aliases,
            )
        })
        .collect();
    let support_inputs = support
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            let index = index + direct.len();
            SupportImportedProviderInput::from_validated(
                certificate(&fixture.coordinate, index as u8 + 1),
                &foundations[index],
                &fixture.interface,
                &aliases,
            )
        })
        .collect();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("consumer-native").identity().unwrap(),
        direct_inputs,
        support_inputs,
    )?;
    Ok(run(&world))
}
