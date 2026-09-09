use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationKind};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, ConeIdentity, PackagePath, PersistentEnumVariantId,
    PersistentExportBindingId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentLocalBindingId,
    PersistentObjectValueId, PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
    SourceIdentity,
};

mod decode;

pub use decode::{
    BindingIdentityResolutionError, BindingResolver, DecodedBindableEntity,
    DecodedExportBindingKey, DecodedLocalBindingKey,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindingNamespace {
    Type,
    Value,
}

impl WireEncode for BindingNamespace {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Type => 1,
            Self::Value => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindableEntity {
    Type(PersistentTypeId),
    GenericType(PersistentGenericTypeId),
    ObjectValue(PersistentObjectValueId),
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Property(PersistentPropertyId),
    ExtensionProperty(PersistentExtensionPropertyId),
    TypeAlias(PersistentTypeAliasId),
    EnumVariant(PersistentEnumVariantId),
}

impl WireEncode for BindableEntity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Type(id) => encode_value_sum(encoder, 1, id),
            Self::GenericType(id) => encode_value_sum(encoder, 2, id),
            Self::ObjectValue(id) => encode_value_sum(encoder, 3, id),
            Self::Function(id) => encode_value_sum(encoder, 4, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 5, id),
            Self::Property(id) => encode_value_sum(encoder, 6, id),
            Self::ExtensionProperty(id) => encode_value_sum(encoder, 7, id),
            Self::TypeAlias(id) => encode_value_sum(encoder, 8, id),
            Self::EnumVariant(id) => encode_value_sum(encoder, 9, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindingRole {
    TypeName,
    ObjectValue,
    Function,
    ExtensionFunction,
    Property,
    ExtensionProperty,
    TypeAlias,
    EnumVariant,
}

impl WireEncode for BindingRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::TypeName => 1,
            Self::ObjectValue => 2,
            Self::Function => 3,
            Self::ExtensionFunction => 4,
            Self::Property => 5,
            Self::ExtensionProperty => 6,
            Self::TypeAlias => 7,
            Self::EnumVariant => 8,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BindingTarget {
    namespace: BindingNamespace,
    target: BindableEntity,
    role: BindingRole,
}

impl BindingTarget {
    pub fn type_name(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        if !key.declaration_kind().is_nominal() {
            return Err(BindingTargetError::ExpectedNominal);
        }
        let target = if key.duplicate_signature().type_parameter_count() == 0 {
            BindableEntity::Type(
                PersistentTypeId::from_source_declaration(key)
                    .map_err(BindingTargetError::SourceDeclaration)?,
            )
        } else {
            BindableEntity::GenericType(
                PersistentGenericTypeId::from_source_declaration(key)
                    .map_err(BindingTargetError::SourceDeclaration)?,
            )
        };
        Ok(Self::new(
            BindingNamespace::Type,
            target,
            BindingRole::TypeName,
        ))
    }

    pub fn object_value(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKind::Object {
            return Err(BindingTargetError::ExpectedObject);
        }
        let target = PersistentObjectValueId::from_source_object(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespace::Value,
            BindableEntity::ObjectValue(target),
            BindingRole::ObjectValue,
        ))
    }

    pub fn function(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        function_target(key, false)
    }

    pub fn extension_function(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        function_target(key, true)
    }

    pub fn property(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKind::Property {
            return Err(BindingTargetError::ExpectedProperty);
        }
        let target = PersistentPropertyId::from_source_declaration(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespace::Value,
            BindableEntity::Property(target),
            BindingRole::Property,
        ))
    }

    pub fn extension_property(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKind::ExtensionProperty {
            return Err(BindingTargetError::ExpectedExtensionProperty);
        }
        let target = PersistentExtensionPropertyId::from_source_declaration(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespace::Value,
            BindableEntity::ExtensionProperty(target),
            BindingRole::ExtensionProperty,
        ))
    }

    pub fn type_alias(key: &SourceDeclarationKey) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKind::TypeAlias {
            return Err(BindingTargetError::ExpectedTypeAlias);
        }
        let target = PersistentTypeAliasId::from_source_declaration(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespace::Type,
            BindableEntity::TypeAlias(target),
            BindingRole::TypeAlias,
        ))
    }

    pub const fn enum_variant(target: PersistentEnumVariantId) -> Self {
        Self::new(
            BindingNamespace::Value,
            BindableEntity::EnumVariant(target),
            BindingRole::EnumVariant,
        )
    }

    pub const fn namespace(&self) -> BindingNamespace {
        self.namespace
    }

    pub const fn target(&self) -> BindableEntity {
        self.target
    }

    pub const fn role(&self) -> BindingRole {
        self.role
    }

    const fn new(namespace: BindingNamespace, target: BindableEntity, role: BindingRole) -> Self {
        Self {
            namespace,
            target,
            role,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportBindingKey {
    exporter: ConeIdentity,
    package: PackagePath,
    namespace: BindingNamespace,
    name: CanonicalIdentifier,
    target: BindableEntity,
    role: BindingRole,
}

impl ExportBindingKey {
    pub fn new(
        exporter: ConeIdentity,
        package: PackagePath,
        name: CanonicalIdentifier,
        binding: BindingTarget,
    ) -> Self {
        Self {
            exporter,
            package,
            namespace: binding.namespace,
            name,
            target: binding.target,
            role: binding.role,
        }
    }
}

impl WireEncode for ExportBindingKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.exporter.encode(encoder)?;
        encoder.field(2)?;
        self.package.encode(encoder)?;
        encoder.field(3)?;
        self.namespace.encode(encoder)?;
        encoder.field(4)?;
        self.name.encode(encoder)?;
        encoder.field(5)?;
        self.target.encode(encoder)?;
        encoder.field(6)?;
        self.role.encode(encoder)
    }
}

