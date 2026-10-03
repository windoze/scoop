use scoop_identity::{
    CallableOwner, CallableTemplateOwner, CborIdentityRecord, DefinitionOriginSubject,
    DefinitionOwnerAtom, GeneratedCallableKey, InitializationUnitKey, NominalDeclarationOwner,
    PersistentId, PropertyOwner, SourceContextKey, SourceDeclarationKey,
};

use super::super::CanonicalHirFoundation;

pub(super) fn source_subject(
    foundation: &CanonicalHirFoundation,
    mut root: CallableTemplateOwner,
    context: &SourceContextKey,
) -> Option<DefinitionOriginSubject> {
    let mut matched = false;
    loop {
        matched |= matches!(context, SourceContextKey::Callable { owner, .. } if same_callable(root, *owner));
        match root {
            CallableTemplateOwner::Function(id) => {
                return matched.then_some(DefinitionOriginSubject::Function(id));
            }
            CallableTemplateOwner::GenericFunction(id) => {
                return matched.then_some(DefinitionOriginSubject::GenericFunction(id));
            }
            CallableTemplateOwner::Constructor(id) => {
                let key = key(&foundation.constructors, id)?;
                matched |= nominal_context(foundation, key, context);
                return matched.then_some(DefinitionOriginSubject::Constructor(id));
            }
            CallableTemplateOwner::Accessor(id) => {
                matched |= key(&foundation.property_accessors, id).is_some_and(|key| {
                    matches!(context, SourceContextKey::Property { owner, .. } if *owner == key.owner())
                });
                return matched.then_some(DefinitionOriginSubject::PropertyAccessor(id));
            }
            CallableTemplateOwner::Generated(id) => {
                let key = key(&foundation.generated_callables, id)?;
                match key {
                    GeneratedCallableKey::Lexical { parent, .. }
                    | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                        root = parent.template()
                    }
                    GeneratedCallableKey::Initialization { unit, .. } => {
                        matched |= matches!(context, SourceContextKey::Initialization { unit: owner, .. } if owner == unit)
                            || initialization_context(foundation, *unit, context);
                        return matched
                            .then_some(DefinitionOriginSubject::InitializationUnit(*unit));
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
                        root = source.template();
                    }
                    GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                        root = CallableTemplateOwner::Constructor(*constructor)
                    }
                    GeneratedCallableKey::DerivedEquality { .. }
                    | GeneratedCallableKey::FunctionAdapter { .. }
                    | GeneratedCallableKey::DynamicFunctionAdapter { .. }
                    | GeneratedCallableKey::ForeignCallbackManagedAdapter { .. }
                    | GeneratedCallableKey::CoroutineStart { .. }
                    | GeneratedCallableKey::FunctionBridge { .. }
                    | GeneratedCallableKey::BoxingAdjust { .. } => return None,
                }
            }
            CallableTemplateOwner::VariantConstructor(_) => return None,
            CallableTemplateOwner::ReleaseHook(exact) => {
                let owner = foundation.release_hook_owner(exact)?;
                if !matches!(context, SourceContextKey::Nominal { owner: source, .. } if *source == owner)
                {
                    return None;
                }
                return Some(match owner {
                    NominalDeclarationOwner::Concrete(id) => DefinitionOriginSubject::Type(id),
                    NominalDeclarationOwner::GenericTemplate(id) => {
                        DefinitionOriginSubject::GenericType(id)
                    }
                });
            }
        }
    }
}

fn same_callable(template: CallableTemplateOwner, owner: CallableOwner) -> bool {
    match (template, owner) {
        (CallableTemplateOwner::Function(a), CallableOwner::Function(b)) => a == b,
        (CallableTemplateOwner::GenericFunction(a), CallableOwner::GenericTemplate(b)) => a == b,
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
) -> bool {
    let owner = nominal_owner(declaration);
    match context {
        SourceContextKey::Nominal { owner: actual, .. } => owner == Some(*actual),
        SourceContextKey::Property {
            owner: property, ..
        } => {
            let declaration = match property {
                PropertyOwner::Property(id) => key(&foundation.properties, *id),
                PropertyOwner::ExtensionProperty(id) => key(&foundation.extension_properties, *id),
            };
            owner.is_some() && declaration.is_some_and(|key| nominal_owner(key) == owner)
        }
        _ => false,
    }
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
) -> bool {
    let Some(unit) = key(&foundation.initialization_units, unit) else {
        return false;
    };
    match (unit, context) {
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
        ) => key(&foundation.properties, *b)
            .is_some_and(|key| nominal_owner(key) == Some(NominalDeclarationOwner::Concrete(*a))),
        _ => false,
    }
}

fn key<I: PersistentId, K>(records: &[CborIdentityRecord<I, K>], id: I) -> Option<&K> {
    // These identity tables retain dependency order, not numeric ID order.

    records
        .iter()
        .find(|record| record.id() == id)
        .map(CborIdentityRecord::key)
}
