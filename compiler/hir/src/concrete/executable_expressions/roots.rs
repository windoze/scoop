use super::*;

pub(super) enum Root<'a> {
    Body(&'a [Statement]),
    StructConstructor {
        arguments: &'a ConstructorArguments,
        body: &'a Body,
    },
}

impl<'a> Root<'a> {
    pub(super) fn schedule(self, traversal: &mut Traversal<'a, '_>) -> Result<(), StructureError> {
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
    meter: &mut BudgetMeter,
) -> Result<Vec<(CallableMaterialization, Root<'a>)>, StructureError> {
    let path = WirePath::root();
    let mut roots = Vec::new();
    meter.charge_work(
        1_u64
            .saturating_add(module.functions.len() as u64)
            .saturating_add(module.class_constructors.len() as u64)
            .saturating_add(module.struct_constructors.len() as u64),
        &path,
    )?;
    let mut add = |key, root| -> Result<(), StructureError> {
        meter.charge_work(1, &path)?;
        meter.charge_owned_bytes(
            std::mem::size_of::<(CallableMaterialization, Root<'_>)>() as u64,
            &path,
        )?;
        meter.try_reserve_collection_slots(&mut roots, 1, &path)?;
        roots.push((key, root));
        Ok(())
    };
    for (_, function) in module.functions.iter() {
        if let FunctionKind::User(body) = &function.kind {
            add(function.materialization, Root::Body(&body.statements))?;
        }
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
    let count = roots.len() as u64;
    meter.charge_work(
        count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
        &path,
    )?;
    roots.sort_unstable_by_key(|(key, _)| *key);
    if let Some(pair) = roots.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(StructureError::DuplicateRoot(pair[0].0));
    }
    Ok(roots)
}
