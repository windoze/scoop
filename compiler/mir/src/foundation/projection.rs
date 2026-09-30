use std::collections::BTreeMap;

use scoop_identity::{CborIdentityRecord, PersistentId};

use super::*;
use crate::{ExactOwnerRoot, LocalValueIdentityAuthority, Module};

impl CanonicalMirFoundation {
    /// Build the complete MIR identity delta from a validated module.
    pub fn from_module(module: &Module) -> Result<Self, MirFoundationBuildError> {
        module
            .validate()
            .map_err(|error| MirFoundationBuildError::InvalidModule(Box::new(error)))?;

        let mut foundation = Self::empty();
        foundation.set_exact_types(
            module
                .meta
                .generated_exact_types
                .iter()
                .map(|entry| entry.exact_record().clone())
                .collect(),
        )?;
        foundation.set_generated_callables(
            module
                .meta
                .generated_callables
                .iter()
                .map(|entry| entry.identity_record().clone())
                .collect(),
        )?;
        foundation.set_generated_types(
            module
                .meta
                .generated_exact_types
                .iter()
                .map(|entry| entry.nominal_record().clone())
                .collect(),
        )?;
        foundation.set_fields(field_records(module)?)?;
        foundation.set_enum_variants(enum_variant_records(module)?)?;
        foundation.set_enum_variant_fields(enum_variant_field_records(module)?)?;
        foundation
            .set_callable_signatures(module.meta.callable_signatures.iter().cloned().collect())?;
        foundation.set_local_values(local_value_records(module)?)?;
        foundation.set_callback_applications(
            module
                .foreign_callback_bridges
                .iter()
                .map(|(_, bridge)| bridge.application_identity.clone())
                .collect(),
        )?;
        foundation.set_callback_application_records(
            module
                .foreign_callback_bridges
                .iter()
                .map(|(_, bridge)| bridge.application_record.clone())
                .collect(),
        )?;
        let (groups, members) = odr_records(module)?;
        foundation.set_odr_groups(groups)?;
        foundation.set_odr_members(members)?;
        Ok(foundation)
    }
}

fn field_records(module: &Module) -> Result<Vec<FieldRecord>, MirFoundationBuildError> {
    let mut records = BTreeMap::new();
    for environment in &module.meta.closure_environments {
        for field in environment.identity().fields() {
            insert_identity(
                &mut records,
                field.field_record(),
                MirFoundationTable::Field,
            )?;
        }
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        insert_identity(
            &mut records,
            adapter.identity().source_field_record(),
            MirFoundationTable::Field,
        )?;
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        insert_identity(
            &mut records,
            adapter.identity().source_field_record(),
            MirFoundationTable::Field,
        )?;
    }
    for (_, frame) in module.meta.coroutine_frames.iter() {
        let identity = frame.identity();
        for field in [
            identity.state_field_record(),
            identity.completion_field_record(),
            identity.failure_field_record(),
        ] {
            insert_identity(&mut records, field, MirFoundationTable::Field)?;
        }
        for saved in identity.saved_fields() {
            insert_identity(
                &mut records,
                saved.field_record(),
                MirFoundationTable::Field,
            )?;
        }
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        let identity = point.identity();
        insert_identity(
            &mut records,
            identity.frame_field_record(),
            MirFoundationTable::Field,
        )?;
        insert_identity(
            &mut records,
            identity.state_field_record(),
            MirFoundationTable::Field,
        )?;
        if let Some(field) = identity.storage().result_field_record() {
            insert_identity(&mut records, field, MirFoundationTable::Field)?;
        }
        if let Some(field) = identity.storage().failure_field_record() {
            insert_identity(&mut records, field, MirFoundationTable::Field)?;
        }
    }
    for boxed in &module.meta.boxed_types {
        insert_identity(
            &mut records,
            boxed.identity().payload_field_record(),
            MirFoundationTable::Field,
        )?;
    }
    Ok(records.into_values().collect())
}

