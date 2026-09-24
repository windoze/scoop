use scoop_identity::{
    CallableMaterializationContext, CallableOwner, CallableTemplateOwner, CborIdentityRecord,
    DefinitionOriginSubject, DefinitionOwnerAtom, GeneratedCallableKey, InitializationUnitKey,
    NominalDeclarationOwner, PersistentId, PropertyOwner, SourceContextKey, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::super::CanonicalHirFoundation;

pub(super) fn source_subject(
    foundation: &CanonicalHirFoundation,
    mut root: CallableTemplateOwner,
    context: &SourceContextKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Option<DefinitionOriginSubject>, WireError> {
    let mut depth = 1;
    let mut matched = false;
    loop {
        meter.check_semantic_depth(depth, path)?;
        meter.charge_work(1, path)?;
        matched |= matches!(context, SourceContextKey::Callable { owner, .. } if same_callable(root, *owner));
        match root {
            CallableTemplateOwner::Function(id) => {
                return Ok(matched.then_some(DefinitionOriginSubject::Function(id)));
            }
            CallableTemplateOwner::Constructor(id) => {
                let Some(key) = key(&foundation.constructors, id, meter, path)? else {
                    return Ok(None);
                };
                matched |= nominal_context(foundation, key, context, meter, path)?;
                return Ok(matched.then_some(DefinitionOriginSubject::Constructor(id)));
            }
            CallableTemplateOwner::Accessor(id) => {
                matched |= key(&foundation.property_accessors, id, meter, path)?.is_some_and(|key| {
                    matches!(context, SourceContextKey::Property { owner, .. } if *owner == key.owner())
                });
                return Ok(matched.then_some(DefinitionOriginSubject::PropertyAccessor(id)));
            }
            CallableTemplateOwner::Generated(id) => {
                let Some(key) = key(&foundation.generated_callables, id, meter, path)? else {
                    return Ok(None);
                };
                match key {
                    GeneratedCallableKey::Lexical { parent, .. }
                    | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                        root = parent.template()
                    }
                    GeneratedCallableKey::Initialization { unit, .. } => {
                        matched |= matches!(context, SourceContextKey::Initialization { unit: owner, .. } if owner == unit)
                            || initialization_context(foundation, *unit, context, meter, path)?;
                        return Ok(
                            matched.then_some(DefinitionOriginSubject::InitializationUnit(*unit))
                        );
                    }
                    GeneratedCallableKey::StaticNoGcCallbackStorageBridge { source, .. }
                    | GeneratedCallableKey::CoroutineDriver {
                        source_callable: source,
                    }
                    | GeneratedCallableKey::CoroutineAdapter {
                        source_callable: source,
                        ..
                    }
                    | GeneratedCallableKey::DispatchAdjust { target: source, .. } => {
                        if source.context() != CallableMaterializationContext::NoSubstitution {
                            return Ok(None);
                        }
                        root = source.template();
                    }
                    GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                        root = CallableTemplateOwner::Constructor(*constructor)
                    }
                    GeneratedCallableKey::DerivedEquality { .. }
                    | GeneratedCallableKey::FunctionAdapter { .. }
                    | GeneratedCallableKey::DynamicFunctionAdapter { .. }
                    | GeneratedCallableKey::ForeignCallbackManagedAdapter { .. }
                    | GeneratedCallableKey::ContinuationShell { .. }
                    | GeneratedCallableKey::CoroutineStart { .. }
                    | GeneratedCallableKey::FunctionBridge { .. }
                    | GeneratedCallableKey::BoxingAdjust { .. } => return Ok(None),
                }
            }
            CallableTemplateOwner::GenericFunction(_)
            | CallableTemplateOwner::VariantConstructor(_) => return Ok(None),
        }
        depth += 1;
        meter.charge_edges(1, path)?;
    }
}

fn same_callable(template: CallableTemplateOwner, owner: CallableOwner) -> bool {
    match (template, owner) {
        (CallableTemplateOwner::Function(a), CallableOwner::Function(b)) => a == b,
        (CallableTemplateOwner::Constructor(a), CallableOwner::Constructor(b)) => a == b,
        (CallableTemplateOwner::Accessor(a), CallableOwner::Accessor(b)) => a == b,
        (CallableTemplateOwner::Generated(a), CallableOwner::Generated(b)) => a == b,
        _ => false,
    }
}

fn nominal_context(
    foundation: &CanonicalHirFoundation,
    declaration: &SourceDeclarationKey,
    context: &SourceContextKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    let owner = nominal_owner(declaration);
    Ok(match context {
        SourceContextKey::Nominal { owner: actual, .. } => owner == Some(*actual),
        SourceContextKey::Property {
            owner: property, ..
        } => {
            let declaration = match property {
                PropertyOwner::Property(id) => key(&foundation.properties, *id, meter, path)?,
                PropertyOwner::ExtensionProperty(id) => {
                    key(&foundation.extension_properties, *id, meter, path)?
                }
            };
            owner.is_some() && declaration.is_some_and(|key| nominal_owner(key) == owner)
        }
        _ => false,
    })
}

fn nominal_owner(key: &SourceDeclarationKey) -> Option<NominalDeclarationOwner> {
    match key.owners().owners().last()? {
        DefinitionOwnerAtom::Type(id) => Some(NominalDeclarationOwner::Concrete(*id)),
        DefinitionOwnerAtom::GenericType(id) => Some(NominalDeclarationOwner::GenericTemplate(*id)),
        _ => None,
    }
}

fn initialization_context(
    foundation: &CanonicalHirFoundation,
    unit: scoop_identity::PersistentInitializationUnitId,
    context: &SourceContextKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    let Some(unit) = key(&foundation.initialization_units, unit, meter, path)? else {
        return Ok(false);
    };
    Ok(match (unit, context) {
        (
            InitializationUnitKey::TopLevelProperty(a),
            SourceContextKey::Property {
                owner: PropertyOwner::Property(b),
                ..
            },
        ) => a == b,
        (
            InitializationUnitKey::ExtensionProperty(a),
            SourceContextKey::Property {
                owner: PropertyOwner::ExtensionProperty(b),
                ..
            },
        ) => a == b,
        (
            InitializationUnitKey::Object(a) | InitializationUnitKey::Companion(a),
            SourceContextKey::Nominal {
                owner: NominalDeclarationOwner::Concrete(b),
                ..
            },
        ) => a == b,
        (
            InitializationUnitKey::Object(a) | InitializationUnitKey::Companion(a),
            SourceContextKey::Property {
                owner: PropertyOwner::Property(b),
                ..
            },
        ) => key(&foundation.properties, *b, meter, path)?
            .is_some_and(|key| nominal_owner(key) == Some(NominalDeclarationOwner::Concrete(*a))),
        _ => false,
    })
}

fn key<'a, I: PersistentId, K>(
    records: &'a [CborIdentityRecord<I, K>],
    id: I,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Option<&'a K>, WireError> {
    // These identity tables retain dependency order, not numeric ID order.
    meter.charge_work((records.len() as u64).saturating_mul(64) + 1, path)?;
    Ok(records
        .iter()
        .find(|record| record.id() == id)
        .map(CborIdentityRecord::key))
}
