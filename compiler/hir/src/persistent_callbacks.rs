//! Persistent identities for source foreign-callback conversion sites.

use std::collections::BTreeMap;
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CallbackMode, CallbackParameterIndex, CallbackRegistrationKey, CborIdentityRecord, Effect,
    PersistentCallbackRegistrationId, SignatureCallableShape, SourceCAbiFunctionSignature,
    SourceCAbiReturn, StructuralDefinitionSiteRole,
};

use crate::{
    AnonymousFunction, ClassConstructor, ForeignCallbackModes, ForeignCallbackRegistration,
    ForeignCallbackRegistrationId, Function, FunctionType, HirConstructorIdentities,
    HirEnumMemberIdentities, HirFunctionIdentities, HirPropertyAccessorIdentities,
    HirSignatureTypeMapper, HirTypeIdentityInputs, Lambda, LocalFunction, StructConstructor, Type,
    TypeId,
};

mod context;
mod error;

use context::CallbackContextResolver;
pub use error::HirCallbackRegistrationIdentityError;

pub type HirCallbackRegistrationIdentity =
    CborIdentityRecord<PersistentCallbackRegistrationId, CallbackRegistrationKey>;

pub struct HirCallbackRegistrationIdentityInputs<'a> {
    pub registrations: &'a Arena<ForeignCallbackRegistration>,
    pub functions: &'a Arena<Function>,
    pub lambdas: &'a Arena<Lambda>,
    pub anonymous_functions: &'a Arena<AnonymousFunction>,
    pub local_functions: &'a Arena<LocalFunction>,
    pub class_constructors: &'a Arena<ClassConstructor>,
    pub struct_constructors: &'a Arena<StructConstructor>,
    pub function_identities: &'a HirFunctionIdentities,
    pub property_accessor_identities: &'a HirPropertyAccessorIdentities,
    pub constructor_identities: &'a HirConstructorIdentities,
    pub enum_member_identities: &'a HirEnumMemberIdentities,
    pub callback_modes: ForeignCallbackModes,
    pub type_inputs: HirTypeIdentityInputs<'a>,
    pub unit: TypeId,
}

/// Total local relation plus the unique canonical callback-registration table.
#[derive(Clone, Debug)]
pub struct HirCallbackRegistrationIdentities {
    identities: Vec<HirCallbackRegistrationIdentity>,
    records: Vec<HirCallbackRegistrationIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportedCoreCallbackIdentityError {
    registration: ForeignCallbackRegistrationId,
}

impl ImportedCoreCallbackIdentityError {
    pub const fn registration(self) -> ForeignCallbackRegistrationId {
        self.registration
    }
}

impl std::fmt::Display for ImportedCoreCallbackIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "imported-core HIR cannot retain a local foreign callback registration target",
        )
    }
}

impl std::error::Error for ImportedCoreCallbackIdentityError {}

impl HirCallbackRegistrationIdentities {
    /// Construct the callback identity relation for an ordinary module whose
    /// core authority is imported. Callback conversions in that domain must
    /// already be represented by imported-core uses rather than by local
    /// callback registrations.
    pub fn for_imported_core(
        registrations: &Arena<ForeignCallbackRegistration>,
    ) -> Result<Self, ImportedCoreCallbackIdentityError> {
        if let Some((registration, _)) = registrations.iter().next() {
            return Err(ImportedCoreCallbackIdentityError { registration });
        }
        Ok(Self {
            identities: Vec::new(),
            records: Vec::new(),
        })
    }

