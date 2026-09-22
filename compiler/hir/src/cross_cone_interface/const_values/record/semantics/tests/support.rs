use std::collections::BTreeMap;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerChain, NormalizedSourcePath, PackagePath,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};

use super::super::*;
use crate::{
    CanonicalBinderListV1, CanonicalBooleanV1, CanonicalConstValueKindV1, CanonicalConstValueV1,
    CanonicalIntegerConstantV1, ExportDefinitionSourceV1, IntegerKind, PropertyCapabilityV1,
    PropertyInterfaceRecordV1, PropertyPublicAccessV1, PropertyRepresentationV1,
    PublicDeclarationOwnerV1,
};

pub(super) struct Fixture {
    pub(super) property: PersistentPropertyId,
    pub(super) other_property: PersistentPropertyId,
    pub(super) other_property_key: SourceDeclarationKey,
    pub(super) user_type: PersistentTypeId,
    pub(super) origin: ExportDefinitionSourceV1,
    source: ConstPropertyDeclarationSourceV1,
    core_types: BTreeMap<CanonicalConstValueKindV1, PersistentTypeId>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let declaration_key = property_key("Answer", current_site());
        let property = PersistentPropertyId::from_source_declaration(&declaration_key).unwrap();
        let other_property_key = property_key("Other", current_site());
        let other_property =
            PersistentPropertyId::from_source_declaration(&other_property_key).unwrap();
        let origin = definition_source(source(ConeIdentity::CORE, "src/Constants.scoop"), 0, 12);
        let core_types = canonical_core_types();
        let user_type = nominal_id("MimicBoolean", SourceNominalKind::Struct, current_site());
        Self {
            property,
            other_property,
            other_property_key,
            user_type,
            source: ConstPropertyDeclarationSourceV1::new(declaration_key, origin.origin().clone()),
            origin,
            core_types,
        }
    }

    pub(super) fn value_type(&self, kind: CanonicalConstValueKindV1) -> PersistentTypeId {
        self.core_types[&kind]
    }

    pub(super) fn record(&self) -> ExportConstValueV1 {
        self.record_with(
            CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
            self.value_type(CanonicalConstValueKindV1::Boolean),
            self.origin.clone(),
        )
    }

    pub(super) fn record_with(
        &self,
        value: CanonicalConstValueV1,
        value_type: PersistentTypeId,
        origin: ExportDefinitionSourceV1,
    ) -> ExportConstValueV1 {
        record_for(self.property, value_type, value, origin)
    }

    pub(super) fn authority(&self) -> TestAuthority {
        let value_type = self.value_type(CanonicalConstValueKindV1::Boolean);
        TestAuthority {
            current_cone: ConeIdentity::CORE,
            source_property: self.property,
            source: self.source.clone(),
            interface_property: self.property,
            interface: Some(property_interface(
                self.property,
                value_type,
                PropertyRepresentationV1::Const,
            )),
            core_types: self.core_types.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    Declaration(PersistentPropertyId),
    Interface(PersistentPropertyId),
    Core(CanonicalConstValueKindV1),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

pub(super) struct TestAuthority {
    pub(super) current_cone: ConeIdentity,
    pub(super) source_property: PersistentPropertyId,
    pub(super) source: ConstPropertyDeclarationSourceV1,
    pub(super) interface_property: PersistentPropertyId,
    pub(super) interface: Option<PropertyInterfaceRecordV1>,
    pub(super) core_types: BTreeMap<CanonicalConstValueKindV1, PersistentTypeId>,
}

impl ExportConstValueSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.current_cone
    }

    fn const_property_declaration_source(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<ConstPropertyDeclarationSourceV1, TestAuthorityError> {
        if property == self.source_property {
            Ok(self.source.clone())
        } else {
            Err(TestAuthorityError::Declaration(property))
        }
    }

    fn validated_property_interface(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<&PropertyInterfaceRecordV1, TestAuthorityError> {
        if property == self.interface_property {
            self.interface
                .as_ref()
                .ok_or(TestAuthorityError::Interface(property))
        } else {
            Err(TestAuthorityError::Interface(property))
        }
    }

    fn validate_const_value_type(
        &mut self,
        value_type: PersistentTypeId,
        kind: CanonicalConstValueKindV1,
    ) -> Result<(), TestAuthorityError> {
        self.core_types
            .get(&kind)
            .filter(|&&expected| expected == value_type)
            .map(|_| ())
            .ok_or(TestAuthorityError::Core(kind))
    }
}

pub(super) fn value_cases() -> Vec<(CanonicalConstValueV1, CanonicalConstValueKindV1)> {
    vec![
        integer(CanonicalIntegerConstantV1::Signed8(0xff)),
        integer(CanonicalIntegerConstantV1::Signed16(0xfffe)),
        integer(CanonicalIntegerConstantV1::Signed32(0xffff_fffd)),
        integer(CanonicalIntegerConstantV1::Signed64(0xffff_ffff_ffff_fffc)),
        integer(CanonicalIntegerConstantV1::Unsigned8(1)),
        integer(CanonicalIntegerConstantV1::Unsigned16(2)),
        integer(CanonicalIntegerConstantV1::Unsigned32(3)),
        integer(CanonicalIntegerConstantV1::Unsigned64(4)),
        (
            CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
            CanonicalConstValueKindV1::Boolean,
        ),
        (
            CanonicalConstValueV1::String("value".to_owned()),
            CanonicalConstValueKindV1::String,
        ),
    ]
}

fn integer(
    value: CanonicalIntegerConstantV1,
) -> (CanonicalConstValueV1, CanonicalConstValueKindV1) {
    (
        CanonicalConstValueV1::Integer(value),
        CanonicalConstValueKindV1::Integer(value.kind()),
    )
}

pub(super) fn record_for(
    property: PersistentPropertyId,
    value_type: PersistentTypeId,
    value: CanonicalConstValueV1,
    origin: ExportDefinitionSourceV1,
) -> ExportConstValueV1 {
    ExportConstValueV1::new(
        property,
        SignatureTypeKey::Nominal(value_type),
        value,
        origin,
    )
}

pub(super) fn property_interface(
    property: PersistentPropertyId,
    value_type: PersistentTypeId,
    representation: PropertyRepresentationV1,
) -> PropertyInterfaceRecordV1 {
    property_interface_with_type(
        property,
        SignatureTypeKey::Nominal(value_type),
        representation,
    )
}

pub(super) fn property_interface_with_type(
    property: PersistentPropertyId,
    value_type: SignatureTypeKey,
    representation: PropertyRepresentationV1,
) -> PropertyInterfaceRecordV1 {
    let declaration = PropertyOwner::Property(property);
    let getter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        declaration,
        AccessorRole::Getter,
    ))
    .unwrap();
    PropertyInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        value_type,
        PropertyCapabilityV1::read_only(getter),
        representation,
        PropertyPublicAccessV1::DirectOnly,
    )
    .unwrap()
}

