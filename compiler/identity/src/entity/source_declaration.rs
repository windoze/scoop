use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    DeclarationName, DeclarationScope, DefinitionOwnerChain, DuplicateSignatureKey,
    OptionalSignatureType, SignatureTypeKey,
};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, ConeIdentity, PackagePath, PersistentConstructorId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentObjectValueId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceNominalKind {
    Class,
    Interface,
    Struct,
    Enum,
    Object,
    AnnotationClass,
}

impl SourceNominalKind {
    const fn declaration_kind(self) -> SourceDeclarationKind {
        match self {
            Self::Class => SourceDeclarationKind::Class,
            Self::Interface => SourceDeclarationKind::Interface,
            Self::Struct => SourceDeclarationKind::Struct,
            Self::Enum => SourceDeclarationKind::Enum,
            Self::Object => SourceDeclarationKind::Object,
            Self::AnnotationClass => SourceDeclarationKind::AnnotationClass,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceDeclarationKind {
    Class,
    Interface,
    Struct,
    Enum,
    Object,
    AnnotationClass,
    Function,
    Constructor,
    Property,
    ExtensionProperty,
    TypeAlias,
}

impl SourceDeclarationKind {
    const fn tag(self) -> u64 {
        match self {
            Self::Class => 1,
            Self::Interface => 2,
            Self::Struct => 3,
            Self::Enum => 4,
            Self::Object => 5,
            Self::AnnotationClass => 6,
            Self::Function => 7,
            Self::Constructor => 8,
            Self::Property => 9,
            Self::ExtensionProperty => 10,
            Self::TypeAlias => 11,
        }
    }

    pub const fn is_nominal(self) -> bool {
        matches!(
            self,
            Self::Class
                | Self::Interface
                | Self::Struct
                | Self::Enum
                | Self::Object
                | Self::AnnotationClass
        )
    }
}

impl WireEncode for SourceDeclarationKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.tag())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceDeclarationSite {
    origin: ConeIdentity,
    package: PackagePath,
    owners: DefinitionOwnerChain,
    scope: DeclarationScope,
}

impl SourceDeclarationSite {
    pub fn new(
        origin: ConeIdentity,
        package: PackagePath,
        owners: DefinitionOwnerChain,
        scope: DeclarationScope,
    ) -> Result<Self, SourceDeclarationKeyError> {
        if scope.source().is_some_and(|source| source.cone() != origin) {
            return Err(SourceDeclarationKeyError::ScopeConeMismatch);
        }
        Ok(Self {
            origin,
            package,
            owners,
            scope,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceDeclarationKey {
    origin: ConeIdentity,
    package: PackagePath,
    owners: DefinitionOwnerChain,
    name: DeclarationName,
    declaration_kind: SourceDeclarationKind,
    duplicate_signature: DuplicateSignatureKey,
    scope: DeclarationScope,
}

impl SourceDeclarationKey {
    pub fn nominal(
        site: SourceDeclarationSite,
        name: CanonicalIdentifier,
        kind: SourceNominalKind,
        type_parameter_count: u32,
    ) -> Self {
        Self::build(
            site,
            DeclarationName::Named(name),
            kind.declaration_kind(),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        )
    }

    pub fn function(
        site: SourceDeclarationSite,
        name: CanonicalIdentifier,
        type_parameter_count: u32,
        receiver: Option<SignatureTypeKey>,
        parameters: Vec<SignatureTypeKey>,
    ) -> Self {
        Self::build(
            site,
            DeclarationName::Named(name),
            SourceDeclarationKind::Function,
            DuplicateSignatureKey::Function {
                type_parameter_count,
                receiver: OptionalSignatureType::from_option(receiver),
                parameters,
            },
        )
    }

    pub fn constructor(site: SourceDeclarationSite, parameters: Vec<SignatureTypeKey>) -> Self {
        Self::build(
            site,
            DeclarationName::Constructor,
            SourceDeclarationKind::Constructor,
            DuplicateSignatureKey::Constructor { parameters },
        )
    }

    pub fn property(site: SourceDeclarationSite, name: CanonicalIdentifier) -> Self {
        Self::build(
            site,
            DeclarationName::Named(name),
            SourceDeclarationKind::Property,
            DuplicateSignatureKey::Property {
                type_parameter_count: 0,
                receiver: OptionalSignatureType::Absent,
            },
        )
    }

    pub fn extension_property(
        site: SourceDeclarationSite,
        name: CanonicalIdentifier,
        type_parameter_count: u32,
        receiver: SignatureTypeKey,
    ) -> Self {
        Self::build(
            site,
            DeclarationName::Named(name),
            SourceDeclarationKind::ExtensionProperty,
            DuplicateSignatureKey::Property {
                type_parameter_count,
                receiver: OptionalSignatureType::Present(Box::new(receiver)),
            },
        )
    }

    pub fn type_alias(site: SourceDeclarationSite, name: CanonicalIdentifier) -> Self {
        Self::build(
            site,
            DeclarationName::Named(name),
            SourceDeclarationKind::TypeAlias,
            DuplicateSignatureKey::TypeAlias,
        )
    }

    fn build(
        site: SourceDeclarationSite,
        name: DeclarationName,
        declaration_kind: SourceDeclarationKind,
        duplicate_signature: DuplicateSignatureKey,
    ) -> Self {
        Self {
            origin: site.origin,
            package: site.package,
            owners: site.owners,
            name,
            declaration_kind,
            duplicate_signature,
            scope: site.scope,
        }
    }

    pub fn origin(&self) -> ConeIdentity {
        self.origin
    }

    pub fn package(&self) -> &PackagePath {
        &self.package
    }

    pub fn owners(&self) -> &DefinitionOwnerChain {
        &self.owners
    }

    pub fn name(&self) -> &DeclarationName {
        &self.name
    }

    pub fn declaration_kind(&self) -> SourceDeclarationKind {
        self.declaration_kind
    }

    pub fn duplicate_signature(&self) -> &DuplicateSignatureKey {
        &self.duplicate_signature
    }

    pub fn scope(&self) -> &DeclarationScope {
        &self.scope
    }
}

impl WireEncode for SourceDeclarationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.package.encode(encoder)?;
        encoder.field(3)?;
        self.owners.encode(encoder)?;
        encoder.field(4)?;
        self.name.encode(encoder)?;
        encoder.field(5)?;
        self.declaration_kind.encode(encoder)?;
        encoder.field(6)?;
        self.duplicate_signature.encode(encoder)?;
        encoder.field(7)?;
        self.scope.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDeclarationKeyError {
    ScopeConeMismatch,
}

impl fmt::Display for SourceDeclarationKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("source-scoped declaration must belong to its origin Cone")
    }
}

impl std::error::Error for SourceDeclarationKeyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDeclarationIdentityError {
    ExpectedNominal,
    ExpectedNonGeneric,
    ExpectedGeneric,
    ExpectedFunction,
    ExpectedConstructor,
    ExpectedProperty,
    ExpectedExtensionProperty,
    ExpectedTypeAlias,
    ExpectedObject,
    Hash(HashError),
}

impl fmt::Display for SourceDeclarationIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedNominal => "identity requires a nominal source declaration key",
            Self::ExpectedNonGeneric => "identity requires a non-generic source declaration key",
            Self::ExpectedGeneric => "identity requires a generic source declaration key",
            Self::ExpectedFunction => "identity requires a function source declaration key",
            Self::ExpectedConstructor => "identity requires a constructor source declaration key",
            Self::ExpectedProperty => {
                "identity requires an ordinary property source declaration key"
            }
            Self::ExpectedExtensionProperty => {
                "identity requires an extension property source declaration key"
            }
            Self::ExpectedTypeAlias => "identity requires a type-alias source declaration key",
            Self::ExpectedObject => "identity requires a non-generic source object declaration key",
            Self::Hash(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for SourceDeclarationIdentityError {}

impl From<HashError> for SourceDeclarationIdentityError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

impl PersistentTypeId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_nominal(key)?;
        require_non_generic(key)?;
        derive_persistent_id("scoop-type-id-v1", &SourceTypeIdentityKey(key)).map_err(Into::into)
    }
}

impl PersistentGenericTypeId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_nominal(key)?;
        require_generic(key)?;
        derive_persistent_id("scoop-generic-type-id-v1", key).map_err(Into::into)
    }
}

impl PersistentFunctionId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::Function,
            SourceDeclarationIdentityError::ExpectedFunction,
        )?;
        require_non_generic(key)?;
        derive_persistent_id("scoop-function-id-v1", key).map_err(Into::into)
    }
}

