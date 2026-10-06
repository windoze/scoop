use std::collections::BTreeMap;

use scoop_identity::CoreBuiltinNominal;

use super::{HirNativeBoundaryTypeDefinitionError, HirNativeBoundaryTypeDefinitionInputs};
use crate::{
    ClassId, EnumId, HirNominalIdentity, HirSourceNominalIdentity, InterfaceId,
    NativeBoundaryNominalOwner, ObjectId, StructId,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum LocalNominalDeclaration {
    CoreBuiltin(CoreBuiltinNominal),
    Struct(StructId),
    Enum(EnumId),
    Class(ClassId),
    Interface(InterfaceId),
    Object(ObjectId),
}

pub(super) struct LocalNominalDeclarations {
    declarations: BTreeMap<NativeBoundaryNominalOwner, LocalNominalDeclaration>,
}

impl LocalNominalDeclarations {
    pub(super) fn new(
        inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    ) -> Result<Self, HirNativeBoundaryTypeDefinitionError> {
        let mut declarations = BTreeMap::new();
        for (id, _) in inputs.structs.iter() {
            insert_source(
                &mut declarations,
                &inputs.nominal_identities[id],
                LocalNominalDeclaration::Struct(id),
            )?;
        }
        for (id, _) in inputs.enums.iter() {
            insert_source(
                &mut declarations,
                &inputs.nominal_identities[id],
                LocalNominalDeclaration::Enum(id),
            )?;
        }
        for (id, _) in inputs.classes.iter() {
            if inputs.nominal_identities[id].source().is_some() {
                insert_source(
                    &mut declarations,
                    &inputs.nominal_identities[id],
                    LocalNominalDeclaration::Class(id),
                )?;
            }
        }
        for (id, _) in inputs.interfaces.iter() {
            insert_source(
                &mut declarations,
                &inputs.nominal_identities[id],
                LocalNominalDeclaration::Interface(id),
            )?;
        }
        for (id, _) in inputs.objects.iter() {
            insert_source(
                &mut declarations,
                &inputs.nominal_identities[id],
                LocalNominalDeclaration::Object(id),
            )?;
        }
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            declarations
                .entry(NativeBoundaryNominalOwner::Concrete(
                    inputs.nominal_identities.core_builtin(builtin).id(),
                ))
                .or_insert(LocalNominalDeclaration::CoreBuiltin(builtin));
        }
        Ok(Self { declarations })
    }

    pub(super) fn get(&self, owner: NativeBoundaryNominalOwner) -> Option<LocalNominalDeclaration> {
        self.declarations.get(&owner).copied()
    }
}

fn insert_source(
    declarations: &mut BTreeMap<NativeBoundaryNominalOwner, LocalNominalDeclaration>,
    identity: &HirNominalIdentity,
    declaration: LocalNominalDeclaration,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    let source = identity
        .source()
        .ok_or(HirNativeBoundaryTypeDefinitionError::UnexpectedGeneratedNominal)?;
    insert(declarations, source_owner(source), declaration)
}

fn insert(
    declarations: &mut BTreeMap<NativeBoundaryNominalOwner, LocalNominalDeclaration>,
    owner: NativeBoundaryNominalOwner,
    declaration: LocalNominalDeclaration,
) -> Result<(), HirNativeBoundaryTypeDefinitionError> {
    if declarations.insert(owner, declaration).is_some() {
        Err(HirNativeBoundaryTypeDefinitionError::DuplicateSourceNominal { owner })
    } else {
        Ok(())
    }
}

fn source_owner(identity: &HirSourceNominalIdentity) -> NativeBoundaryNominalOwner {
    match identity {
        HirSourceNominalIdentity::Concrete(record) => {
            NativeBoundaryNominalOwner::Concrete(record.id())
        }
        HirSourceNominalIdentity::Generic(record) => {
            NativeBoundaryNominalOwner::GenericTemplate(record.id())
        }
    }
}
