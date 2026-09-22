//! Exact source-nominal closure required to validate native boundaries.

use la_arena::Arena;

use crate::{
    ClassDecl, EnumDecl, HirCallbackRegistrationIdentities, HirEnumMemberIdentities,
    HirFieldIdentities, HirInitializationUnitIdentities, HirNominalIdentities,
    HirSignatureTypeMapper, HirSourceNativeContracts, HirTypeIdentityInputs, ImportedSemanticWorld,
    InterfaceDecl, NativeBoundaryTypeDefinitionRecord, ObjectDecl, StructDecl,
};

mod c_abi;
pub(crate) mod closure;
mod declarations;
mod error;
mod records;
mod roots;

use declarations::LocalNominalDeclarations;
pub use error::HirNativeBoundaryTypeDefinitionError;

pub struct HirNativeBoundaryTypeDefinitionInputs<'a> {
    pub protocols: &'a crate::CoreProtocols,
    pub structs: &'a Arena<StructDecl>,
    pub enums: &'a Arena<EnumDecl>,
    pub classes: &'a Arena<ClassDecl>,
    pub interfaces: &'a Arena<InterfaceDecl>,
    pub objects: &'a Arena<ObjectDecl>,
    pub nominal_identities: &'a HirNominalIdentities,
    pub field_identities: &'a HirFieldIdentities,
    pub enum_member_identities: &'a HirEnumMemberIdentities,
    pub type_inputs: HirTypeIdentityInputs<'a>,
    pub source_native_contracts: &'a HirSourceNativeContracts,
    pub callback_registration_identities: &'a HirCallbackRegistrationIdentities,
    pub initialization_unit_identities: &'a HirInitializationUnitIdentities,
    pub local: &'a crate::concrete::Module,
    pub dependencies: &'a ImportedSemanticWorld<'a>,
}

/// Canonically ordered, exact native-boundary source-nominal closure.
///
/// The relation contains no general-purpose layout information. Its records
/// are only the source witnesses needed to revalidate the native contracts,
/// callback registrations and callback applications in the same HIR output.
#[derive(Clone, Debug)]
pub struct HirNativeBoundaryTypeDefinitions {
    records: Vec<NativeBoundaryTypeDefinitionRecord>,
}

impl HirNativeBoundaryTypeDefinitions {
    pub fn from_roots(
        inputs: HirNativeBoundaryTypeDefinitionInputs<'_>,
    ) -> Result<Self, HirNativeBoundaryTypeDefinitionError> {
        let declarations = LocalNominalDeclarations::new(&inputs)?;
        let required = roots::collect(&inputs)?;
        let mapper = HirSignatureTypeMapper::new(inputs.type_inputs);
        let result = closure::close(required, |owner| {
            if let Some(declaration) = declarations.get(owner) {
                c_abi::project(records::build(declaration, &inputs, &mapper)?, &inputs)
            } else {
                inputs
                    .dependencies
                    .native_boundary_type_definition(owner, |record| {
                        c_abi::project(record, &inputs)
                    })
            }
        })?;
        Ok(Self { records: result })
    }

    pub fn records(&self) -> &[NativeBoundaryTypeDefinitionRecord] {
        &self.records
    }
}