impl PersistentGenericFunctionId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::Function,
            SourceDeclarationIdentityError::ExpectedFunction,
        )?;
        require_generic(key)?;
        derive_persistent_id("scoop-generic-function-id-v1", key).map_err(Into::into)
    }
}

impl PersistentConstructorId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::Constructor,
            SourceDeclarationIdentityError::ExpectedConstructor,
        )?;
        derive_persistent_id("scoop-constructor-id-v1", key).map_err(Into::into)
    }
}

impl PersistentPropertyId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::Property,
            SourceDeclarationIdentityError::ExpectedProperty,
        )?;
        derive_persistent_id("scoop-property-id-v1", key).map_err(Into::into)
    }
}

impl crate::PersistentExtensionPropertyId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::ExtensionProperty,
            SourceDeclarationIdentityError::ExpectedExtensionProperty,
        )?;
        derive_persistent_id("scoop-extension-property-id-v1", key).map_err(Into::into)
    }
}

impl PersistentTypeAliasId {
    pub fn from_source_declaration(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        require_kind(
            key,
            SourceDeclarationKind::TypeAlias,
            SourceDeclarationIdentityError::ExpectedTypeAlias,
        )?;
        derive_persistent_id("scoop-type-alias-id-v1", key).map_err(Into::into)
    }
}

