//! Local declaration signatures introduce their own nonempty binder frame.

use super::super::{DefaultEntityProjectionError, arena_get, raw_index};
use super::DefaultEntityProjector;
use crate::{ExportDefaultTypeTarget, HirSignatureBinder, LocalFunctionId};
use scoop_identity::SignatureTypeKey;

impl DefaultEntityProjector<'_> {
    pub(in super::super) fn reference_type(
        &self,
        target: ExportDefaultTypeTarget,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, DefaultEntityProjectionError> {
        match target {
            ExportDefaultTypeTarget::Type(ty) => self.type_key(ty, binders),
            ExportDefaultTypeTarget::LocalFunctionSignature(id) => {
                self.local_function_signature(id, binders)
            }
        }
    }

    pub(in super::super) fn local_function_signature(
        &self,
        id: LocalFunctionId,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, DefaultEntityProjectionError> {
        let local = arena_get(&self.export.local_functions, id).ok_or(
            DefaultEntityProjectionError::Unknown {
                kind: "local function",
                index: raw_index(id),
            },
        )?;
        let invalid = || DefaultEntityProjectionError::InvalidLocalFunctionBinders {
            local_function: raw_index(id),
        };
        let parameters = match local.definition {
            crate::LexicalFunctionDefinition::Source { function, .. } => {
                let function = arena_get(&self.export.functions, function).ok_or(
                    DefaultEntityProjectionError::Unknown {
                        kind: "function",
                        index: raw_index(function),
                    },
                )?;
                function
                    .type_params()
                    .into_iter()
                    .map(|parameter| parameter.id)
                    .collect::<Vec<_>>()
            }
            crate::LexicalFunctionDefinition::Template(template) => {
                self.export.imported_generic_templates[template]
                    .type_parameters
                    .ids()
            }
        };
        let own = parameters
            .get(local.owner_type_param_count..)
            .ok_or_else(invalid)?;
        let ty = arena_get(&self.export.function_types, local.function_type)
            .ok_or(DefaultEntityProjectionError::Unknown {
                kind: "function type",
                index: raw_index(local.function_type),
            })?
            .canonical_type;
        if own.is_empty() {
            return self.type_key(ty, binders);
        }
        let count = own.len().checked_add(binders.len()).ok_or_else(invalid)?;

        let mut scoped = Vec::with_capacity(count);
        for binder in binders {
            scoped.push(HirSignatureBinder {
                parameter: binder.parameter,
                depth: binder.depth.checked_add(1).ok_or_else(invalid)?,
                index: binder.index,
            });
        }
        for (index, &parameter) in own.iter().enumerate() {
            scoped.push(HirSignatureBinder {
                parameter,
                depth: 0,
                index: u32::try_from(index).map_err(|_| invalid())?,
            });
        }
        self.type_key(ty, &scoped)
    }
}
