use super::*;

impl Collector<'_> {
    pub(super) fn roots(
        &mut self,
        output: &LocalConcreteHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        let module = self.module;
        for (_, function) in module.functions.iter() {
            meter.charge_work(1, &WirePath::root())?;
            match &function.kind {
                FunctionKind::User(body) => {
                    self.signature(function, meter)?;
                    self.locals(&body.locals, meter)?;
                }
                FunctionKind::Extern(_) => self.signature(function, meter)?,
                FunctionKind::Intrinsic(_) => continue,
            }
        }
        for (_, function) in module.extern_functions.iter() {
            self.types(function.params.iter().copied(), 1, meter)?;
            self.add(function.return_type, 1, meter)?;
        }
        for (_, constructor) in module.class_constructors.iter() {
            self.add(module.classes[constructor.class].canonical_type, 1, meter)?;
            // The paired initializer receives the allocated object and
            // returns Unit, independently of the allocation result type.
            self.add(module.unit, 1, meter)?;
            self.types(constructor.parameters.iter().map(|p| p.ty), 1, meter)?;
            self.locals(&constructor.body().locals, meter)?;
        }
        for (_, constructor) in module.struct_constructors.iter() {
            self.add(
                module.structs[constructor.structure].canonical_type,
                1,
                meter,
            )?;
            self.types(constructor.parameters.iter().map(|p| p.ty), 1, meter)?;
            if let StructConstructorKind::Secondary {
                arguments, body, ..
            } = &constructor.kind
            {
                self.locals(&arguments.locals, meter)?;
                self.locals(&body.locals, meter)?;
            }
        }
        for (_, global) in module.globals.iter() {
            self.add(global.ty, 1, meter)?;
        }
        for (_, root) in module.singleton_published_roots.iter() {
            self.add(root.ty, 1, meter)?;
        }
        // These are physical local declarations. Enum instances are demand
        // driven: their canonical types enter through a use or shape root.
        for (_, declaration) in module.structs.iter() {
            self.add(declaration.canonical_type, 1, meter)?;
        }
        for (_, declaration) in module.classes.iter() {
            self.add(declaration.canonical_type, 1, meter)?;
        }
        for (_, declaration) in module.interfaces.iter() {
            self.add(declaration.canonical_type, 1, meter)?;
        }
        for root in output.materialization().roots() {
            self.add(root.ty(), 1, meter)?;
        }
        if !module.initialization_units.is_empty() {
            // Each actual ensure formats its cycle message in this type.
            self.add(module.string, 1, meter)?;
        }
        Ok(())
    }

    fn signature(
        &mut self,
        function: &Function,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        self.types(function.receiver.value_type(), 1, meter)?;
        self.types(function.params.iter().map(|p| p.ty), 1, meter)?;
        self.add(function.return_ty, 1, meter)
    }

    fn locals(
        &mut self,
        locals: &la_arena::Arena<Local>,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        // Includes catch bindings, pattern bindings and hidden captures even
        // when no expression subsequently reads the local value.
        self.types(locals.iter().map(|(_, local)| local.ty), 1, meter)
    }
}
