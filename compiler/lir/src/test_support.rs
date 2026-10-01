use crate::*;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    ExactTypeKey, PackagePath, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

pub(crate) fn module(functions: Vec<Function>) -> Module {
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("String").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let string = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&declaration).unwrap(),
    ))
    .unwrap();
    let mut external_type_descriptors = Arena::new();
    let string_descriptor = external_type_descriptors
        .alloc(ExternalTypeDescriptor::new(ConeIdentity::CORE, string.id()).unwrap());
    Module {
        cone: ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions,
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: LirOutput::Library,
        meta: LirMeta {
            exact_types: Vec::new(),
            target_profile: LirTargetProfile::DARWIN_AARCH64,
            canonical_c_abi: CanonicalCAbiMetadata::default(),
            native_externals: NativeExternalMetadata::default(),
            well_known_type_descriptors: WellKnownTypeDescriptors {
                string: TypeDescriptorRef::External(string_descriptor),
            },
            arrays: Arena::new(),
            layouts: Arena::new(),
            type_descriptors: Arena::new(),
            external_type_descriptors,
            external_callables: Arena::new(),
        },
    }
}
