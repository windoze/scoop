use scoop_identity::{
    BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain, EnumVariantIdentityKey,
    ExportBindingKey, FieldIdentityKey, NominalDeclarationOwner, PackagePath,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExportBindingId, PersistentFieldId,
    PersistentFunctionId, PersistentGenericTypeId, PersistentPropertyId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    ValidatedIdentityGraph,
};

use super::super::*;
use crate::{
    CanonicalBinderListV1, CanonicalPersistentIdsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, PublicMemberRefV1, StructSourceFieldV1, StructSourceShapeV1,
    TypeParameterBinderV1, TypeParameterBoundsV1,
};

pub(super) struct Fixture {
    pub(super) owner: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>,
    pub(super) superclass: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    pub(super) constructor: CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>,
    pub(super) function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    pub(super) property: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    pub(super) field: CborIdentityRecord<PersistentFieldId, FieldIdentityKey>,
    pub(super) nested_binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>,
    pub(super) variant: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let owner_key = nominal("Container", SourceNominalKind::Struct, 1);
        let owner = CborIdentityRecord::from_key(owner_key.clone()).unwrap();
        let superclass =
            CborIdentityRecord::from_key(nominal("Base", SourceNominalKind::Class, 0)).unwrap();
        let member_site = owned_site(DefinitionOwnerAtom::GenericType(owner.id()));
        let constructor = CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
            member_site.clone(),
            vec![binder(0)],
        ))
        .unwrap();
        let function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            member_site.clone(),
            identifier("inspect"),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
        let property = CborIdentityRecord::from_key(SourceDeclarationKey::property(
            member_site,
            identifier("value"),
        ))
        .unwrap();
        let field = CborIdentityRecord::from_key(
            FieldIdentityKey::source_declared(&owner_key, identifier("stored")).unwrap(),
        )
        .unwrap();
        let nested_key = nominal("Nested", SourceNominalKind::Class, 0);
        let nested_binding = CborIdentityRecord::from_key(ExportBindingKey::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            identifier("Nested"),
            BindingTarget::type_name(&nested_key).unwrap(),
        ))
        .unwrap();
        let enum_key = nominal("Auxiliary", SourceNominalKind::Enum, 0);
        let variant = CborIdentityRecord::from_key(
            EnumVariantIdentityKey::source(&enum_key, identifier("Entry")).unwrap(),
        )
        .unwrap();

        Self {
            owner,
            superclass,
            constructor,
            function,
            property,
            field,
            nested_binding,
            variant,
        }
    }

    pub(super) fn record(&self) -> NominalInterfaceRecordV1 {
        NominalInterfaceRecordV1::try_new(
            NominalDeclarationOwner::GenericTemplate(self.owner.id()),
            PublicNominalKindV1::Struct,
            CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
                identifier("T"),
                TypeParameterBoundsV1::Unconstrained,
            )])
            .unwrap(),
            CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::Nominal(
                self.superclass.id(),
            )])
            .unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![self.constructor.id()]).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(vec![
                PublicMemberRefV1::Callable(CallableTemplateOrigin::Function(self.function.id())),
                PublicMemberRefV1::Property(scoop_identity::PropertyOwner::Property(
                    self.property.id(),
                )),
            ])
            .unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![self.nested_binding.id()]).unwrap(),
            NominalSourceShapeV1::Struct(
                StructSourceShapeV1::try_new(
                    vec![StructSourceFieldV1::new(self.field.id(), binder(0))],
                    crate::NominalCLayoutPolicyV1::Ordinary,
                )
                .unwrap(),
            ),
        )
        .unwrap()
    }

    pub(super) fn authority(&self, include_field: bool) -> ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending
            .register_external_canonical_authority(self.owner.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.superclass.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.constructor.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.function.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.property.clone())
            .unwrap();
        if include_field {
            pending
                .register_external_canonical_authority(self.field.clone())
                .unwrap();
        }
        pending
            .register_external_canonical_authority(self.nested_binding.clone())
            .unwrap();
        pending.finish().unwrap()
    }
}

fn nominal(name: &str, kind: SourceNominalKind, type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        identifier(name),
        kind,
        type_parameter_count,
    )
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}
