use la_arena::Arena;
use scoop_identity::{
    CallableInstantiationOwner, CallableMaterialization, CallableMaterializationContext,
    CborIdentityRecord, DefinitionOriginRecord, LocalValueKey, LocalValueSelector,
    PersistentLocalValueId,
};
use std::collections::{BTreeMap, HashMap, HashSet};

use super::{
    AnonymousFunction, AnonymousFunctionId, BindingId, CallableApplicationIdentities,
    CallableReference, CallableReferenceId, CallableReferenceTarget, Capture, ClassConstructor,
    ClassConstructorId, ClassConstructorKind, Function, FunctionId, FunctionKind, Lambda, LambdaId,
    LocalFunction, LocalId, StructConstructor, StructConstructorId, StructConstructorKind,
};

mod error;
pub use error::LocalValueIdentityError;
mod definition_origins;
pub use definition_origins::LocalValueDefinitionOrigins;

pub type LocalValueIdentityRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;

#[derive(Clone, Copy)]
pub struct LocalValueIdentityInputs<'a> {
    pub source_files: &'a [crate::SourceFileMetadata],
    pub source_contexts: &'a crate::HirSourceContextIdentities,
    pub callable_applications: &'a CallableApplicationIdentities,
    pub functions: &'a Arena<Function>,
    pub lambdas: &'a Arena<Lambda>,
    pub anonymous_functions: &'a Arena<AnonymousFunction>,
    pub local_functions: &'a Arena<LocalFunction>,
    pub callable_references: &'a Arena<CallableReference>,
    pub class_constructors: &'a Arena<ClassConstructor>,
    pub struct_constructors: &'a Arena<StructConstructor>,
}

/// Total persistent identity relation for values owned by LocalConcrete HIR
/// callables. Captured local-function ABI parameters map to the original
/// value identity instead of introducing a second semantic value.
#[derive(Clone, Debug)]
pub struct LocalValueIdentities {
    records: Vec<LocalValueIdentityRecord>,
    definition_origins: LocalValueDefinitionOrigins,
    function_locals: Vec<Vec<PersistentLocalValueId>>,
    lambda_captures: Vec<Vec<PersistentLocalValueId>>,
    anonymous_function_captures: Vec<Vec<PersistentLocalValueId>>,
    callable_reference_captures: Vec<Vec<PersistentLocalValueId>>,
    callable_references: Vec<CallableReferenceLocalValue>,
    class_constructors: Vec<ClassConstructorLocalValues>,
    struct_constructors: Vec<StructConstructorLocalValues>,
}

#[derive(Clone, Debug)]
struct ClassConstructorLocalValues {
    receiver: PersistentLocalValueId,
    parameters: Vec<PersistentLocalValueId>,
    locals: Vec<PersistentLocalValueId>,
}

#[derive(Clone, Debug)]
struct StructConstructorLocalValues {
    parameters: Vec<PersistentLocalValueId>,
    kind: StructConstructorLocalValueKind,
}

#[derive(Clone, Debug)]
enum StructConstructorLocalValueKind {
    Primary,
    Secondary {
        receiver: PersistentLocalValueId,
        argument_locals: Vec<PersistentLocalValueId>,
        body_locals: Vec<PersistentLocalValueId>,
    },
}

#[derive(Clone, Debug)]
enum CallableReferenceLocalValue {
    Unbound,
    Bound(PersistentLocalValueId),
}

impl LocalValueIdentities {
    pub fn from_callables(
        inputs: LocalValueIdentityInputs<'_>,
    ) -> Result<Self, LocalValueIdentityError> {
        LocalValueIdentityBuilder::new(inputs).build()
    }

    pub fn records(&self) -> &[LocalValueIdentityRecord] {
        &self.records
    }

    pub const fn definition_origins(&self) -> &LocalValueDefinitionOrigins {
        &self.definition_origins
    }

