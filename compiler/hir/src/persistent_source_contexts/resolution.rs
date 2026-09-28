use la_arena::{Arena, Idx};
use scoop_identity::{
    CallableOwner, NominalDeclarationOwner, PersistentGeneratedCallableId,
    PropertyOwner as PersistentPropertyOwner, SourceContextKey, SourceDeclarationKey,
    SourceIdentity,
};

use super::{
    HirSourceContextIdentityError, HirSourceContextIdentityInputs, HirSourceContextReferenceKind,
};
use crate::{
    FunctionId, HirClassConstructorIdentity, HirFunctionIdentity, HirPropertyAccessorFunction,
    HirSourceFunctionIdentity, HirSourceNominalIdentity, InitializationUnitId,
    InitializationUnitKind, LexicalDefinitionRoot, PropertyId, SourceContext,
    SourceContextConstructor, SourceContextId, SourceContextNominal, SourceContextSubject,
};

pub(super) fn context_key(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    value: &SourceContext,
) -> Result<SourceContextKey, HirSourceContextIdentityError> {
    let source = value.source();
    match value.subject() {
        SourceContextSubject::File => Ok(SourceContextKey::File {
            source: source.clone(),
        }),
        SourceContextSubject::Imported(key) => {
            if key.source() != source {
                return Err(HirSourceContextIdentityError::SourceMismatch {
                    context: raw_index(context),
                });
            }
            Ok(key.clone())
        }
        SourceContextSubject::Nominal(owner) => Ok(SourceContextKey::Nominal {
            source: source.clone(),
            owner: nominal_owner(inputs, context, *owner, source)?,
        }),
        SourceContextSubject::Function(function) => {
            function_context_key(inputs, context, *function, source)
        }
        SourceContextSubject::Constructor(owner) => Ok(SourceContextKey::Callable {
            source: source.clone(),
            owner: constructor_owner(inputs, context, *owner, source)?,
        }),
        SourceContextSubject::Property(property) => Ok(SourceContextKey::Property {
            source: source.clone(),
            owner: property_owner(inputs, context, *property, source)?,
        }),
        SourceContextSubject::LexicalCallable { root, path, role } => {
            validate_root_source(inputs, context, *root, source)?;
            let generated = lexical_callable_identity(inputs, context, *root, path, *role)?;
            Ok(SourceContextKey::Callable {
                source: source.clone(),
                owner: CallableOwner::Generated(generated),
            })
        }
    }
}

fn nominal_owner(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    owner: SourceContextNominal,
    source: &SourceIdentity,
) -> Result<NominalDeclarationOwner, HirSourceContextIdentityError> {
    let identity = match owner {
        SourceContextNominal::Struct(id) => {
            require_reference(
                context,
                id,
                inputs.structs,
                HirSourceContextReferenceKind::Struct,
            )?;
            &inputs.nominal_identities[id]
        }
        SourceContextNominal::Enum(id) => {
            require_reference(
                context,
                id,
                inputs.enums,
                HirSourceContextReferenceKind::Enum,
            )?;
            &inputs.nominal_identities[id]
        }
        SourceContextNominal::Class(id) => {
            require_reference(
                context,
                id,
                inputs.classes,
                HirSourceContextReferenceKind::Class,
            )?;
            &inputs.nominal_identities[id]
        }
        SourceContextNominal::Interface(id) => {
            require_reference(
                context,
                id,
                inputs.interfaces,
                HirSourceContextReferenceKind::Interface,
            )?;
            &inputs.nominal_identities[id]
        }
        SourceContextNominal::Object(id) => {
            require_reference(
                context,
                id,
                inputs.objects,
                HirSourceContextReferenceKind::Object,
            )?;
            &inputs.nominal_identities[id]
        }
    };
    let identity = identity
        .source()
        .ok_or(HirSourceContextIdentityError::NonSourceNominal {
            context: raw_index(context),
        })?;
    require_declaration_source(context, source, identity.declaration())?;
    Ok(match identity {
        HirSourceNominalIdentity::Concrete(record) => {
            NominalDeclarationOwner::Concrete(record.id())
        }
        HirSourceNominalIdentity::Generic(record) => {
            NominalDeclarationOwner::GenericTemplate(record.id())
        }
    })
}