impl PersistentObjectValueId {
    pub fn from_source_object(
        key: &SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        if key.declaration_kind != SourceDeclarationKind::Object {
            return Err(SourceDeclarationIdentityError::ExpectedObject);
        }
        require_non_generic(key).map_err(|_| SourceDeclarationIdentityError::ExpectedObject)?;
        let type_id = PersistentTypeId::from_source_declaration(key)?;
        derive_persistent_id("scoop-object-value-id-v1", &ObjectValueKey(type_id))
            .map_err(Into::into)
    }
}

struct SourceTypeIdentityKey<'key>(&'key SourceDeclarationKey);

impl WireEncode for SourceTypeIdentityKey<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(1)?;
        encoder.field(1)?;
        self.0.encode(encoder)
    }
}

struct ObjectValueKey(PersistentTypeId);

impl WireEncode for ObjectValueKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.0.encode(encoder)
    }
}

fn require_nominal(key: &SourceDeclarationKey) -> Result<(), SourceDeclarationIdentityError> {
    if key.declaration_kind.is_nominal() {
        Ok(())
    } else {
        Err(SourceDeclarationIdentityError::ExpectedNominal)
    }
}

fn require_non_generic(key: &SourceDeclarationKey) -> Result<(), SourceDeclarationIdentityError> {
    if key.duplicate_signature.type_parameter_count() == 0 {
        Ok(())
    } else {
        Err(SourceDeclarationIdentityError::ExpectedNonGeneric)
    }
}

fn require_generic(key: &SourceDeclarationKey) -> Result<(), SourceDeclarationIdentityError> {
    if key.duplicate_signature.type_parameter_count() > 0 {
        Ok(())
    } else {
        Err(SourceDeclarationIdentityError::ExpectedGeneric)
    }
}

