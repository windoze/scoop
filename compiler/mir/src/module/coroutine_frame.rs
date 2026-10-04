//! Typed roles of a resumable frame, including its owning task root.

use super::*;

#[derive(Debug)]
pub struct CoroutineFrame {
    class: ClassId,
    owner: CoroutineFunctionId,
    state: CoroutineFrameFieldRef,
    completion: CoroutineFrameFieldRef,
    task: CoroutineFrameFieldRef,
    saved_values: Vec<CoroutineSavedValueId>,
    failure: CoroutineFailureValueId,
    identity: Box<CoroutineFrameIdentity>,
}

impl CoroutineFrame {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        classes: &Arena<ClassDef>,
        saved: &Arena<CoroutineSavedValue>,
        failures: &Arena<CoroutineFailureValue>,
        class: ClassId,
        owner: CoroutineFunctionId,
        state: CoroutineFrameFieldRef,
        completion: CoroutineFrameFieldRef,
        task: CoroutineFrameFieldRef,
        saved_values: Vec<CoroutineSavedValueId>,
        failure: CoroutineFailureValueId,
        identity: CoroutineFrameIdentity,
    ) -> Option<Self> {
        if state.class() != class
            || completion.class() != class
            || task.class() != class
            || !matches!(task.definition(classes)?.ty, Type::Context(storage)
                if storage.role == ContextStorageRole::Task)
            || state.definition(classes)?.ty
                != Type::MachineScalar(MachineScalarKind::CoroutineFrameState)
        {
            return None;
        }
        let failure_metadata = arena_get(failures, failure)?;
        if failure_metadata.field().class() != class {
            return None;
        }
        let mut fields = vec![
            state.field_index(),
            completion.field_index(),
            task.field_index(),
        ];
        for value in &saved_values {
            let metadata = arena_get(saved, *value)?;
            if metadata.field().class() != class {
                return None;
            }
            fields.push(metadata.field().field_index());
        }
        fields.push(failure_metadata.field().field_index());
        fields.sort_unstable();
        if fields.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        if identity.saved_fields().len() != saved_values.len()
            || state.field_index() != 0
            || completion.field_index() != 1
            || task.field_index() != 2
            || saved_values.iter().enumerate().any(|(index, value)| {
                arena_get(saved, *value).is_none_or(|metadata| {
                    metadata.field().field_index()
                        != u32::try_from(index).ok().unwrap_or(u32::MAX) + 3
                })
            })
            || failure_metadata.field().field_index()
                != u32::try_from(saved_values.len()).ok()?.checked_add(3)?
        {
            return None;
        }
        Some(Self {
            class,
            owner,
            state,
            completion,
            task,
            saved_values,
            failure,
            identity: Box::new(identity),
        })
    }

    pub const fn class(&self) -> ClassId {
        self.class
    }

    pub const fn owner(&self) -> CoroutineFunctionId {
        self.owner
    }

    pub const fn state(&self) -> CoroutineFrameFieldRef {
        self.state
    }

    pub const fn completion(&self) -> CoroutineFrameFieldRef {
        self.completion
    }

    pub const fn task(&self) -> CoroutineFrameFieldRef {
        self.task
    }

    pub fn saved_values(&self) -> &[CoroutineSavedValueId] {
        &self.saved_values
    }

    pub const fn failure(&self) -> CoroutineFailureValueId {
        self.failure
    }

    pub const fn identity(&self) -> &CoroutineFrameIdentity {
        &self.identity
    }

    pub fn owns_saved_value(&self, value: CoroutineSavedValueId) -> bool {
        self.saved_values.contains(&value)
    }
}