fn enum_variant_records(
    module: &Module,
) -> Result<Vec<EnumVariantRecord>, MirFoundationBuildError> {
    let mut records = BTreeMap::new();
    for (_, step) in module.meta.coroutine_steps.iter() {
        for record in [
            step.identity().completed_variant_record(),
            step.identity().suspended_variant_record(),
        ] {
            insert_identity(&mut records, record, MirFoundationTable::EnumVariant)?;
        }
    }
    for (_, slot) in module.meta.coroutine_slots.iter() {
        for record in [
            slot.identity().empty_variant_record(),
            slot.identity().value_variant_record(),
        ] {
            insert_identity(&mut records, record, MirFoundationTable::EnumVariant)?;
        }
    }
    Ok(records.into_values().collect())
}

fn enum_variant_field_records(
    module: &Module,
) -> Result<Vec<EnumVariantFieldRecord>, MirFoundationBuildError> {
    let mut records = BTreeMap::new();
    for (_, step) in module.meta.coroutine_steps.iter() {
        insert_identity(
            &mut records,
            step.identity().completed_payload_record(),
            MirFoundationTable::EnumVariantField,
        )?;
    }
    for (_, slot) in module.meta.coroutine_slots.iter() {
        insert_identity(
            &mut records,
            slot.identity().value_payload_record(),
            MirFoundationTable::EnumVariantField,
        )?;
    }
    Ok(records.into_values().collect())
}

fn local_value_records(module: &Module) -> Result<Vec<LocalValueRecord>, MirFoundationBuildError> {
    let mut records = BTreeMap::new();
    for entry in module
        .meta
        .local_values
        .iter()
        .filter(|entry| entry.authority() == LocalValueIdentityAuthority::Mir)
    {
        insert_identity(
            &mut records,
            entry.identity_record(),
            MirFoundationTable::LocalValue,
        )?;
    }
    Ok(records.into_values().collect())
}

fn odr_records(
    module: &Module,
) -> Result<(Vec<OdrGroupRecord>, Vec<OdrMemberRecord>), MirFoundationBuildError> {
    let mut groups = BTreeMap::new();
    let mut members = BTreeMap::new();

    for source in module.meta.source_callable_materializations.iter() {
        if let Some(owner) = source.exact_owner() {
            register_exact_root(owner, &mut groups, &mut members)?;
        }
        if matches!(
            source.materialization().context(),
            scoop_identity::CallableMaterializationContext::InitializationApplication(_)
        ) {
            insert_optional_identity(
                &mut members,
                source.odr_member_record(),
                MirFoundationTable::OdrMember,
            )?;
        }
    }
    for environment in &module.meta.closure_environments {
        insert_optional_identity(
            &mut members,
            environment.identity().odr_member_record(),
            MirFoundationTable::OdrMember,
        )?;
    }
    for (_, bridge) in module.callback_bridges.iter() {
        let Some((_, _)) = bridge.local_definition() else {
            continue;
        };
        insert_optional_identity(
            &mut members,
            bridge.identity().odr_member_record(),
            MirFoundationTable::OdrMember,
        )?;
    }
    for (_, adapter) in module.foreign_callback_adapters.iter() {
        insert_optional_identity(
            &mut members,
            adapter.odr_member_record(),
            MirFoundationTable::OdrMember,
        )?;
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        register_function_adapter(adapter.identity(), &mut groups, &mut members)?;
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        register_function_adapter(adapter.identity(), &mut groups, &mut members)?;
    }
    for bridge in &module.meta.function_bridges {
        insert_optional_identity(
            &mut members,
            bridge.identity().odr_member_record(),
            MirFoundationTable::OdrMember,
        )?;
    }
    for boxed in &module.meta.boxed_types {
        register_exact_root(boxed.identity().root(), &mut groups, &mut members)?;
    }
    for (_, step) in module.meta.coroutine_steps.iter() {
        register_exact_root(step.identity().root(), &mut groups, &mut members)?;
    }
    for (_, slot) in module.meta.coroutine_slots.iter() {
        register_exact_root(slot.identity().root(), &mut groups, &mut members)?;
    }
    for shell in &module.meta.continuation_shells {
        register_exact_root(shell.identity().success_root(), &mut groups, &mut members)?;
        register_exact_root(shell.identity().failure_root(), &mut groups, &mut members)?;
    }
    for start in &module.meta.coroutine_starts {
        register_exact_root(start.identity().root(), &mut groups, &mut members)?;
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        if let crate::CoroutineLowering::StateMachine {
            driver_identity, ..
        } = &coroutine.lowering
        {
            insert_optional_identity(
                &mut members,
                driver_identity.odr_member_record(),
                MirFoundationTable::OdrMember,
            )?;
        }
    }
    for (_, frame) in module.meta.coroutine_frames.iter() {
        insert_optional_identity(
            &mut members,
            frame.identity().odr_member_record(),
            MirFoundationTable::OdrMember,
        )?;
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        let identity = point.identity();
        for member in [
            identity.odr_member_record(),
            identity.success().odr_member_record(),
            identity.failure().odr_member_record(),
        ] {
            insert_optional_identity(&mut members, member, MirFoundationTable::OdrMember)?;
        }
    }
    for adjust in &module.meta.boxing_adjusts {
        register_exact_root(adjust.identity().root(), &mut groups, &mut members)?;
    }

    Ok((
        groups.into_values().collect(),
        members.into_values().collect(),
    ))
}

