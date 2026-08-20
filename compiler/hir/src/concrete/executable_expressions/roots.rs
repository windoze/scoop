use super::*;

pub(super) enum Root<'a> {
    Body(&'a [Statement]),
    StructConstructor {
        arguments: &'a ConstructorArguments,
        body: &'a Body,
    },
}

impl<'a> Root<'a> {
    pub(super) fn schedule(self, traversal: &mut Traversal<'a>) -> Result<(), StructureError> {
        match self {
            Self::Body(body) => traversal.statements(body),
            Self::StructConstructor { arguments, body } => {
                traversal.statements(&body.statements)?;
                traversal.expressions(&arguments.args)?;
                traversal.statements(&arguments.statements)
            }
        }
    }
}

pub(super) fn collect<'a>(
    module: &'a Module,
) -> Result<Vec<(CallableMaterialization, Root<'a>)>, StructureError> {
    let path = WirePath::root();
    let mut roots = Vec::new();

    let mut add = |key, root| -> Result<(), StructureError> {
        scoop_wire::allocation::try_reserve(&mut roots, 1, &path)?;
        roots.push((key, root));
        Ok(())
    };
    for (_, function) in module.functions.iter() {
        if let FunctionKind::User(body) = &function.kind {
            add(function.materialization, Root::Body(&body.statements))?;
        }
    }
    for (_, hook) in module.release_hooks.iter() {
        add(hook.materialization, Root::Body(&hook.body.statements))?;
    }
    for (_, constructor) in module.class_constructors.iter() {
        add(
            constructor.materialization,
            Root::Body(&constructor.body().statements),
        )?;
    }
    for (_, constructor) in module.struct_constructors.iter() {
        if let StructConstructorKind::Secondary {
            arguments, body, ..
        } = &constructor.kind
        {
            add(
                constructor.materialization,
                Root::StructConstructor { arguments, body },
            )?;
        }
    }

    roots.sort_unstable_by_key(|(key, _)| *key);
    if let Some(pair) = roots.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(StructureError::DuplicateRoot(pair[0].0));
    }
    Ok(roots)
}