fn function_context_key(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    function: FunctionId,
    source: &SourceIdentity,
) -> Result<SourceContextKey, HirSourceContextIdentityError> {
    require_reference(
        context,
        function,
        inputs.functions,
        HirSourceContextReferenceKind::Function,
    )?;
    match &inputs.function_identities[function] {
        HirFunctionIdentity::Source(identity) => {
            require_declaration_source(context, source, identity.declaration())?;
            let owner = match identity {
                HirSourceFunctionIdentity::Plain(record) => CallableOwner::Function(record.id()),
                HirSourceFunctionIdentity::Generic(record) => {
                    CallableOwner::GenericTemplate(record.id())
                }
            };
            Ok(SourceContextKey::Callable {
                source: source.clone(),
                owner,
            })
        }
        HirFunctionIdentity::PropertyAccessor(accessor) => {
            let owner = accessor_owner(inputs, context, *accessor, source)?;
            Ok(SourceContextKey::Callable {
                source: source.clone(),
                owner: CallableOwner::Accessor(owner),
            })
        }
        HirFunctionIdentity::LexicalGenerated(record) => {
            validate_generated_function_source(inputs, context, function, source)?;
            Ok(SourceContextKey::Callable {
                source: source.clone(),
                owner: CallableOwner::Generated(record.id()),
            })
        }
        HirFunctionIdentity::Initialization { unit, .. } => {
            validate_initialization_source(inputs, context, *unit, source)?;
            Ok(SourceContextKey::Initialization {
                source: source.clone(),
                unit: inputs.initialization_unit_identities[*unit].id(),
            })
        }
        HirFunctionIdentity::DerivedEquality(_) => {
            Err(HirSourceContextIdentityError::InvalidFunctionSubject {
                context: raw_index(context),
            })
        }
    }
}

fn accessor_owner(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    accessor: HirPropertyAccessorFunction,
    source: &SourceIdentity,
) -> Result<scoop_identity::PersistentPropertyAccessorId, HirSourceContextIdentityError> {
    let identity = match accessor {
        HirPropertyAccessorFunction::Getter(getter) => inputs
            .property_accessor_identities
            .get_getter(getter)
            .ok_or(HirSourceContextIdentityError::InvalidFunctionSubject {
                context: raw_index(context),
            })?,
        HirPropertyAccessorFunction::Setter(setter) => inputs
            .property_accessor_identities
            .get_setter(setter)
            .ok_or(HirSourceContextIdentityError::InvalidFunctionSubject {
                context: raw_index(context),
            })?,
    };
    property_owner(inputs, context, identity.property(), source)?;
    Ok(identity.id())
}

fn constructor_owner(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    owner: SourceContextConstructor,
    source: &SourceIdentity,
) -> Result<CallableOwner, HirSourceContextIdentityError> {
    match owner {
        SourceContextConstructor::Struct(constructor) => {
            require_reference(
                context,
                constructor,
                inputs.struct_constructors,
                HirSourceContextReferenceKind::StructConstructor,
            )?;
            let record = &inputs.constructor_identities[constructor];
            require_declaration_source(context, source, record.key())?;
            Ok(CallableOwner::Constructor(record.id()))
        }
        SourceContextConstructor::Class(constructor) => {
            require_reference(
                context,
                constructor,
                inputs.class_constructors,
                HirSourceContextReferenceKind::ClassConstructor,
            )?;
            match &inputs.constructor_identities[constructor] {
                HirClassConstructorIdentity::Source(record) => {
                    require_declaration_source(context, source, record.key())?;
                    Ok(CallableOwner::Constructor(record.id()))
                }
                HirClassConstructorIdentity::ZeroArgumentAdapter {
                    source: source_constructor,
                    record,
                } => {
                    require_reference(
                        context,
                        *source_constructor,
                        inputs.class_constructors,
                        HirSourceContextReferenceKind::ClassConstructor,
                    )?;
                    let source_record = inputs.constructor_identities[*source_constructor]
                        .source_record()
                        .ok_or(HirSourceContextIdentityError::InvalidConstructorSubject {
                            context: raw_index(context),
                        })?;
                    require_declaration_source(context, source, source_record.key())?;
                    Ok(CallableOwner::Generated(record.id()))
                }
            }
        }
    }
}

fn property_owner(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    property: PropertyId,
    source: &SourceIdentity,
) -> Result<PersistentPropertyOwner, HirSourceContextIdentityError> {
    require_reference(
        context,
        property,
        inputs.properties,
        HirSourceContextReferenceKind::Property,
    )?;
    let identity = &inputs.property_identities[property];
    require_declaration_source(context, source, identity.declaration())?;
    Ok(identity.property_owner())
}

fn validate_initialization_source(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    unit: InitializationUnitId,
    source: &SourceIdentity,
) -> Result<(), HirSourceContextIdentityError> {
    require_reference(
        context,
        unit,
        inputs.initialization_units,
        HirSourceContextReferenceKind::InitializationUnit,
    )?;
    match inputs.initialization_units[unit].kind {
        InitializationUnitKind::EagerTopLevel { property, .. }
        | InitializationUnitKind::GenericDelegatedExtension { property, .. } => {
            property_owner(inputs, context, property, source)?;
        }
        InitializationUnitKind::LazySingleton { value, .. } => {
            require_reference(
                context,
                value,
                inputs.singleton_values,
                HirSourceContextReferenceKind::SingletonValue,
            )?;
            let object = inputs.singleton_values[value].declaration;
            nominal_owner(
                inputs,
                context,
                SourceContextNominal::Object(object),
                source,
            )?;
        }
    }
    Ok(())
}

