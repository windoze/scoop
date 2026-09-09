use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{SourceDeclarationIdentityError, SourceDeclarationKeyV1, SourceDeclarationKindV1};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, ConeIdentity, PackagePath, PersistentEnumVariantId,
    PersistentExportBindingId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentLocalBindingId,
    PersistentObjectValueId, PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
    SourceIdentity,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindingNamespaceV1 {
    Type,
    Value,
}

impl WireEncodeV1 for BindingNamespaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Type => 1,
            Self::Value => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindableEntityV1 {
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

impl WireEncodeV1 for BindableEntityV1 {
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
pub enum BindingRoleV1 {
    TypeName,
    ObjectValue,
    Function,
    ExtensionFunction,
    Property,
    ExtensionProperty,
    TypeAlias,
    EnumVariant,
}

impl WireEncodeV1 for BindingRoleV1 {
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
pub struct BindingTargetV1 {
    namespace: BindingNamespaceV1,
    target: BindableEntityV1,
    role: BindingRoleV1,
}

impl BindingTargetV1 {
    pub fn type_name(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        if !key.declaration_kind().is_nominal() {
            return Err(BindingTargetError::ExpectedNominal);
        }
        let target = if key.duplicate_signature().type_parameter_count() == 0 {
            BindableEntityV1::Type(
                PersistentTypeId::from_source_declaration(key)
                    .map_err(BindingTargetError::SourceDeclaration)?,
            )
        } else {
            BindableEntityV1::GenericType(
                PersistentGenericTypeId::from_source_declaration(key)
                    .map_err(BindingTargetError::SourceDeclaration)?,
            )
        };
        Ok(Self::new(
            BindingNamespaceV1::Type,
            target,
            BindingRoleV1::TypeName,
        ))
    }

    pub fn object_value(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKindV1::Object {
            return Err(BindingTargetError::ExpectedObject);
        }
        let target = PersistentObjectValueId::from_source_object(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespaceV1::Value,
            BindableEntityV1::ObjectValue(target),
            BindingRoleV1::ObjectValue,
        ))
    }

    pub fn function(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        function_target(key, false)
    }

    pub fn extension_function(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        function_target(key, true)
    }

    pub fn property(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKindV1::Property {
            return Err(BindingTargetError::ExpectedProperty);
        }
        let target = PersistentPropertyId::from_source_declaration(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespaceV1::Value,
            BindableEntityV1::Property(target),
            BindingRoleV1::Property,
        ))
    }

    pub fn extension_property(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKindV1::ExtensionProperty {
            return Err(BindingTargetError::ExpectedExtensionProperty);
        }
        let target = PersistentExtensionPropertyId::from_source_declaration(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespaceV1::Value,
            BindableEntityV1::ExtensionProperty(target),
            BindingRoleV1::ExtensionProperty,
        ))
    }

    pub fn type_alias(key: &SourceDeclarationKeyV1) -> Result<Self, BindingTargetError> {
        if key.declaration_kind() != SourceDeclarationKindV1::TypeAlias {
            return Err(BindingTargetError::ExpectedTypeAlias);
        }
        let target = PersistentTypeAliasId::from_source_declaration(key)
            .map_err(BindingTargetError::SourceDeclaration)?;
        Ok(Self::new(
            BindingNamespaceV1::Type,
            BindableEntityV1::TypeAlias(target),
            BindingRoleV1::TypeAlias,
        ))
    }

    pub const fn enum_variant(target: PersistentEnumVariantId) -> Self {
        Self::new(
            BindingNamespaceV1::Value,
            BindableEntityV1::EnumVariant(target),
            BindingRoleV1::EnumVariant,
        )
    }

    pub const fn namespace(&self) -> BindingNamespaceV1 {
        self.namespace
    }

    pub const fn target(&self) -> BindableEntityV1 {
        self.target
    }

    pub const fn role(&self) -> BindingRoleV1 {
        self.role
    }

    const fn new(
        namespace: BindingNamespaceV1,
        target: BindableEntityV1,
        role: BindingRoleV1,
    ) -> Self {
        Self {
            namespace,
            target,
            role,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportBindingKeyV1 {
    exporter: ConeIdentity,
    package: PackagePath,
    namespace: BindingNamespaceV1,
    name: CanonicalIdentifier,
    target: BindableEntityV1,
    role: BindingRoleV1,
}

impl ExportBindingKeyV1 {
    pub fn new(
        exporter: ConeIdentity,
        package: PackagePath,
        name: CanonicalIdentifier,
        binding: BindingTargetV1,
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

impl WireEncodeV1 for ExportBindingKeyV1 {
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
    pub fn from_key(key: &ExportBindingKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-export-binding-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocalBindingRoleV1 {
    Declaration,
    ExactImport,
    StarImport,
    AliasImport,
}

impl WireEncodeV1 for LocalBindingRoleV1 {
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
pub struct LocalBindingKeyV1 {
    origin: ConeIdentity,
    source: SourceIdentity,
    package: PackagePath,
    namespace: BindingNamespaceV1,
    local_name: CanonicalIdentifier,
    target: BindableEntityV1,
    binding_role: BindingRoleV1,
    source_role: LocalBindingRoleV1,
}

impl LocalBindingKeyV1 {
    pub fn new(
        source: SourceIdentity,
        package: PackagePath,
        local_name: CanonicalIdentifier,
        binding: BindingTargetV1,
        source_role: LocalBindingRoleV1,
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

impl WireEncodeV1 for LocalBindingKeyV1 {
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
    pub fn from_key(key: &LocalBindingKeyV1) -> Result<Self, HashError> {
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
    key: &SourceDeclarationKeyV1,
    extension: bool,
) -> Result<BindingTargetV1, BindingTargetError> {
    if key.declaration_kind() != SourceDeclarationKindV1::Function {
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
        BindableEntityV1::Function(
            PersistentFunctionId::from_source_declaration(key)
                .map_err(BindingTargetError::SourceDeclaration)?,
        )
    } else {
        BindableEntityV1::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(key)
                .map_err(BindingTargetError::SourceDeclaration)?,
        )
    };
    Ok(BindingTargetV1::new(
        BindingNamespaceV1::Value,
        target,
        if extension {
            BindingRoleV1::ExtensionFunction
        } else {
            BindingRoleV1::Function
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
    value: &impl WireEncodeV1,
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
        BindableEntityV1, BindingRoleV1, BindingTargetError, BindingTargetV1, ExportBindingKeyV1,
        LocalBindingKeyV1, LocalBindingRoleV1,
    };
    use crate::{
        CanonicalIdentifier, ConeIdentity, DeclarationScopeV1, DefinitionOwnerChainV1, PackagePath,
        PersistentEnumVariantId, PersistentExportBindingId, SignatureTypeKeyV1,
        SourceDeclarationKeyV1, SourceDeclarationSiteV1, SourceIdentity,
    };

    #[test]
    fn export_binding_has_fixed_identity() {
        let target = BindingTargetV1::enum_variant(PersistentEnumVariantId(ConeIdentity::CORE.0));
        let key = ExportBindingKeyV1::new(
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
        let extension = function(Some(SignatureTypeKeyV1::Nominal(crate::PersistentTypeId(
            ConeIdentity::CORE.0,
        ))));
        let ordinary_target = BindingTargetV1::function(&ordinary).unwrap();
        assert_eq!(ordinary_target.role(), BindingRoleV1::Function);
        assert!(matches!(
            ordinary_target.target(),
            BindableEntityV1::Function(_)
        ));
        assert_eq!(
            BindingTargetV1::extension_function(&ordinary),
            Err(BindingTargetError::ExpectedExtensionFunction)
        );
        assert_eq!(
            BindingTargetV1::function(&extension),
            Err(BindingTargetError::ExpectedOrdinaryFunction)
        );
        assert!(BindingTargetV1::extension_function(&extension).is_ok());
    }

    #[test]
    fn local_binding_origin_is_forced_to_source_cone() {
        let source = SourceIdentity::single_file();
        let key = LocalBindingKeyV1::new(
            source.clone(),
            package(),
            identifier("Case"),
            BindingTargetV1::enum_variant(PersistentEnumVariantId(ConeIdentity::CORE.0)),
            LocalBindingRoleV1::ExactImport,
        );
        assert_eq!(key.origin(), source.cone());
        assert_eq!(key.source(), &source);
        assert!(encode(&key).is_ok());
    }

    fn function(receiver: Option<SignatureTypeKeyV1>) -> SourceDeclarationKeyV1 {
        SourceDeclarationKeyV1::function(
            SourceDeclarationSiteV1::new(
                ConeIdentity::CORE,
                package(),
                DefinitionOwnerChainV1::top_level(),
                DeclarationScopeV1::ConeWide,
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
