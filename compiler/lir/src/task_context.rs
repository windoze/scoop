//! Context key cells are associated atoms of their existing callable body.

pub use scoop_identity::ContextKey;

use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableContextKeyCellV1 {
    pub key: ContextKey,
    pub atom: ObjectDefinitionAtomId,
}

pub fn context_key_cell_atom(
    plan: ObjectDefinitionPlanId,
    key: ContextKey,
) -> Result<ObjectDefinitionAtomId, scoop_wire::HashError> {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::ContextKeyCell,
        DefinitionAtomSubkey::ExactType(key.0),
    ))
}

pub fn context_key_table_atom(
    plan: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, scoop_wire::HashError> {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::ContextKeyTable,
        DefinitionAtomSubkey::Singleton,
    ))
}

pub fn function_context_keys(function: &crate::Function) -> Vec<ContextKey> {
    let mut keys = std::collections::BTreeSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            let arguments = match instruction {
                crate::Instruction::Call { site } => site.args(),
                crate::Instruction::Invoke { site } => site.args(),
                _ => continue,
            };
            for argument in arguments {
                if let crate::AbiCallArgument::Direct(crate::Value::ContextKeyCell(key)) = argument
                {
                    keys.insert(*key);
                }
            }
        }
    }
    keys.into_iter().collect()
}

pub(crate) fn validate_context_plan(
    foundation: &crate::ConeLirFoundation,
    plan: ObjectDefinitionPlanId,
    callable: &crate::StrongCallableRuntimeScanPlanV1,
) -> Result<(), crate::StrongCallableRuntimeScanPlanError> {
    let invalid = || crate::StrongCallableRuntimeScanPlanError::InvalidContextKeys(callable.body());
    let keys = callable.context_keys();
    if !keys.windows(2).all(|pair| pair[0].key < pair[1].key) {
        return Err(invalid());
    }
    let mut expected = std::collections::BTreeSet::new();
    for cell in keys {
        if context_key_cell_atom(plan, cell.key)
            .map_err(crate::StrongCallableRuntimeScanPlanError::Hash)?
            != cell.atom
        {
            return Err(invalid());
        }
        expected.insert(cell.atom);
    }
    if !keys.is_empty() {
        expected.insert(
            context_key_table_atom(plan)
                .map_err(crate::StrongCallableRuntimeScanPlanError::Hash)?,
        );
    }
    let actual = foundation
        .definition_atoms()
        .iter()
        .filter(|atom| {
            atom.key().plan() == plan
                && matches!(
                    atom.key().role(),
                    DefinitionAtomRole::ContextKeyCell | DefinitionAtomRole::ContextKeyTable
                )
        })
        .map(|atom| atom.id())
        .collect();
    if expected != actual {
        return Err(invalid());
    }
    Ok(())
}
