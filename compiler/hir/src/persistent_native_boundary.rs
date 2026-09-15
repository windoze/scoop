//! Exact source-nominal closure required to validate native boundaries.

use std::collections::BTreeSet;

use la_arena::Arena;

use crate::{
    ClassDecl, EnumDecl, HirCallbackRegistrationIdentities, HirEnumMemberIdentities,
    HirFieldIdentities, HirInitializationUnitIdentities, HirNominalIdentities,
    HirSignatureTypeMapper, HirSourceNativeContracts, HirTypeIdentityInputs,
    ImportedCoreNativeBoundaryTypes, InterfaceDecl, NativeBoundaryNominalOwner,
    NativeBoundaryTypeDefinitionRecord, ObjectDecl, StructDecl,
};

mod declarations;
mod error;
mod records;
mod roots;

use declarations::LocalNominalDeclarations;
pub use error::HirNativeBoundaryTypeDefinitionError;

#[derive(Clone, Copy, Debug)]
pub enum HirNativeBoundaryExternalTypes<'a> {
    CurrentArtifactOnly,
    TrustedCore(&'a ImportedCoreNativeBoundaryTypes),
}

impl<'a> HirNativeBoundaryExternalTypes<'a> {
    fn definition(
        self,
        owner: NativeBoundaryNominalOwner,
    ) -> Option<&'a NativeBoundaryTypeDefinitionRecord> {
        match self {
            Self::CurrentArtifactOnly => None,
            Self::TrustedCore(core) => core.definition(owner),
        }
    }
}

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
    pub external_types: HirNativeBoundaryExternalTypes<'a>,
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
            if let Some(declaration) = declarations.get(owner) {
                result.push(records::build(
                    declaration,
                    &inputs,
                    &mapper,
                    &mut required,
                )?);
            } else if let Some(definition) = inputs.external_types.definition(owner) {
                result.push(definition.clone());
            } else {
                return Err(HirNativeBoundaryTypeDefinitionError::MissingSourceNominal { owner });
            }
        }
        result.sort_by(|left, right| left.owner().compare_sort_key(right.owner()));
        Ok(Self { records: result })
    }

    pub fn records(&self) -> &[NativeBoundaryTypeDefinitionRecord] {
        &self.records
    }
}
