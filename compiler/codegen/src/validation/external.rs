use super::*;

pub(super) fn validate_external_metadata(module: &Module) -> Result<(), CodegenError> {
    validate_callable_metadata(module)?;
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

fn validate_callable_metadata(module: &Module) -> Result<(), CodegenError> {
    let mut bodies = HashSet::new();
    let local_bodies = module
        .callable_bodies()
        .map(|function| function.callable_body.id())
        .collect::<HashSet<_>>();
    for (_, callable) in module.meta.external_callables.iter() {
        if callable.provider() == module.cone {
            return Err(CodegenError(format!(
                "external callable {:?} names the current Cone as provider",
                callable.target()
            )));
        }
        if !bodies.insert(callable.body()) {
            return Err(CodegenError(format!(
                "duplicate external callable body {}",
                callable.body()
            )));
        }
        if local_bodies.contains(&callable.body()) {
            return Err(CodegenError(format!(
                "external callable body {} is also defined locally",
                callable.body()
            )));
        }
    }
    Ok(())
}
