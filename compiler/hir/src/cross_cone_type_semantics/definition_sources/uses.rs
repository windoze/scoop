use crate::{
    DefaultBodyOriginSiteV1, ExportConstValueV1, ExportDefinitionSourceV1,
    InheritanceConstructorInterfaceV1, InheritanceSlotContractV1, InheritanceSlotTargetV1,
    NestedSourceSupportV1, NominalRepresentationSupportV1, NominalSourcePropertyPayloadV1,
    NominalSupportPropertyInterfaceV1, ProtectedCallableSourceInterfaceV1,
    ProtectedDeclarationInterfaceV1, ProtectedDefaultCallableReferenceV1,
    ProtectedDefaultConstructorReferenceV1, ProtectedDefaultFieldReferenceV1,
    ProtectedDefaultGlobalReferenceV1, ProtectedDefaultSingletonReferenceV1,
    ProtectedDefaultTemplateV1, ProtectedDefaultTypeReferenceV1, SourceNominalId,
    TemplateLocalRecordV1,
};
use scoop_identity::{PersistentExactTypeId, PersistentPropertyId};
use scoop_wire::WirePath;

/// Actual typed source locations; the authority must join these to independent
/// foundation/provider records, rather than trusting the supplied origin.
#[derive(Clone, Copy, Debug)]
pub enum TypeDefinitionSourceUseV1<'a> {
    Representation(&'a NominalRepresentationSupportV1),
    InheritanceConstructor {
        owner: PersistentExactTypeId,
        constructor: &'a InheritanceConstructorInterfaceV1,
    },
    SlotDeclaration {
        owner: PersistentExactTypeId,
        slot: &'a InheritanceSlotContractV1,
    },
    SlotImplementation {
        owner: PersistentExactTypeId,
        slot: &'a InheritanceSlotContractV1,
        target: &'a InheritanceSlotTargetV1,
    },
    ProtectedDeclaration(&'a ProtectedDeclarationInterfaceV1),
    NestedSupport {
        owner: SourceNominalId,
        declaration: &'a NestedSourceSupportV1,
    },
    PropertySetter {
        property: PersistentPropertyId,
        interface: &'a NominalSourcePropertyPayloadV1,
    },
    NestedConst {
        property: &'a NominalSupportPropertyInterfaceV1,
        value: &'a ExportConstValueV1,
    },
    SourceParameter {
        source: &'a ProtectedCallableSourceInterfaceV1,
        parameter_index: usize,
    },
    Default {
        template: &'a ProtectedDefaultTemplateV1,
        site: TypeDefinitionDefaultOriginSiteV1<'a>,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum TypeDefinitionDefaultOriginSiteV1<'a> {
    Root,
    Local(&'a TemplateLocalRecordV1),
    Body(DefaultBodyOriginSiteV1),
    Callable(&'a ProtectedDefaultCallableReferenceV1),
    Constructor(&'a ProtectedDefaultConstructorReferenceV1),
    Type(&'a ProtectedDefaultTypeReferenceV1),
    Global(&'a ProtectedDefaultGlobalReferenceV1),
    Singleton(&'a ProtectedDefaultSingletonReferenceV1),
    Field(&'a ProtectedDefaultFieldReferenceV1),
}

pub trait TypeDefinitionSourceSemanticAuthority<E> {
    /// Verifies this exact occurrence against its real provider's foundation
    /// source/context/span and declaration/body relation. Foreign origins stay
    /// foreign; an enclosing section's Cone is not an origin replacement.
    fn validate_type_definition_source_use(
        &mut self,
        source_use: TypeDefinitionSourceUseV1<'_>,
        source: &ExportDefinitionSourceV1,

        path: &WirePath,
    ) -> Result<(), E>;
}