fn require_kind(
    key: &SourceDeclarationKey,
    expected: SourceDeclarationKind,
    error: SourceDeclarationIdentityError,
) -> Result<(), SourceDeclarationIdentityError> {
    if key.declaration_kind == expected {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationSite,
        SourceNominalKind,
    };
    use crate::{
        CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
        NormalizedSourcePath, PackagePath, PersistentConstructorId, PersistentFunctionId,
        PersistentGenericFunctionId, PersistentGenericTypeId, PersistentObjectValueId,
        PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
        SourceIdentity,
    };

    fn class_key(type_parameter_count: u32) -> SourceDeclarationKey {
        SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeCoordinate::new("org.example", "demo", "1.2.3")
                    .unwrap()
                    .identity()
                    .unwrap(),
                PackagePath::from_segments(vec![CanonicalIdentifier::new("app").unwrap()]),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("User").unwrap(),
            SourceNominalKind::Class,
            type_parameter_count,
        )
    }

    #[test]
    fn source_declaration_key_has_fixed_wire() {
        assert_eq!(
            hex(&encode(&class_key(0)).unwrap()),
            "a7015820730c104d0d63b08d3ac5eecd4f73859a83715e34cb775d5e988d3dd139d28301028163617070038004a20001016455736572050106a20001010007a10001"
        );
    }

    #[test]
    fn nominal_id_kind_and_genericity_are_enforced() {
        let concrete = class_key(0);
        assert!(PersistentTypeId::from_source_declaration(&concrete).is_ok());
        assert_eq!(
            PersistentGenericTypeId::from_source_declaration(&concrete),
            Err(SourceDeclarationIdentityError::ExpectedGeneric)
        );

        let generic = class_key(1);
        assert!(PersistentGenericTypeId::from_source_declaration(&generic).is_ok());
        assert_eq!(
            PersistentTypeId::from_source_declaration(&generic),
            Err(SourceDeclarationIdentityError::ExpectedNonGeneric)
        );
    }

    #[test]
    fn source_nominal_and_object_value_have_fixed_distinct_hashes() {
        let object = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeCoordinate::new("org.example", "demo", "1.2.3")
                    .unwrap()
                    .identity()
                    .unwrap(),
                PackagePath::from_segments(vec![CanonicalIdentifier::new("app").unwrap()]),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Global").unwrap(),
            SourceNominalKind::Object,
            0,
        );
        let type_id = PersistentTypeId::from_source_declaration(&object).unwrap();
        let value_id = PersistentObjectValueId::from_source_object(&object).unwrap();
        assert_eq!(
            type_id.to_string(),
            "2917b044e5466d7efa4989070d893eaf313fa2561460fd9c65619538fe8bf5d0"
        );
        assert_eq!(
            value_id.to_string(),
            "72facdaa3ed11e993a971a19e20edade4299c7601decf2de52c59c49c99fe1ab"
        );
        assert_ne!(type_id.as_array(), value_id.as_array());
    }

    #[test]
    fn each_source_declaration_family_accepts_only_its_canonical_key() {
        let origin = ConeCoordinate::reserved_core().identity().unwrap();
        let common = || {
            SourceDeclarationSite::new(
                origin,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap()
        };
        let function = SourceDeclarationKey::function(
            common(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        );
        assert!(PersistentFunctionId::from_source_declaration(&function).is_ok());
        assert_eq!(
            PersistentGenericFunctionId::from_source_declaration(&function),
            Err(SourceDeclarationIdentityError::ExpectedGeneric)
        );

        let generic_function = SourceDeclarationKey::function(
            common(),
            CanonicalIdentifier::new("map").unwrap(),
            1,
            Some(SignatureTypeKey::Binder { depth: 0, index: 0 }),
            Vec::new(),
        );
        assert!(PersistentGenericFunctionId::from_source_declaration(&generic_function).is_ok());

        let constructor = SourceDeclarationKey::constructor(common(), Vec::new());
        assert!(PersistentConstructorId::from_source_declaration(&constructor).is_ok());

        let property =
            SourceDeclarationKey::property(common(), CanonicalIdentifier::new("value").unwrap());
        assert!(PersistentPropertyId::from_source_declaration(&property).is_ok());

        let alias =
            SourceDeclarationKey::type_alias(common(), CanonicalIdentifier::new("Alias").unwrap());
        assert!(PersistentTypeAliasId::from_source_declaration(&alias).is_ok());
        assert_eq!(
            PersistentFunctionId::from_source_declaration(&alias),
            Err(SourceDeclarationIdentityError::ExpectedFunction)
        );
    }

    #[test]
    fn scoped_declaration_must_stay_in_its_origin_cone() {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/core.scoop").unwrap(),
        )
        .unwrap();
        let result = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::SourceScoped(source),
        );
        assert_eq!(
            result,
            Err(super::SourceDeclarationKeyError::ScopeConeMismatch)
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
