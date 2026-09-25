use super::*;

impl Collector<'_> {
    pub(super) fn roots(
        &mut self,
        output: &LocalConcreteHirOutput,
    ) -> Result<(), MaterializedTypeClosureError> {
        let module = self.module;
        for (_, function) in module.functions.iter() {
            match &function.kind {
                FunctionKind::User(body) => {
                    self.signature(function)?;
                    self.locals(&body.locals)?;
                }
                FunctionKind::Extern(_) => self.signature(function)?,
                FunctionKind::Intrinsic(_) => continue,
            }
        }
        for (_, function) in module.extern_functions.iter() {
            self.types(function.params.iter().copied())?;
            self.add(function.return_type)?;
        }
        for (_, constructor) in module.class_constructors.iter() {
            self.add(module.classes[constructor.class].canonical_type)?;
            // The paired initializer receives the allocated object and
            // returns Unit, independently of the allocation result type.
            self.add(module.unit)?;
            self.types(constructor.parameters.iter().map(|p| p.ty))?;
            self.locals(&constructor.body().locals)?;
        }
        for (_, constructor) in module.struct_constructors.iter() {
            self.add(module.structs[constructor.structure].canonical_type)?;
            self.types(constructor.parameters.iter().map(|p| p.ty))?;
            if let StructConstructorKind::Secondary {
                arguments, body, ..
            } = &constructor.kind
            {
                self.locals(&arguments.locals)?;
                self.locals(&body.locals)?;
            }
        }
        for (_, global) in module.globals.iter() {
            self.add(global.ty)?;
        }
        for (_, root) in module.singleton_published_roots.iter() {
            self.add(root.ty)?;
        }
        // These are physical local declarations. Enum instances are demand
        // driven: their canonical types enter through a use or shape root.
        for (_, declaration) in module.structs.iter() {
            self.add(declaration.canonical_type)?;
        }
        for (_, declaration) in module.classes.iter() {
            self.add(declaration.canonical_type)?;
        }
        for (_, declaration) in module.interfaces.iter() {
            self.add(declaration.canonical_type)?;
        }
        for root in output.materialization().roots() {
            self.add(root.ty())?;
        }
        if !module.initialization_units.is_empty() {
            // Each actual ensure formats its cycle message in this type.
            self.add(module.string)?;
        }
        Ok(())
    }

    fn signature(&mut self, function: &Function) -> Result<(), MaterializedTypeClosureError> {
        self.types(function.receiver.value_type())?;
        self.types(function.params.iter().map(|p| p.ty))?;
        self.add(function.return_ty)
    }

    fn locals(
        &mut self,
        locals: &la_arena::Arena<Local>,
    ) -> Result<(), MaterializedTypeClosureError> {
        // Includes catch bindings, pattern bindings and hidden captures even
        // when no expression subsequently reads the local value.
        self.types(locals.iter().map(|(_, local)| local.ty))
    }
}
