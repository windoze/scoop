//! Exact source-nominal closure required to validate native boundaries.

use std::collections::BTreeSet;

use la_arena::Arena;

use crate::{
    ClassDecl, EnumDecl, HirCallbackRegistrationIdentities, HirEnumMemberIdentities,
    HirFieldIdentities, HirInitializationUnitIdentities, HirNominalIdentities,
    HirSignatureTypeMapper, HirSourceNativeContracts, HirTypeIdentityInputs, InterfaceDecl,
    NativeBoundaryTypeDefinitionRecord, ObjectDecl, StructDecl,
};

mod declarations;
mod error;
mod records;
mod roots;

use declarations::LocalNominalDeclarations;
pub use error::HirNativeBoundaryTypeDefinitionError;

pub struct HirNativeBoundaryTypeDefinitionInputs<'a> {
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
        let mut required = roots::collect(&inputs)?;
        let mapper = HirSignatureTypeMapper::new(inputs.type_inputs);
        let mut visited = BTreeSet::new();
        let mut result = Vec::new();

        while let Some(owner) = required.pop_first() {
            if !visited.insert(owner) {
                continue;
            }
            let declaration = declarations
                .get(owner)
                .ok_or(HirNativeBoundaryTypeDefinitionError::MissingSourceNominal { owner })?;
            result.push(records::build(
                declaration,
                &inputs,
                &mapper,
                &mut required,
            )?);
        }
        result.sort_by(|left, right| left.owner().compare_sort_key(right.owner()));
        Ok(Self { records: result })
    }

    pub fn records(&self) -> &[NativeBoundaryTypeDefinitionRecord] {
        &self.records
    }
}