fn register_function_adapter(
    identity: &crate::FunctionAdapterIdentity,
    groups: &mut BTreeMap<OdrGroupId, OdrGroupRecord>,
    members: &mut BTreeMap<OdrMemberId, OdrMemberRecord>,
) -> Result<(), MirFoundationBuildError> {
    insert_identity(
        groups,
        identity.odr_group_record(),
        MirFoundationTable::OdrGroup,
    )?;
    insert_identity(
        members,
        identity.environment_member_record(),
        MirFoundationTable::OdrMember,
    )?;
    insert_identity(
        members,
        identity.callable_member_record(),
        MirFoundationTable::OdrMember,
    )
}

fn register_exact_root(
    root: &ExactOwnerRoot,
    groups: &mut BTreeMap<OdrGroupId, OdrGroupRecord>,
    members: &mut BTreeMap<OdrMemberId, OdrMemberRecord>,
) -> Result<(), MirFoundationBuildError> {
    insert_optional_identity(
        groups,
        root.mir_odr_group_record(),
        MirFoundationTable::OdrGroup,
    )?;
    insert_optional_identity(members, root.member_record(), MirFoundationTable::OdrMember)
}

fn insert_optional_identity<I, K>(
    records: &mut BTreeMap<I, CborIdentityRecord<I, K>>,
    record: Option<&CborIdentityRecord<I, K>>,
    table: MirFoundationTable,
) -> Result<(), MirFoundationBuildError>
where
    I: Copy + Ord + PersistentId,
    K: Clone + Eq,
{
    if let Some(record) = record {
        insert_identity(records, record, table)?;
    }
    Ok(())
}

fn insert_identity<I, K>(
    records: &mut BTreeMap<I, CborIdentityRecord<I, K>>,
    record: &CborIdentityRecord<I, K>,
    table: MirFoundationTable,
) -> Result<(), MirFoundationBuildError>
where
    I: Copy + Ord + PersistentId,
    K: Clone + Eq,
{
    match records.entry(record.id()) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(record.clone());
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == record => Ok(()),
        std::collections::btree_map::Entry::Occupied(entry) => {
            Err(MirFoundationBuildError::IdentityCollision {
                table,
                identity: *entry.key().as_array(),
            })
        }
    }
}
