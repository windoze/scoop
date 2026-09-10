use std::collections::{HashMap, HashSet};

use la_arena::Idx;
use scoop_identity::{
    GeneratedCallableKey, InitializationCallableRole, LexicalCallableParent, LexicalCallableRole,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    StructuralDefinitionPath,
};

use super::{
    FunctionIdentityRelation, HirFunctionIdentity, HirFunctionIdentityError,
    HirFunctionIdentityInputs, HirPropertyAccessorFunction, HirSourceFunctionIdentity,
};
use crate::{FunctionId, FunctionKind, LexicalDefinitionRoot, PropertyAccessorImplementation};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClaimKind {
    PropertyAccessor(HirPropertyAccessorFunction),
    Lexical {
        root: LexicalDefinitionRoot,
        role: LexicalCallableRole,
        path: StructuralDefinitionPath,
    },
    Initialization {
        unit: crate::InitializationUnitId,
        role: InitializationCallableRole,
    },
    DerivedEquality,
}

#[derive(Clone, Debug)]
struct Claim {
    relation: FunctionIdentityRelation,
    kind: ClaimKind,
}

pub(super) fn validate(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
) -> Result<(), HirFunctionIdentityError> {
    if inputs.functions.len() != identities.len() {
        return Err(HirFunctionIdentityError::Length {
            expected: inputs.functions.len(),
            actual: identities.len(),
        });
    }

    let mut claims = vec![None; inputs.functions.len()];
    claim_accessors(inputs, &mut claims)?;
    claim_lexical_functions(inputs, &mut claims)?;
    claim_initialization(inputs, &mut claims)?;
    let nominal_derived = claim_derived_equality(inputs, &mut claims)?;

    let mut plain_ids = HashSet::<PersistentFunctionId>::new();
    let mut generic_ids = HashSet::<PersistentGenericFunctionId>::new();
    let mut generated_ids = HashSet::<PersistentGeneratedCallableId>::new();

    for (function, declaration) in inputs.functions.iter() {
        let identity = &identities[local_index(function)];
        let claim = claims[local_index(function)].as_ref();
        validate_entry(
            inputs,
            identities,
            function,
            declaration,
            identity,
            claim,
            nominal_derived.contains(&function),
        )?;
        collect_unique_ids(
            function,
            identity,
            &mut plain_ids,
            &mut generic_ids,
            &mut generated_ids,
        )?;
    }
    Ok(())
}

fn claim_accessors(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (getter, declaration) in inputs.property_getters.iter() {
        if let PropertyAccessorImplementation::Body(function)
        | PropertyAccessorImplementation::AbstractSlot(function) = declaration.implementation
        {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::PropertyGetter,
                raw_index(getter),
                ClaimKind::PropertyAccessor(HirPropertyAccessorFunction::Getter(getter)),
            )?;
        }
    }
    for (setter, declaration) in inputs.property_setters.iter() {
        if let PropertyAccessorImplementation::Body(function)
        | PropertyAccessorImplementation::AbstractSlot(function) = declaration.implementation
        {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::PropertySetter,
                raw_index(setter),
                ClaimKind::PropertyAccessor(HirPropertyAccessorFunction::Setter(setter)),
            )?;
        }
    }
    Ok(())
}

fn claim_lexical_functions(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (lambda, declaration) in inputs.lambdas.iter() {
        claim(
            inputs,
            claims,
            declaration.function,
            FunctionIdentityRelation::Lambda,
            raw_index(lambda),
            ClaimKind::Lexical {
                root: declaration.definition_root,
                role: LexicalCallableRole::LambdaBody,
                path: declaration.definition_path.clone(),
            },
        )?;
    }
    for (anonymous, declaration) in inputs.anonymous_functions.iter() {
        claim(
            inputs,
            claims,
            declaration.function,
            FunctionIdentityRelation::AnonymousFunction,
            raw_index(anonymous),
            ClaimKind::Lexical {
                root: declaration.definition_root,
                role: LexicalCallableRole::AnonymousFunctionBody,
                path: declaration.definition_path.clone(),
            },
        )?;
    }
    Ok(())
}