impl PersistentExportBindingId {
    pub fn from_key(key: &ExportBindingKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-export-binding-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocalBindingRole {
    Declaration,
    ExactImport,
    StarImport,
    AliasImport,
}

impl WireEncode for LocalBindingRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Declaration => 1,
            Self::ExactImport => 2,
            Self::StarImport => 3,
            Self::AliasImport => 4,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalBindingKey {
    origin: ConeIdentity,
    source: SourceIdentity,
    package: PackagePath,
    namespace: BindingNamespace,
    local_name: CanonicalIdentifier,
    target: BindableEntity,
    binding_role: BindingRole,
    source_role: LocalBindingRole,
}

impl LocalBindingKey {
    pub fn new(
        source: SourceIdentity,
        package: PackagePath,
        local_name: CanonicalIdentifier,
        binding: BindingTarget,
        source_role: LocalBindingRole,
    ) -> Self {
        Self {
            origin: source.cone(),
            source,
            package,
            namespace: binding.namespace,
            local_name,
            target: binding.target,
            binding_role: binding.role,
            source_role,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.origin
    }

    pub fn source(&self) -> &SourceIdentity {
        &self.source
    }
}

impl WireEncode for LocalBindingKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.package.encode(encoder)?;
        encoder.field(4)?;
        self.namespace.encode(encoder)?;
        encoder.field(5)?;
        self.local_name.encode(encoder)?;
        encoder.field(6)?;
        self.target.encode(encoder)?;
        encoder.field(7)?;
        self.binding_role.encode(encoder)?;
        encoder.field(8)?;
        self.source_role.encode(encoder)
    }
}

impl PersistentLocalBindingId {
    pub fn from_key(key: &LocalBindingKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-local-binding-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingTargetError {
    ExpectedNominal,
    ExpectedObject,
    ExpectedFunction,
    ExpectedOrdinaryFunction,
    ExpectedExtensionFunction,
    ExpectedProperty,
    ExpectedExtensionProperty,
    ExpectedTypeAlias,
    SourceDeclaration(SourceDeclarationIdentityError),
}

impl fmt::Display for BindingTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedNominal => "type-name binding target must be a source nominal",
            Self::ExpectedObject => "object-value binding target must be a source object",
            Self::ExpectedFunction => "function binding target must be a source function",
            Self::ExpectedOrdinaryFunction => {
                "ordinary function binding target must not have a receiver"
            }
            Self::ExpectedExtensionFunction => {
                "extension-function binding target must have a receiver"
            }
            Self::ExpectedProperty => "property binding target must be an ordinary property",
            Self::ExpectedExtensionProperty => {
                "extension-property binding target must be an extension property"
            }
            Self::ExpectedTypeAlias => "type-alias binding target must be a source type alias",
            Self::SourceDeclaration(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for BindingTargetError {}

fn function_target(
    key: &SourceDeclarationKey,
    extension: bool,
) -> Result<BindingTarget, BindingTargetError> {
    if key.declaration_kind() != SourceDeclarationKind::Function {
        return Err(BindingTargetError::ExpectedFunction);
    }
    if key.duplicate_signature().receiver_is_present() != extension {
        return Err(if extension {
            BindingTargetError::ExpectedExtensionFunction
        } else {
            BindingTargetError::ExpectedOrdinaryFunction
        });
    }
    let target = if key.duplicate_signature().type_parameter_count() == 0 {
        BindableEntity::Function(
            PersistentFunctionId::from_source_declaration(key)
                .map_err(BindingTargetError::SourceDeclaration)?,
        )
    } else {
        BindableEntity::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(key)
                .map_err(BindingTargetError::SourceDeclaration)?,
        )
    };
    Ok(BindingTarget::new(
        BindingNamespace::Value,
        target,
        if extension {
            BindingRole::ExtensionFunction
        } else {
            BindingRole::Function
        },
    ))
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        BindableEntity, BindingRole, BindingTarget, BindingTargetError, ExportBindingKey,
        LocalBindingKey, LocalBindingRole,
    };
    use crate::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentEnumVariantId, PersistentExportBindingId, SignatureTypeKey, SourceDeclarationKey,
        SourceDeclarationSite, SourceIdentity,
    };

