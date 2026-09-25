use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use super::*;
use scoop_identity::{AccessorRole, SourceNominalKind};

#[test]
fn constant_overlap_requires_property_owner_representation_and_type_alongside_value() {
    let mut fixture = SourceFixture::default();
    let owner = fixture
        .graph
        .add("Constants", SourceNominalKind::Object, &[]);
    let other = fixture.graph.add("Other", SourceNominalKind::Object, &[]);
    let boolean = fixture.graph.add("Bool", SourceNominalKind::Struct, &[]);
    let SourceNominalId::Concrete(boolean) = boolean.source else {
        unreachable!()
    };
    let ty = SignatureTypeKey::Nominal(boolean);
    let getter = fixture.accessor(owner, AccessorRole::Getter, ty.clone());
    let CallableTemplateOrigin::Accessor(getter) = getter else {
        unreachable!()
    };
    let scoop_identity::PropertyOwner::Property(property) = fixture.accessors[&getter].owner()
    else {
        unreachable!()
    };
    let access = fixture.access(owner, DeclaredVisibilityV1::Public);
    let constant = ExportConstValueV1::new(
        property,
        ty.clone(),
        CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
        access.definition_origin().clone(),
    );
    let source = NominalSupportPropertyInterfaceV1::try_new(
        property,
        access,
        NominalSupportPropertyPayloadV1::Const {
            value: constant.clone(),
        },
    )
    .unwrap();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.graph.records.values(), &fixture.graph)
            .unwrap();
    let interface = |owner, value_type, representation| {
        PropertyInterfaceRecordV1::try_new(
            PropertyDeclarationId::Property(property),
            PublicDeclarationOwnerV1::Nominal(owner),
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            None,
            value_type,
            Accessors::read_only(AccessorSource::new(
                getter,
                match representation {
                    PropertyRepresentationV1::Const => AccessorForm::Constant,
                    PropertyRepresentationV1::RuntimeAccessor => AccessorForm::Body,
                    PropertyRepresentationV1::AbstractSlot => AccessorForm::AbstractSlot,
                },
            )),
            representation,
            PropertyPublicAccessV1::DirectOnly,
            crate::PropertySetterPublicAccessV1::Restricted,
        )
        .unwrap()
    };
    let section = |property: Option<PropertyInterfaceRecordV1>| {
        CrossConeHirInterfaceSectionV1::new(
            Default::default(),
            Default::default(),
            Default::default(),
            CanonicalPropertyInterfacesV1::try_new(property.into_iter().collect()).unwrap(),
            Default::default(),
            CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
            CanonicalExportConstValuesV1::try_new(vec![constant.clone()]).unwrap(),
            Default::default(),
            Default::default(),
        )
    };
    properties::validate::<&str>(
        &source,
        &section(Some(interface(
            owner.source,
            ty.clone(),
            PropertyRepresentationV1::Const,
        ))),
        &graph,
        &path(),
    )
    .unwrap();
    for wrong in [
        None,
        Some(interface(
            other.source,
            ty.clone(),
            PropertyRepresentationV1::Const,
        )),
        Some(interface(
            owner.source,
            unit(),
            PropertyRepresentationV1::Const,
        )),
        Some(interface(
            owner.source,
            ty,
            PropertyRepresentationV1::RuntimeAccessor,
        )),
    ] {
        assert!(properties::validate::<&str>(&source, &section(wrong), &graph, &path()).is_err());
    }
}