fn claim_initialization(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (unit, declaration) in inputs.initialization_units.iter() {
        for (function, relation, role) in [
            (
                declaration.initializer,
                FunctionIdentityRelation::InitializationInitializer,
                InitializationCallableRole::Initializer,
            ),
            (
                declaration.ensure,
                FunctionIdentityRelation::InitializationEnsure,
                InitializationCallableRole::Ensure,
            ),
        ] {
            claim(
                inputs,
                claims,
                function,
                relation,
                raw_index(unit),
                ClaimKind::Initialization { unit, role },
            )?;
        }
    }
    Ok(())
}

fn claim_derived_equality(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<HashSet<FunctionId>, HirFunctionIdentityError> {
    let mut nominal = HashMap::new();
    for (owner, declaration) in inputs.structs.iter() {
        if let Some(function) = declaration.derived_equality {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::StructDerivedEquality,
                raw_index(owner),
                ClaimKind::DerivedEquality,
            )?;
            if let Some(first) =
                nominal.insert(function, FunctionIdentityRelation::StructDerivedEquality)
            {
                return Err(HirFunctionIdentityError::ConflictingClaim {
                    function: raw_index(function),
                    first,
                    second: FunctionIdentityRelation::StructDerivedEquality,
                });
            }
        }
    }
    for (owner, declaration) in inputs.enums.iter() {
        if let Some(function) = declaration.derived_equality {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::EnumDerivedEquality,
                raw_index(owner),
                ClaimKind::DerivedEquality,
            )?;
            if let Some(first) =
                nominal.insert(function, FunctionIdentityRelation::EnumDerivedEquality)
            {
                return Err(HirFunctionIdentityError::ConflictingClaim {
                    function: raw_index(function),
                    first,
                    second: FunctionIdentityRelation::EnumDerivedEquality,
                });
            }
        }
    }
    for (application, declaration) in inputs.derived_equality_applications.iter() {
        claim(
            inputs,
            claims,
            declaration.function,
            FunctionIdentityRelation::DerivedEqualityApplication,
            raw_index(application),
            ClaimKind::DerivedEquality,
        )?;
    }
    Ok(nominal.into_keys().collect())
}