fn canonical_core_types() -> BTreeMap<CanonicalConstValueKindV1, PersistentTypeId> {
    let mut types = BTreeMap::new();
    for integer_kind in IntegerKind::ALL {
        types.insert(
            CanonicalConstValueKindV1::Integer(integer_kind),
            nominal_id(
                integer_kind.canonical_name(),
                SourceNominalKind::Struct,
                core_site(),
            ),
        );
    }
    types.insert(
        CanonicalConstValueKindV1::Boolean,
        nominal_id("Boolean", SourceNominalKind::Struct, core_site()),
    );
    types.insert(
        CanonicalConstValueKindV1::String,
        nominal_id("String", SourceNominalKind::Class, core_site()),
    );
    types
}

fn nominal_id(
    name: &str,
    kind: SourceNominalKind,
    site: SourceDeclarationSite,
) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site,
        identifier(name),
        kind,
        0,
    ))
    .unwrap()
}

pub(super) fn property_key(name: &str, site: SourceDeclarationSite) -> SourceDeclarationKey {
    SourceDeclarationKey::property(site, identifier(name))
}

pub(super) fn current_site() -> SourceDeclarationSite {
    site(ConeIdentity::CORE, DeclarationScope::ConeWide)
}

fn core_site() -> SourceDeclarationSite {
    site(ConeIdentity::CORE, DeclarationScope::ConeWide)
}

pub(super) fn site(cone: ConeIdentity, scope: DeclarationScope) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        scope,
    )
    .unwrap()
}

pub(super) fn definition_source(
    source: SourceIdentity,
    start: u64,
    end: u64,
) -> ExportDefinitionSourceV1 {
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(start, end).unwrap(), &context).unwrap(),
    )
}

pub(super) fn source(cone: ConeIdentity, path: &str) -> SourceIdentity {
    SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
}

pub(super) fn foreign_cone() -> ConeIdentity {
    ConeCoordinate::new("example", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

pub(super) fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