    pub fn record(&self, identity: PersistentLocalValueId) -> Option<&LocalValueIdentityRecord> {
        self.records
            .binary_search_by_key(&identity, CborIdentityRecord::id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn function_local(
        &self,
        function: FunctionId,
        local: LocalId,
    ) -> &LocalValueIdentityRecord {
        let identity = self.function_locals[arena_index(function)][arena_index(local)];
        self.record(identity)
            .expect("the total function-local relation references a canonical record")
    }

    pub fn class_receiver(&self, constructor: ClassConstructorId) -> &LocalValueIdentityRecord {
        let identity = self.class_constructors[arena_index(constructor)].receiver;
        self.record(identity)
            .expect("the total class-receiver relation references a canonical record")
    }

    pub fn callable_reference_receiver(
        &self,
        reference: CallableReferenceId,
    ) -> Option<&LocalValueIdentityRecord> {
        match self.callable_references[arena_index(reference)] {
            CallableReferenceLocalValue::Unbound => None,
            CallableReferenceLocalValue::Bound(identity) => self.record(identity),
        }
    }

    pub fn lambda_capture(&self, lambda: LambdaId, capture: usize) -> &LocalValueIdentityRecord {
        self.capture_record(self.lambda_captures[arena_index(lambda)][capture])
    }

    pub fn anonymous_function_capture(
        &self,
        function: AnonymousFunctionId,
        capture: usize,
    ) -> &LocalValueIdentityRecord {
        self.capture_record(self.anonymous_function_captures[arena_index(function)][capture])
    }

    pub fn callable_reference_capture(
        &self,
        reference: CallableReferenceId,
        capture: usize,
    ) -> &LocalValueIdentityRecord {
        self.capture_record(self.callable_reference_captures[arena_index(reference)][capture])
    }

    pub fn class_parameter(
        &self,
        constructor: ClassConstructorId,
        declaration_index: usize,
    ) -> &LocalValueIdentityRecord {
        let identity =
            self.class_constructors[arena_index(constructor)].parameters[declaration_index];
        self.record(identity)
            .expect("the total class-parameter relation references a canonical record")
    }

    pub fn class_local(
        &self,
        constructor: ClassConstructorId,
        local: LocalId,
    ) -> &LocalValueIdentityRecord {
        let identity = self.class_constructors[arena_index(constructor)].locals[arena_index(local)];
        self.record(identity)
            .expect("the total class-local relation references a canonical record")
    }

    pub fn struct_parameter(
        &self,
        constructor: StructConstructorId,
        declaration_index: usize,
    ) -> &LocalValueIdentityRecord {
        let identity =
            self.struct_constructors[arena_index(constructor)].parameters[declaration_index];
        self.record(identity)
            .expect("the total struct-parameter relation references a canonical record")
    }

    pub fn struct_receiver(
        &self,
        constructor: StructConstructorId,
    ) -> Option<&LocalValueIdentityRecord> {
        match &self.struct_constructors[arena_index(constructor)].kind {
            StructConstructorLocalValueKind::Primary => None,
            StructConstructorLocalValueKind::Secondary { receiver, .. } => self.record(*receiver),
        }
    }

    pub fn struct_argument_local(
        &self,
        constructor: StructConstructorId,
        local: LocalId,
    ) -> Option<&LocalValueIdentityRecord> {
        match &self.struct_constructors[arena_index(constructor)].kind {
            StructConstructorLocalValueKind::Primary => None,
            StructConstructorLocalValueKind::Secondary {
                argument_locals, ..
            } => self.record(argument_locals[arena_index(local)]),
        }
    }

    pub fn struct_body_local(
        &self,
        constructor: StructConstructorId,
        local: LocalId,
    ) -> Option<&LocalValueIdentityRecord> {
        match &self.struct_constructors[arena_index(constructor)].kind {
            StructConstructorLocalValueKind::Primary => None,
            StructConstructorLocalValueKind::Secondary { body_locals, .. } => {
                self.record(body_locals[arena_index(local)])
            }
        }
    }

    fn capture_record(&self, identity: PersistentLocalValueId) -> &LocalValueIdentityRecord {
        self.record(identity)
            .expect("the total capture relation references a canonical local-value record")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureOwnerLocation {
    Lambda(u32),
    AnonymousFunction(u32),
    CallableReference(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalValueLocation {
    FunctionLocal { function: u32, local: u32 },
    ClassReceiver { constructor: u32 },
    ClassParameter { constructor: u32, parameter: u32 },
    ClassLocal { constructor: u32, local: u32 },
    StructParameter { constructor: u32, parameter: u32 },
    StructReceiver { constructor: u32 },
    StructArgumentLocal { constructor: u32, local: u32 },
    StructBodyLocal { constructor: u32, local: u32 },
    CallableReferenceReceiver { reference: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct BindingKey {
    context: CallableMaterializationContext,
    binding: BindingId,
}

#[derive(Clone, Copy)]
struct CaptureAlias {
    local_function: u32,
    capture: u32,
    binding: BindingId,
}

struct LocalValueIdentityBuilder<'a> {
    inputs: LocalValueIdentityInputs<'a>,
    records: BTreeMap<PersistentLocalValueId, (LocalValueIdentityRecord, LocalValueLocation)>,
    definition_origins: BTreeMap<PersistentLocalValueId, DefinitionOriginRecord>,
    locations_by_key: BTreeMap<LocalValueKey, LocalValueLocation>,
    values_by_binding: HashMap<BindingKey, Vec<PersistentLocalValueId>>,
    capture_aliases: HashMap<(FunctionId, LocalId), CaptureAlias>,
}

impl<'a> LocalValueIdentityBuilder<'a> {
    fn new(inputs: LocalValueIdentityInputs<'a>) -> Self {
        Self {
            inputs,
            records: BTreeMap::new(),
            definition_origins: BTreeMap::new(),
            locations_by_key: BTreeMap::new(),
            values_by_binding: HashMap::new(),
            capture_aliases: HashMap::new(),
        }
    }

    fn build(mut self) -> Result<LocalValueIdentities, LocalValueIdentityError> {
        self.collect_capture_aliases()?;

        let mut function_locals = Vec::with_capacity(self.inputs.functions.len());
        for (function_id, function) in self.inputs.functions.iter() {
            let FunctionKind::User(body) = &function.kind else {
                function_locals.push(Vec::new());
                continue;
            };
            let mut identities = vec![None; body.locals.len()];
            for (local_id, local) in body.locals.iter() {
                if self.capture_aliases.contains_key(&(function_id, local_id)) {
                    continue;
                }
                let location = LocalValueLocation::FunctionLocal {
                    function: raw_arena_index(function_id),
                    local: raw_arena_index(local_id),
                };
                let identity = self.record(
                    function.materialization,
                    local.selector.clone(),
                    &local.definition,
                    location,
                )?;
                self.bind(function.materialization.context(), local.binding, identity);
                identities[arena_index(local_id)] = Some(identity);
            }
            function_locals.push(identities);
        }

        let class_constructors = self.collect_class_constructors()?;
        let struct_constructors = self.collect_struct_constructors()?;
        let callable_references = self.collect_callable_references()?;

        for ((function, local), alias) in &self.capture_aliases {
            let context = self.inputs.functions[*function].materialization.context();
            let Some(identity) = self.find_captured_value(context, alias.binding) else {
                return Err(LocalValueIdentityError::MissingCapturedValue {
                    local_function: alias.local_function,
                    capture: alias.capture,
                    binding: alias.binding.into_raw(),
                });
            };
            function_locals[arena_index(*function)][arena_index(*local)] = Some(identity);
        }

        let lambda_captures = self
            .inputs
            .lambdas
            .iter()
            .map(|(lambda, declaration)| {
                self.collect_captures(
                    self.inputs.functions[declaration.function]
                        .materialization
                        .context(),
                    &declaration.captures,
                    CaptureOwnerLocation::Lambda(raw_arena_index(lambda)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let anonymous_function_captures = self
            .inputs
            .anonymous_functions
            .iter()
            .map(|(function, declaration)| {
                self.collect_captures(
                    self.inputs.functions[declaration.function]
                        .materialization
                        .context(),
                    &declaration.captures,
                    CaptureOwnerLocation::AnonymousFunction(raw_arena_index(function)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let callable_reference_captures = self
            .inputs
            .callable_references
            .iter()
            .map(|(reference, declaration)| {
                self.collect_captures(
                    declaration.identity.materialization().context(),
                    &declaration.captures,
                    CaptureOwnerLocation::CallableReference(raw_arena_index(reference)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let function_locals = function_locals
            .into_iter()
            .enumerate()
            .map(|(function, locals)| {
                locals
                    .into_iter()
                    .enumerate()
                    .map(|(local, identity)| {
                        identity.ok_or(LocalValueIdentityError::MissingFunctionLocal {
                            function: function as u32,
                            local: local as u32,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(LocalValueIdentities {
            records: self
                .records
                .into_values()
                .map(|(record, _)| record)
                .collect(),
            definition_origins: LocalValueDefinitionOrigins {
                records: self.definition_origins.into_values().collect(),
            },
            function_locals,
            lambda_captures,
            anonymous_function_captures,
            callable_reference_captures,
            callable_references,
            class_constructors,
            struct_constructors,
        })
    }

    fn collect_capture_aliases(&mut self) -> Result<(), LocalValueIdentityError> {
        for (local_function_id, local_function) in self.inputs.local_functions.iter() {
            let function = &self.inputs.functions[local_function.function];
            let FunctionKind::User(body) = &function.kind else {
                return Err(LocalValueIdentityError::LocalFunctionRequiresBody {
                    local_function: raw_arena_index(local_function_id),
                    function: raw_arena_index(local_function.function),
                });
            };
            for (capture_index, capture) in local_function.captures.iter().enumerate() {
                let Some(parameter) = function.params.get(capture_index) else {
                    return Err(LocalValueIdentityError::MissingCaptureParameter {
                        local_function: raw_arena_index(local_function_id),
                        capture: capture_index as u32,
                    });
                };
                if arena_index(parameter.local) >= body.locals.len() {
                    return Err(LocalValueIdentityError::MissingCaptureParameter {
                        local_function: raw_arena_index(local_function_id),
                        capture: capture_index as u32,
                    });
                }
                if self
                    .capture_aliases
                    .insert(
                        (local_function.function, parameter.local),
                        CaptureAlias {
                            local_function: raw_arena_index(local_function_id),
                            capture: capture_index as u32,
                            binding: capture.binding,
                        },
                    )
                    .is_some()
                {
                    return Err(LocalValueIdentityError::DuplicateCaptureParameter {
                        function: raw_arena_index(local_function.function),
                        local: raw_arena_index(parameter.local),
                    });
                }
            }
        }
        Ok(())
    }

    fn collect_class_constructors(
        &mut self,
    ) -> Result<Vec<ClassConstructorLocalValues>, LocalValueIdentityError> {
        let mut output = Vec::with_capacity(self.inputs.class_constructors.len());
        for (constructor_id, constructor) in self.inputs.class_constructors.iter() {
            let owner = constructor.materialization;
            let receiver_location = LocalValueLocation::ClassReceiver {
                constructor: raw_arena_index(constructor_id),
            };
            let receiver = self.record_source(
                owner,
                LocalValueSelector::This,
                constructor.origin,
                receiver_location,
            )?;
            let parameters = constructor
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let location = LocalValueLocation::ClassParameter {
                        constructor: raw_arena_index(constructor_id),
                        parameter: index as u32,
                    };
                    let identity = self.record_source(
                        owner,
                        LocalValueSelector::Parameter {
                            declaration_index: index as u32,
                        },
                        parameter.definition,
                        location,
                    )?;
                    self.bind(owner.context(), parameter.binding, identity);
                    Ok(identity)
                })
                .collect::<Result<Vec<_>, LocalValueIdentityError>>()?;
            let body = match &constructor.kind {
                ClassConstructorKind::This { body, .. }
                | ClassConstructorKind::Terminal { body } => body,
            };
            let locals = self.collect_locals(
                owner,
                body.locals.iter().map(|(local, value)| {
                    (
                        local,
                        value,
                        LocalValueLocation::ClassLocal {
                            constructor: raw_arena_index(constructor_id),
                            local: raw_arena_index(local),
                        },
                    )
                }),
            )?;
            output.push(ClassConstructorLocalValues {
                receiver,
                parameters,
                locals,
            });
        }
        Ok(output)
    }

    fn collect_struct_constructors(
        &mut self,
    ) -> Result<Vec<StructConstructorLocalValues>, LocalValueIdentityError> {
        let mut output = Vec::with_capacity(self.inputs.struct_constructors.len());
        for (constructor_id, constructor) in self.inputs.struct_constructors.iter() {
            let owner = constructor.materialization;
            let parameters = constructor
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let location = LocalValueLocation::StructParameter {
                        constructor: raw_arena_index(constructor_id),
                        parameter: index as u32,
                    };
                    let identity = self.record_source(
                        owner,
                        LocalValueSelector::Parameter {
                            declaration_index: index as u32,
                        },
                        parameter.definition,
                        location,
                    )?;
                    self.bind(owner.context(), parameter.binding, identity);
                    Ok(identity)
                })
                .collect::<Result<Vec<_>, LocalValueIdentityError>>()?;
            let kind = match &constructor.kind {
                StructConstructorKind::Primary => StructConstructorLocalValueKind::Primary,
                StructConstructorKind::Secondary {
                    arguments, body, ..
                } => {
                    let receiver_location = LocalValueLocation::StructReceiver {
                        constructor: raw_arena_index(constructor_id),
                    };
                    let receiver = self.record_source(
                        owner,
                        LocalValueSelector::This,
                        constructor.origin,
                        receiver_location,
                    )?;
                    let argument_locals = self.collect_locals(
                        owner,
                        arguments.locals.iter().map(|(local, value)| {
                            (
                                local,
                                value,
                                LocalValueLocation::StructArgumentLocal {
                                    constructor: raw_arena_index(constructor_id),
                                    local: raw_arena_index(local),
                                },
                            )
                        }),
                    )?;
                    let body_locals = self.collect_locals(
                        owner,
                        body.locals.iter().map(|(local, value)| {
                            (
                                local,
                                value,
                                LocalValueLocation::StructBodyLocal {
                                    constructor: raw_arena_index(constructor_id),
                                    local: raw_arena_index(local),
                                },
                            )
                        }),
                    )?;
                    StructConstructorLocalValueKind::Secondary {
                        receiver,
                        argument_locals,
                        body_locals,
                    }
                }
            };
            output.push(StructConstructorLocalValues { parameters, kind });
        }
        Ok(output)
    }

    fn collect_callable_references(
        &mut self,
    ) -> Result<Vec<CallableReferenceLocalValue>, LocalValueIdentityError> {
        let mut output = Vec::with_capacity(self.inputs.callable_references.len());
        for (reference_id, reference) in self.inputs.callable_references.iter() {
            let value = match &reference.target {
                CallableReferenceTarget::Named(_) | CallableReferenceTarget::Local { .. } => {
                    CallableReferenceLocalValue::Unbound
                }
                CallableReferenceTarget::BoundMember { .. }
                | CallableReferenceTarget::BoundExtension { .. } => {
                    let location = LocalValueLocation::CallableReferenceReceiver {
                        reference: raw_arena_index(reference_id),
                    };
                    let identity = self.record_source(
                        *reference.identity.materialization(),
                        LocalValueSelector::BoundReceiver {
                            path: reference.identity.definition_path().clone(),
                        },
                        reference.origin,
                        location,
                    )?;
                    CallableReferenceLocalValue::Bound(identity)
                }
            };
            output.push(value);
        }
        Ok(output)
    }

    fn collect_locals<'b>(
        &mut self,
        owner: CallableMaterialization,
        locals: impl Iterator<Item = (LocalId, &'b super::Local, LocalValueLocation)>,
    ) -> Result<Vec<PersistentLocalValueId>, LocalValueIdentityError> {
        locals
            .map(|(_, local, location)| {
                let identity =
                    self.record(owner, local.selector.clone(), &local.definition, location)?;
                self.bind(owner.context(), local.binding, identity);
                Ok(identity)
            })
            .collect()
    }

    fn collect_captures(
        &self,
        context: CallableMaterializationContext,
        captures: &[Capture],
        owner: CaptureOwnerLocation,
    ) -> Result<Vec<PersistentLocalValueId>, LocalValueIdentityError> {
        captures
            .iter()
            .enumerate()
            .map(|(capture, declaration)| {
                self.find_captured_value(context, declaration.binding)
                    .ok_or(LocalValueIdentityError::MissingClosureCapture {
                        owner,
                        capture: capture as u32,
                        binding: declaration.binding.into_raw(),
                    })
            })
            .collect()
    }

    fn record_source(
        &mut self,
        owner: CallableMaterialization,
        selector: LocalValueSelector,
        origin: crate::DefinitionOrigin,
        location: LocalValueLocation,
    ) -> Result<PersistentLocalValueId, LocalValueIdentityError> {
        self.record(
            owner,
            selector,
            &crate::LocalValueDefinitionSite::Source(origin),
            location,
        )
    }

    fn record(
        &mut self,
        owner: CallableMaterialization,
        selector: LocalValueSelector,
        definition: &crate::LocalValueDefinitionSite,
        location: LocalValueLocation,
    ) -> Result<PersistentLocalValueId, LocalValueIdentityError> {
        let key = LocalValueKey::new(owner, selector);
        if let Some(first) = self.locations_by_key.insert(key.clone(), location) {
            return Err(LocalValueIdentityError::DuplicateSelector {
                first,
                second: location,
            });
        }
        let record =
            CborIdentityRecord::from_key(key).map_err(|error| LocalValueIdentityError::Hash {
                location,
                reason: error.to_string(),
            })?;
        let identity = record.id();
        if let Some((existing, first)) = self.records.insert(identity, (record.clone(), location))
            && existing.key() != record.key()
        {
            return Err(LocalValueIdentityError::HashCollision {
                first,
                second: location,
            });
        }
        if let crate::LocalValueDefinitionSite::Source(origin) = definition {
            let record = definition_origins::record(&self.inputs, identity, *origin, location)?;
            let previous = self.definition_origins.insert(identity, record);
            assert!(
                previous.is_none(),
                "one local-value identity is recorded at exactly one source definition site"
            );
        }
        Ok(identity)
    }

    fn bind(
        &mut self,
        context: CallableMaterializationContext,
        binding: BindingId,
        identity: PersistentLocalValueId,
    ) {
        self.values_by_binding
            .entry(BindingKey { context, binding })
            .or_default()
            .push(identity);
    }

    fn find_captured_value(
        &self,
        mut context: CallableMaterializationContext,
        binding: BindingId,
    ) -> Option<PersistentLocalValueId> {
        let mut visited = HashSet::new();
        loop {
            if let Some(identities) = self.values_by_binding.get(&BindingKey { context, binding }) {
                match identities.as_slice() {
                    [identity] => return Some(*identity),
                    [] => unreachable!("a binding index entry is non-empty"),
                    _ => return None,
                }
            }
            if !visited.insert(context) {
                return None;
            }
            let CallableMaterializationContext::Application(application) = context else {
                return None;
            };
            let record = self.inputs.callable_applications.get(application)?;
            context = match record.key().instantiation_owner() {
                CallableInstantiationOwner::EnclosingCallableApplication(parent) => {
                    CallableMaterializationContext::Application(parent)
                }
                CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
                    CallableMaterializationContext::InitializationApplication(unit)
                }
                CallableInstantiationOwner::NoOwner
                | CallableInstantiationOwner::ExactNominalOwner(_) => return None,
            };
        }
    }
}

fn arena_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_arena_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