fn claim(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
    function: FunctionId,
    relation: FunctionIdentityRelation,
    owner: u32,
    kind: ClaimKind,
) -> Result<(), HirFunctionIdentityError> {
    let index = local_index(function);
    if index >= inputs.functions.len() {
        return Err(HirFunctionIdentityError::UnknownFunction {
            relation,
            owner,
            function: raw_index(function),
        });
    }
    if let Some(existing) = &claims[index] {
        if existing.kind == kind
            && matches!(kind, ClaimKind::Lexical { .. } | ClaimKind::DerivedEquality)
        {
            return Ok(());
        }
        return Err(HirFunctionIdentityError::ConflictingClaim {
            function: raw_index(function),
            first: existing.relation,
            second: relation,
        });
    }
    claims[index] = Some(Claim { relation, kind });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_entry(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    declaration: &crate::Function,
    identity: &HirFunctionIdentity,
    claim: Option<&Claim>,
    nominal_derived: bool,
) -> Result<(), HirFunctionIdentityError> {
    let is_derived = matches!(declaration.kind, FunctionKind::DerivedEquality);
    match (identity, claim) {
        (HirFunctionIdentity::Source(_), None) if !is_derived => Ok(()),
        (
            HirFunctionIdentity::PropertyAccessor(actual),
            Some(Claim {
                kind: ClaimKind::PropertyAccessor(expected),
                ..
            }),
        ) if actual == expected => validate_accessor(inputs, function, *actual),
        (
            HirFunctionIdentity::LexicalGenerated(record),
            Some(Claim {
                kind: ClaimKind::Lexical { root, role, path },
                ..
            }),
        ) => {
            let parent = resolve_lexical_parent(inputs, identities, function, *root)?;
            if record.key()
                == &(GeneratedCallableKey::Lexical {
                    parent,
                    role: *role,
                    path: path.clone(),
                })
            {
                Ok(())
            } else {
                Err(HirFunctionIdentityError::LexicalIdentity {
                    function: raw_index(function),
                })
            }
        }
        (
            HirFunctionIdentity::Initialization { unit, role, record },
            Some(Claim {
                kind:
                    ClaimKind::Initialization {
                        unit: expected_unit,
                        role: expected_role,
                    },
                ..
            }),
        ) => {
            let expected_key = GeneratedCallableKey::Initialization {
                unit: inputs.initialization_unit_identities[*expected_unit].id(),
                role: *expected_role,
            };
            if unit == expected_unit && role == expected_role && record.key() == &expected_key {
                Ok(())
            } else {
                Err(HirFunctionIdentityError::InitializationIdentity {
                    function: raw_index(function),
                })
            }
        }
        (
            HirFunctionIdentity::DerivedEquality(applications),
            Some(Claim {
                kind: ClaimKind::DerivedEquality,
                ..
            }),
        ) if is_derived => {
            validate_derived_equality(inputs, function, applications, nominal_derived)
        }
        _ => Err(HirFunctionIdentityError::IdentityKind {
            function: raw_index(function),
        }),
    }
}

fn validate_accessor(
    inputs: &HirFunctionIdentityInputs<'_>,
    function: FunctionId,
    accessor: HirPropertyAccessorFunction,
) -> Result<(), HirFunctionIdentityError> {
    let valid = match accessor {
        HirPropertyAccessorFunction::Getter(getter) => {
            local_index(getter) < inputs.property_getters.len()
                && matches!(
                    inputs.property_getters[getter].implementation,
                    PropertyAccessorImplementation::Body(actual)
                        | PropertyAccessorImplementation::AbstractSlot(actual)
                        if actual == function
                )
                && inputs
                    .property_accessor_identities
                    .get_getter(getter)
                    .is_some()
        }
        HirPropertyAccessorFunction::Setter(setter) => {
            local_index(setter) < inputs.property_setters.len()
                && matches!(
                    inputs.property_setters[setter].implementation,
                    PropertyAccessorImplementation::Body(actual)
                        | PropertyAccessorImplementation::AbstractSlot(actual)
                        if actual == function
                )
                && inputs
                    .property_accessor_identities
                    .get_setter(setter)
                    .is_some()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(HirFunctionIdentityError::AccessorIdentity {
            function: raw_index(function),
        })
    }
}

fn validate_derived_equality(
    inputs: &HirFunctionIdentityInputs<'_>,
    function: FunctionId,
    identities: &[super::HirDerivedEqualityFunctionIdentity],
    nominal_derived: bool,
) -> Result<(), HirFunctionIdentityError> {
    let expected = inputs
        .derived_equality_applications
        .iter()
        .filter(|(_, application)| application.function == function)
        .collect::<Vec<_>>();
    if expected.is_empty() && !nominal_derived {
        return Err(HirFunctionIdentityError::UnownedDerivedEqualityTemplate {
            function: raw_index(function),
        });
    }
    if identities.len() != expected.len() {
        return Err(HirFunctionIdentityError::DerivedEqualityIdentity {
            function: raw_index(function),
        });
    }
    for (identity, (application, declaration)) in identities.iter().zip(expected) {
        let exact_owner = inputs
            .type_identities
            .get(declaration.owner_ty)
            .and_then(crate::HirTypeIdentity::exact)
            .ok_or(HirFunctionIdentityError::OpenDerivedEqualityOwner {
                application: raw_index(application),
            })?
            .id();
        if identity.application() != application
            || identity.record().key() != &(GeneratedCallableKey::DerivedEquality { exact_owner })
        {
            return Err(HirFunctionIdentityError::DerivedEqualityIdentity {
                function: raw_index(function),
            });
        }
    }
    Ok(())
}

fn resolve_lexical_parent(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    root: LexicalDefinitionRoot,
) -> Result<LexicalCallableParent, HirFunctionIdentityError> {
    let invalid = || HirFunctionIdentityError::LexicalIdentity {
        function: raw_index(function),
    };
    match root {
        LexicalDefinitionRoot::Function(root) => {
            let root_identity = identities.get(local_index(root)).ok_or_else(invalid)?;
            match root_identity {
                HirFunctionIdentity::Source(identity) => Ok(identity.lexical_parent()),
                HirFunctionIdentity::PropertyAccessor(accessor) => {
                    let id = match accessor {
                        HirPropertyAccessorFunction::Getter(getter) => inputs
                            .property_accessor_identities
                            .get_getter(*getter)
                            .ok_or_else(invalid)?
                            .id(),
                        HirPropertyAccessorFunction::Setter(setter) => inputs
                            .property_accessor_identities
                            .get_setter(*setter)
                            .ok_or_else(invalid)?
                            .id(),
                    };
                    Ok(LexicalCallableParent::accessor(id))
                }
                HirFunctionIdentity::LexicalGenerated(record)
                | HirFunctionIdentity::Initialization { record, .. } => {
                    LexicalCallableParent::from_generated_key(record.key())
                        .map_err(HirFunctionIdentityError::LexicalParent)
                }
                HirFunctionIdentity::DerivedEquality(_) => Err(invalid()),
            }
        }
        LexicalDefinitionRoot::ClassConstructor(root) => {
            if local_index(root) >= inputs.class_constructors.len() {
                return Err(invalid());
            }
            let identity = &inputs.constructor_identities[root];
            if let Some(record) = identity.source_record() {
                Ok(LexicalCallableParent::constructor(record.id()))
            } else if let Some(record) = identity.generated_record() {
                LexicalCallableParent::from_generated_key(record.key())
                    .map_err(HirFunctionIdentityError::LexicalParent)
            } else {
                Err(invalid())
            }
        }
        LexicalDefinitionRoot::StructConstructor(root) => {
            if local_index(root) >= inputs.struct_constructors.len() {
                return Err(invalid());
            }
            Ok(LexicalCallableParent::constructor(
                inputs.constructor_identities[root].id(),
            ))
        }
        LexicalDefinitionRoot::VariantConstructor(root) => {
            let enumeration = root.enumeration();
            if local_index(enumeration) >= inputs.enums.len() {
                return Err(invalid());
            }
            let declaration = &inputs.enums[enumeration];
            if root.local_index() as usize >= declaration.variants.len() {
                return Err(invalid());
            }
            Ok(LexicalCallableParent::variant_constructor(
                inputs.enum_member_identities[root].id(),
            ))
        }
    }
}

fn collect_unique_ids(
    function: FunctionId,
    identity: &HirFunctionIdentity,
    plain: &mut HashSet<PersistentFunctionId>,
    generic: &mut HashSet<PersistentGenericFunctionId>,
    generated: &mut HashSet<PersistentGeneratedCallableId>,
) -> Result<(), HirFunctionIdentityError> {
    match identity {
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
            if !plain.insert(record.id()) {
                return Err(HirFunctionIdentityError::DuplicatePlainIdentity {
                    function: raw_index(function),
                });
            }
        }
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(record)) => {
            if !generic.insert(record.id()) {
                return Err(HirFunctionIdentityError::DuplicateGenericIdentity {
                    function: raw_index(function),
                });
            }
        }
        HirFunctionIdentity::LexicalGenerated(record)
        | HirFunctionIdentity::Initialization { record, .. } => {
            insert_generated(function, record.id(), generated)?;
        }
        HirFunctionIdentity::DerivedEquality(applications) => {
            for application in applications {
                insert_generated(function, application.record().id(), generated)?;
            }
        }
        HirFunctionIdentity::PropertyAccessor(_) => {}
    }
    Ok(())
}

fn insert_generated(
    function: FunctionId,
    id: PersistentGeneratedCallableId,
    generated: &mut HashSet<PersistentGeneratedCallableId>,
) -> Result<(), HirFunctionIdentityError> {
    if generated.insert(id) {
        Ok(())
    } else {
        Err(HirFunctionIdentityError::DuplicateGeneratedIdentity {
            function: raw_index(function),
        })
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
