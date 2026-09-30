use super::*;

pub(super) fn resolve_lexical_parent(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    root: LexicalDefinitionRoot,
    path: &StructuralDefinitionPath,
) -> Result<LexicalCallableParent, HirFunctionIdentityError> {
    if let Some(parent) = immediate_parent_function(inputs, function, root, path)? {
        return function_lexical_parent(inputs, identities, function, parent);
    }
    root_lexical_parent(inputs, identities, function, root)
}

pub(super) fn resolve_definition_owner(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    root: LexicalDefinitionRoot,
    path: &StructuralDefinitionPath,
) -> Result<DefinitionOwnerAtom, HirFunctionIdentityError> {
    if let Some(parent) = immediate_parent_function(inputs, function, root, path)? {
        return function_definition_owner(inputs, identities, function, parent);
    }
    root_definition_owner(inputs, identities, function, root)
}

fn immediate_parent_function(
    inputs: &HirFunctionIdentityInputs<'_>,
    function: FunctionId,
    root: LexicalDefinitionRoot,
    path: &StructuralDefinitionPath,
) -> Result<Option<FunctionId>, HirFunctionIdentityError> {
    let mut candidate = None;
    for (_, declaration) in inputs.local_functions.iter() {
        let Some((source_function, source_root)) = declaration.source() else {
            continue;
        };
        consider_parent(
            function,
            root,
            path,
            source_function,
            source_root,
            &declaration.definition_path,
            &mut candidate,
        )?;
    }
    for (_, declaration) in inputs.lambdas.iter() {
        consider_parent(
            function,
            root,
            path,
            declaration.function,
            declaration.definition_root,
            &declaration.definition_path,
            &mut candidate,
        )?;
    }
    for (_, declaration) in inputs.anonymous_functions.iter() {
        consider_parent(
            function,
            root,
            path,
            declaration.function,
            declaration.definition_root,
            &declaration.definition_path,
            &mut candidate,
        )?;
    }
    Ok(candidate.map(|(_, parent)| parent))
}

#[allow(clippy::too_many_arguments)]
fn consider_parent(
    function: FunctionId,
    root: LexicalDefinitionRoot,
    path: &StructuralDefinitionPath,
    possible_parent: FunctionId,
    possible_root: LexicalDefinitionRoot,
    possible_path: &StructuralDefinitionPath,
    candidate: &mut Option<(usize, FunctionId)>,
) -> Result<(), HirFunctionIdentityError> {
    let possible_segments = possible_path.segments();
    let segments = path.segments();
    if possible_root != root
        || possible_segments.len() >= segments.len()
        || !segments.starts_with(possible_segments)
    {
        return Ok(());
    }
    if possible_parent == function {
        return Err(HirFunctionIdentityError::LexicalIdentity {
            function: raw_index(function),
        });
    }
    match candidate {
        Some((length, parent)) if *length == possible_segments.len() => {
            if *parent != possible_parent {
                return Err(HirFunctionIdentityError::LexicalIdentity {
                    function: raw_index(function),
                });
            }
        }
        Some((length, _)) if *length > possible_segments.len() => {}
        _ => *candidate = Some((possible_segments.len(), possible_parent)),
    }
    Ok(())
}

fn function_lexical_parent(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    parent: FunctionId,
) -> Result<LexicalCallableParent, HirFunctionIdentityError> {
    let invalid = || HirFunctionIdentityError::LexicalIdentity {
        function: raw_index(function),
    };
    let identity = identities.get(local_index(parent)).ok_or_else(invalid)?;
    match identity {
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

fn function_definition_owner(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    parent: FunctionId,
) -> Result<DefinitionOwnerAtom, HirFunctionIdentityError> {
    let invalid = || HirFunctionIdentityError::LexicalIdentity {
        function: raw_index(function),
    };
    let identity = identities.get(local_index(parent)).ok_or_else(invalid)?;
    match identity {
        HirFunctionIdentity::Source(identity) => Ok(identity.definition_owner()),
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
            Ok(DefinitionOwnerAtom::PropertyAccessor(id))
        }
        HirFunctionIdentity::LexicalGenerated(record)
        | HirFunctionIdentity::Initialization { record, .. } => {
            Ok(DefinitionOwnerAtom::GeneratedCallable(record.id()))
        }
        HirFunctionIdentity::DerivedEquality(_) => Err(invalid()),
    }
}

fn root_definition_owner(
    inputs: &HirFunctionIdentityInputs<'_>,
    identities: &[HirFunctionIdentity],
    function: FunctionId,
    root: LexicalDefinitionRoot,
) -> Result<DefinitionOwnerAtom, HirFunctionIdentityError> {
    let invalid = || HirFunctionIdentityError::LexicalIdentity {
        function: raw_index(function),
    };
    match root {
        LexicalDefinitionRoot::Function(root) => {
            function_definition_owner(inputs, identities, function, root)
        }
        LexicalDefinitionRoot::ClassConstructor(root) => {
            if local_index(root) >= inputs.class_constructors.len() {
                return Err(invalid());
            }
            let identity = &inputs.constructor_identities[root];
            if let Some(record) = identity.source_record() {
                Ok(DefinitionOwnerAtom::Constructor(record.id()))
            } else if let Some(record) = identity.generated_record() {
                Ok(DefinitionOwnerAtom::GeneratedCallable(record.id()))
            } else {
                Err(invalid())
            }
        }
        LexicalDefinitionRoot::StructConstructor(root) => {
            if local_index(root) >= inputs.struct_constructors.len() {
                return Err(invalid());
            }
            Ok(DefinitionOwnerAtom::Constructor(
                inputs.constructor_identities[root].id(),
            ))
        }
        LexicalDefinitionRoot::VariantConstructor(root) => {
            let enumeration = root.enumeration();
            if local_index(enumeration) >= inputs.enums.len()
                || root.local_index() as usize >= inputs.enums[enumeration].variants.len()
            {
                return Err(invalid());
            }
            Ok(DefinitionOwnerAtom::EnumVariant(
                inputs.enum_member_identities[root].id(),
            ))
        }
    }
}

fn root_lexical_parent(
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