    #[test]
    fn export_binding_has_fixed_identity() {
        let target = BindingTarget::enum_variant(PersistentEnumVariantId(ConeIdentity::CORE.0));
        let key = ExportBindingKey::new(
            ConeIdentity::SINGLE_FILE,
            package(),
            identifier("Case"),
            target,
        );
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a601582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b6028163706b67030204644361736505a200090158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d0608"
        );
        assert_eq!(
            PersistentExportBindingId::from_key(&key)
                .unwrap()
                .to_string(),
            "1a346a153530c521d7eac7578dc1adb362271e6795cdafd9bc1486d64147d28d"
        );
    }

    #[test]
    fn function_role_distinguishes_receiver_presence() {
        let ordinary = function(None);
        let extension = function(Some(SignatureTypeKey::Nominal(crate::PersistentTypeId(
            ConeIdentity::CORE.0,
        ))));
        let ordinary_target = BindingTarget::function(&ordinary).unwrap();
        assert_eq!(ordinary_target.role(), BindingRole::Function);
        assert!(matches!(
            ordinary_target.target(),
            BindableEntity::Function(_)
        ));
        assert_eq!(
            BindingTarget::extension_function(&ordinary),
            Err(BindingTargetError::ExpectedExtensionFunction)
        );
        assert_eq!(
            BindingTarget::function(&extension),
            Err(BindingTargetError::ExpectedOrdinaryFunction)
        );
        assert!(BindingTarget::extension_function(&extension).is_ok());
    }

    #[test]
    fn local_binding_origin_is_forced_to_source_cone() {
        let source = SourceIdentity::single_file();
        let key = LocalBindingKey::new(
            source.clone(),
            package(),
            identifier("Case"),
            BindingTarget::enum_variant(PersistentEnumVariantId(ConeIdentity::CORE.0)),
            LocalBindingRole::ExactImport,
        );
        assert_eq!(key.origin(), source.cone());
        assert_eq!(key.source(), &source);
        assert!(encode(&key).is_ok());
    }

    fn function(receiver: Option<SignatureTypeKey>) -> SourceDeclarationKey {
        SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                package(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            identifier("run"),
            0,
            receiver,
            Vec::new(),
        )
    }

    fn package() -> PackagePath {
        PackagePath::from_segments(vec![identifier("pkg")])
    }

    fn identifier(value: &str) -> CanonicalIdentifier {
        CanonicalIdentifier::new(value).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