    pub fn from_registrations(
        inputs: HirCallbackRegistrationIdentityInputs<'_>,
    ) -> Result<Self, HirCallbackRegistrationIdentityError> {
        if local_index(inputs.unit) >= inputs.type_inputs.types.len()
            || !matches!(inputs.type_inputs.types[inputs.unit], Type::Unit)
        {
            return Err(HirCallbackRegistrationIdentityError::invalid_unit());
        }

        let mut resolver = CallbackContextResolver::new(&inputs);
        let contexts = inputs
            .registrations
            .iter()
            .map(|(registration, declaration)| {
                resolver
                    .resolve(declaration.definition_root, &declaration.definition_path)
                    .map_err(|detail| {
                        HirCallbackRegistrationIdentityError::new(registration, detail)
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mapper = HirSignatureTypeMapper::new(inputs.type_inputs);
        let mut identities = Vec::with_capacity(inputs.registrations.len());
        let mut sites = BTreeMap::new();

        for ((registration, declaration), context) in inputs.registrations.iter().zip(contexts) {
            let result = build_identity(&inputs, &mapper, registration, declaration, context)?;
            let site = (result.key().parent(), result.key().path().clone());
            if let Some(previous) = sites.insert(site, result.key().clone()) {
                if previous != *result.key() {
                    return Err(HirCallbackRegistrationIdentityError::conflicting_site(
                        registration,
                    ));
                }
            }
            identities.push(result);
        }

        let mut records = identities.clone();
        records.sort_by_key(CborIdentityRecord::id);
        records.dedup_by(|right, left| left == right);
        if let Some(pair) = records.windows(2).find(|pair| pair[0].id() == pair[1].id()) {
            let registration = identities
                .iter()
                .position(|record| record.id() == pair[1].id())
                .map(raw_registration)
                .expect("a duplicate callback identity comes from the local relation");
            return Err(HirCallbackRegistrationIdentityError::duplicate_identity(
                registration,
            ));
        }

        Ok(Self {
            identities,
            records,
        })
    }

    pub fn records(&self) -> &[HirCallbackRegistrationIdentity] {
        &self.records
    }
}

impl Index<ForeignCallbackRegistrationId> for HirCallbackRegistrationIdentities {
    type Output = HirCallbackRegistrationIdentity;

    fn index(&self, id: ForeignCallbackRegistrationId) -> &Self::Output {
        &self.identities[local_index(id)]
    }
}

fn build_identity(
    inputs: &HirCallbackRegistrationIdentityInputs<'_>,
    mapper: &HirSignatureTypeMapper<'_>,
    registration: ForeignCallbackRegistrationId,
    declaration: &ForeignCallbackRegistration,
    context: context::CallbackSiteContext,
) -> Result<HirCallbackRegistrationIdentity, HirCallbackRegistrationIdentityError> {
    if declaration
        .definition_path
        .segments()
        .last()
        .is_none_or(|segment| {
            segment.site_role() != StructuralDefinitionSiteRole::CallbackConversion
        })
    {
        return Err(HirCallbackRegistrationIdentityError::invalid_path(
            registration,
        ));
    }
    let native = function_type(inputs, registration, declaration.native_function_type)?;
    let managed = function_type(inputs, registration, declaration.managed_function_type)?;
    let context_index = usize::try_from(declaration.context_index)
        .map_err(|_| HirCallbackRegistrationIdentityError::invalid_context_index(registration))?;
    if context_index >= native.parameter_types.len() {
        return Err(HirCallbackRegistrationIdentityError::invalid_context_index(
            registration,
        ));
    }
    if native.is_suspend || managed.is_suspend || native.parameter_types.contains(&inputs.unit) {
        return Err(HirCallbackRegistrationIdentityError::invalid_native_signature(registration));
    }
    if native.return_type != managed.return_type
        || native
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != context_index).then_some(ty))
            .ne(managed.parameter_types.iter())
    {
        return Err(HirCallbackRegistrationIdentityError::signature_relation(
            registration,
        ));
    }

    let map = |ty| {
        mapper.map(ty, &context.binders).map_err(|error| {
            HirCallbackRegistrationIdentityError::signature_type(registration, error)
        })
    };
    let native_parameters = native
        .parameter_types
        .iter()
        .copied()
        .map(map)
        .collect::<Result<Vec<_>, _>>()?;
    let native_result = map(native.return_type)?;
    let source_signature = SourceCAbiFunctionSignature::new(
        native_parameters,
        if native.return_type == inputs.unit {
            SourceCAbiReturn::Void
        } else {
            SourceCAbiReturn::Value(native_result)
        },
    );
    let managed_parameters = managed
        .parameter_types
        .iter()
        .copied()
        .map(map)
        .collect::<Result<Vec<_>, _>>()?;
    let managed_signature = SignatureCallableShape::new(
        if managed.is_suspend {
            Effect::Suspend
        } else {
            Effect::Ordinary
        },
        None,
        managed_parameters,
        map(managed.return_type)?,
    );
    let mode = if declaration.mode == inputs.callback_modes.reusable() {
        CallbackMode::Reusable
    } else if declaration.mode == inputs.callback_modes.one_shot() {
        CallbackMode::OneShot
    } else {
        return Err(HirCallbackRegistrationIdentityError::invalid_mode(
            registration,
        ));
    };
    CborIdentityRecord::from_key(CallbackRegistrationKey::new(
        context.parent,
        declaration.definition_path.clone(),
        source_signature,
        CallbackParameterIndex::new(declaration.context_index),
        managed_signature,
        mode,
    ))
    .map_err(|error| HirCallbackRegistrationIdentityError::identity(registration, error))
}

fn function_type<'a>(
    inputs: &'a HirCallbackRegistrationIdentityInputs<'_>,
    registration: ForeignCallbackRegistrationId,
    id: crate::FunctionTypeId,
) -> Result<&'a FunctionType, HirCallbackRegistrationIdentityError> {
    if local_index(id) >= inputs.type_inputs.function_types.len() {
        return Err(HirCallbackRegistrationIdentityError::invalid_function_type(
            registration,
        ));
    }
    let function = &inputs.type_inputs.function_types[id];
    if local_index(function.canonical_type) >= inputs.type_inputs.types.len()
        || !matches!(inputs.type_inputs.types[function.canonical_type], Type::Function(found) if found == id)
    {
        return Err(HirCallbackRegistrationIdentityError::invalid_function_type(
            registration,
        ));
    }
    Ok(function)
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_registration(index: usize) -> ForeignCallbackRegistrationId {
    ForeignCallbackRegistrationId::from_raw((index as u32).into())
}