fn validate_generated_function_source(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    function: FunctionId,
    source: &SourceIdentity,
) -> Result<(), HirSourceContextIdentityError> {
    let root = inputs
        .lambdas
        .iter()
        .find_map(|(_, literal)| (literal.function == function).then_some(literal.definition_root))
        .or_else(|| {
            inputs.anonymous_functions.iter().find_map(|(_, literal)| {
                (literal.function == function).then_some(literal.definition_root)
            })
        })
        .ok_or(HirSourceContextIdentityError::InvalidFunctionSubject {
            context: raw_index(context),
        })?;
    validate_root_source(inputs, context, root, source)
}

fn lexical_callable_identity(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    root: LexicalDefinitionRoot,
    path: &scoop_identity::StructuralDefinitionPath,
    role: scoop_identity::LexicalCallableRole,
) -> Result<PersistentGeneratedCallableId, HirSourceContextIdentityError> {
    let mut identity = None;
    let functions = match role {
        scoop_identity::LexicalCallableRole::LambdaBody => inputs
            .lambdas
            .iter()
            .filter_map(|(_, literal)| {
                (literal.definition_root == root && literal.definition_path == *path)
                    .then_some(literal.function)
            })
            .collect::<Vec<_>>(),
        scoop_identity::LexicalCallableRole::AnonymousFunctionBody => inputs
            .anonymous_functions
            .iter()
            .filter_map(|(_, literal)| {
                (literal.definition_root == root && literal.definition_path == *path)
                    .then_some(literal.function)
            })
            .collect::<Vec<_>>(),
    };
    for function in functions {
        require_reference(
            context,
            function,
            inputs.functions,
            HirSourceContextReferenceKind::Function,
        )?;
        let HirFunctionIdentity::LexicalGenerated(record) = &inputs.function_identities[function]
        else {
            return Err(HirSourceContextIdentityError::InvalidLexicalSubject {
                context: raw_index(context),
            });
        };
        match identity {
            Some(existing) if existing != record.id() => {
                return Err(HirSourceContextIdentityError::ConflictingLexicalIdentity {
                    context: raw_index(context),
                });
            }
            Some(_) => {}
            None => identity = Some(record.id()),
        }
    }
    identity.ok_or(HirSourceContextIdentityError::InvalidLexicalSubject {
        context: raw_index(context),
    })
}

fn validate_root_source(
    inputs: &HirSourceContextIdentityInputs<'_>,
    context: SourceContextId,
    root: LexicalDefinitionRoot,
    source: &SourceIdentity,
) -> Result<(), HirSourceContextIdentityError> {
    match root {
        LexicalDefinitionRoot::Function(function) => {
            function_context_key(inputs, context, function, source)?;
        }
        LexicalDefinitionRoot::ClassConstructor(constructor) => {
            constructor_owner(
                inputs,
                context,
                SourceContextConstructor::Class(constructor),
                source,
            )?;
        }
        LexicalDefinitionRoot::StructConstructor(constructor) => {
            constructor_owner(
                inputs,
                context,
                SourceContextConstructor::Struct(constructor),
                source,
            )?;
        }
        LexicalDefinitionRoot::VariantConstructor(variant) => {
            nominal_owner(
                inputs,
                context,
                SourceContextNominal::Enum(variant.enumeration()),
                source,
            )?;
        }
    }
    Ok(())
}

fn require_declaration_source(
    context: SourceContextId,
    source: &SourceIdentity,
    declaration: &SourceDeclarationKey,
) -> Result<(), HirSourceContextIdentityError> {
    if declaration.origin() != source.cone()
        || declaration
            .scope()
            .source()
            .is_some_and(|declared| declared != source)
    {
        Err(HirSourceContextIdentityError::SourceMismatch {
            context: raw_index(context),
        })
    } else {
        Ok(())
    }
}

fn require_reference<T>(
    context: SourceContextId,
    target: Idx<T>,
    arena: &Arena<T>,
    kind: HirSourceContextReferenceKind,
) -> Result<(), HirSourceContextIdentityError> {
    if local_index(target) < arena.len() {
        Ok(())
    } else {
        Err(HirSourceContextIdentityError::UnknownReference {
            context: raw_index(context),
            kind,
            target: raw_index(target),
        })
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
