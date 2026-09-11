use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirRuntimeTypeLocation {
    Function(FunctionTypeId),
    Class(ClassId),
    Interface(InterfaceId),
    Closure(ClosureClassId),
}

impl std::fmt::Display for MirRuntimeTypeLocation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function(id) => write!(formatter, "function type {}", id.into_raw().into_u32()),
            Self::Class(id) => write!(formatter, "class {}", id.into_raw().into_u32()),
            Self::Interface(id) => write!(formatter, "interface {}", id.into_raw().into_u32()),
            Self::Closure(id) => write!(formatter, "closure class {}", id.into_raw().into_u32()),
        }
    }
}

pub(super) fn validate_runtime_type_metadata(module: &Module) -> Result<(), MirValidationError> {
    for identity in module.meta.source_exact_types.iter() {
        let Type::Function(id) = identity.ty() else {
            continue;
        };
        if arena_get(&module.function_types, *id).is_none() {
            return invalid(
                MirRuntimeTypeLocation::Function(*id),
                "the source exact identity references a missing function type",
            );
        }
    }
    for (id, _) in module.interfaces.iter() {
        require_source(
            module,
            MirRuntimeTypeLocation::Interface(id),
            Type::Interface(id),
        )?;
    }
    for (id, class) in module.classes.iter() {
        let location = MirRuntimeTypeLocation::Class(id);
        if matches!(
            class.representation,
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String)
        ) {
            require_source(module, location, Type::String)?;
            continue;
        }
        let has_source = module
            .meta
            .source_exact_types
            .get(&Type::Class(id))
            .is_some();
        let has_generated = module
            .meta
            .generated_exact_types
            .get(GeneratedExactTypeLocation::Class(id))
            .is_some();
        require_exactly_one(location, has_source, has_generated)?;
    }
    for (id, _) in module.closure_classes.iter() {
        let location = MirRuntimeTypeLocation::Closure(id);
        let has_generated = module
            .meta
            .generated_exact_types
            .get(GeneratedExactTypeLocation::Closure(id))
            .is_some();
        if !has_generated {
            return invalid(
                location,
                "the closure class has no generated exact identity",
            );
        }
        let function_type = module.closure_classes[id].function_type;
        if module
            .meta
            .source_exact_types
            .get(&Type::Function(function_type))
            .is_none()
        {
            return invalid(
                location,
                "the closure class function type has no source exact identity",
            );
        }
    }
    Ok(())
}

fn require_source(
    module: &Module,
    location: MirRuntimeTypeLocation,
    ty: Type,
) -> Result<(), MirValidationError> {
    if module.meta.source_exact_types.get(&ty).is_none() {
        return invalid(location, "the descriptor type has no source exact identity");
    }
    Ok(())
}

fn require_exactly_one(
    location: MirRuntimeTypeLocation,
    has_source: bool,
    has_generated: bool,
) -> Result<(), MirValidationError> {
    match (has_source, has_generated) {
        (true, false) | (false, true) => Ok(()),
        (false, false) => invalid(location, "the descriptor type has no exact identity"),
        (true, true) => invalid(
            location,
            "the descriptor type has both source and generated exact identities",
        ),
    }
}

fn invalid(
    location: MirRuntimeTypeLocation,
    reason: &'static str,
) -> Result<(), MirValidationError> {
    Err(MirValidationError {
        location: MirValidationLocation::RuntimeType { location },
        kind: MirValidationErrorKind::InvalidRuntimeTypeIdentity { reason },
    })
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
