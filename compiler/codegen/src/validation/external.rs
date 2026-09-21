use super::*;

pub(super) fn validate_external_metadata(module: &Module) -> Result<(), CodegenError> {
    validate_core_external_metadata(module)?;
    validate_dependency_external_metadata(module)?;
    let mut targets = HashSet::new();
    for (_, descriptor) in module.meta.external_type_descriptors.iter() {
        if descriptor.provider() == module.cone {
            return Err(CodegenError(format!(
                "external TypeDescriptor {} names the current Cone as provider",
                descriptor.target()
            )));
        }
        if !targets.insert(descriptor.target()) {
            return Err(CodegenError(format!(
                "duplicate external TypeDescriptor target {}",
                descriptor.target()
            )));
        }
        if module
            .meta
            .type_descriptors
            .iter()
            .any(|(_, local)| local.identity.exact_type() == descriptor.target())
        {
            return Err(CodegenError(format!(
                "external TypeDescriptor target {} is also defined locally",
                descriptor.target()
            )));
        }
        let expected =
            scoop_lir::ExternalTypeDescriptor::new(descriptor.provider(), descriptor.target())
                .map_err(|error| CodegenError(error.to_string()))?;
        if *descriptor != expected {
            return Err(CodegenError(format!(
                "external TypeDescriptor {} disagrees with its provider definition contract",
                descriptor.target()
            )));
        }
    }
    Ok(())
}

fn validate_core_external_metadata(module: &Module) -> Result<(), CodegenError> {
    if module.cone == scoop_lir::ConeIdentity::CORE
        && !module.meta.core_external_callables.is_empty()
    {
        return Err(CodegenError(
            "the core bootstrap Cone cannot import its own external definitions".to_string(),
        ));
    }

    let mut callable_targets = HashSet::new();
    let mut callable_bodies = HashSet::new();
    for (_, callable) in module.meta.core_external_callables.iter() {
        if !callable_targets.insert(callable.target()) {
            return Err(CodegenError(format!(
                "duplicate core external callable target {:?}",
                callable.target()
            )));
        }
        if !callable_bodies.insert(callable.body()) {
            return Err(CodegenError(format!(
                "duplicate core external callable body {}",
                callable.body()
            )));
        }
        if module
            .functions
            .iter()
            .any(|local| local.callable_body.id() == callable.body())
        {
            return Err(CodegenError(format!(
                "core external callable body {} is also defined locally",
                callable.body()
            )));
        }
    }

    Ok(())
}

fn validate_dependency_external_metadata(module: &Module) -> Result<(), CodegenError> {
    let mut declarations = HashSet::new();
    let mut bodies = HashSet::new();
    for (_, callable) in module.meta.dependency_external_callables.iter() {
        if callable.provider() == module.cone {
            return Err(CodegenError(format!(
                "dependency external callable {:?} names the current Cone as provider",
                callable.target()
            )));
        }
        if let Some(declaration) = callable.legacy_declaration()
            && !declarations.insert((callable.provider(), declaration))
        {
            return Err(CodegenError(format!(
                "duplicate dependency external callable {}:{declaration:?}",
                callable.provider()
            )));
        }
        if !bodies.insert(callable.body()) {
            return Err(CodegenError(format!(
                "duplicate dependency external callable body {}",
                callable.body()
            )));
        }
        if module
            .functions
            .iter()
            .any(|local| local.callable_body.id() == callable.body())
        {
            return Err(CodegenError(format!(
                "dependency external callable body {} is also defined locally",
                callable.body()
            )));
        }
        if module
            .meta
            .core_external_callables
            .iter()
            .any(|(_, core)| core.body() == callable.body())
        {
            return Err(CodegenError(format!(
                "dependency external callable body {} also uses trusted-core authority",
                callable.body()
            )));
        }
    }
    Ok(())
}
