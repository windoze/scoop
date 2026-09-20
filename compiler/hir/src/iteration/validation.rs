use std::collections::HashSet;
use std::fmt;

use super::*;

mod binding;
mod definitions;
mod types;
mod uses;

type Check<T = ()> = Result<T, InvalidIterationPlan>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidIterationPlan {
    message: &'static str,
}

impl InvalidIterationPlan {
    pub const fn message(self) -> &'static str {
        self.message
    }
}

impl fmt::Display for InvalidIterationPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for InvalidIterationPlan {}

pub fn validate_iteration_plans(module: &Module) -> Check {
    let CoreProtocols::Defined(protocols) = &module.core_protocols else {
        return validate_imported_iteration_absence(module);
    };
    let checked_core = IterationCore::checked(
        &module.interfaces,
        &module.interface_applications,
        &module.interface_methods,
        &module.functions,
        &module.enums,
        &module.enum_applications,
        &module.types,
        protocols.option,
        protocols.iteration.iterator(),
        protocols.iteration.next(),
    )
    .ok_or_else(|| invalid("invalid canonical Iterator core relation"))?;
    if checked_core != protocols.iteration {
        return fail("iteration core does not match its checked identities");
    }

    for (_, function) in module.functions.iter() {
        if let FunctionKind::User(body) = &function.kind {
            let parameters = function
                .type_params()
                .into_iter()
                .map(|parameter| parameter.id)
                .collect();
            Validator::new(
                module,
                protocols,
                &body.locals,
                parameters,
                function.is_suspend,
            )
            .validate_with_predefined(
                body.statements.as_slice(),
                function.params.iter().map(|parameter| parameter.local),
            )?;
        }
    }
    for (_, application) in module.derived_equality_applications.iter() {
        let function = checked_arena(&module.functions, application.function)
            .ok_or_else(|| invalid("derived equality body has an invalid function identity"))?;
        let parameters = function
            .type_params()
            .into_iter()
            .map(|parameter| parameter.id)
            .collect();
        Validator::new(
            module,
            protocols,
            &application.body.locals,
            parameters,
            false,
        )
        .validate_with_predefined(
            application.body.statements.as_slice(),
            function.params.iter().map(|parameter| parameter.local),
        )?;
    }
    for (expression_id, expression) in module.export_default_exprs.iter() {
        let owner_allows_suspend = default_expression_allows_suspend(module, expression_id)?;
        if expression.allows_suspend != owner_allows_suspend {
            return fail("export default suspend permission differs from its owner");
        }
        Validator::new(
            module,
            protocols,
            &expression.locals,
            expression.type_parameters.clone(),
            expression.allows_suspend,
        )
        .validate_with_predefined(
            expression.statements.as_slice(),
            expression
                .receiver
                .iter()
                .map(|receiver| receiver.local)
                .chain(
                    expression
                        .value_parameters
                        .iter()
                        .map(|parameter| parameter.local),
                ),
        )?;
    }
    validate_class_constructor_regions(module, protocols)?;
    validate_struct_constructor_regions(module, protocols)?;
    Ok(())
}

fn validate_imported_iteration_absence(module: &Module) -> Check {
    let contains_for = module
        .functions
        .iter()
        .filter_map(|(_, function)| match &function.kind {
            FunctionKind::User(body) => Some(body.statements.as_slice()),
            FunctionKind::Intrinsic(_)
            | FunctionKind::Extern(_)
            | FunctionKind::DerivedEquality => None,
        })
        .chain(
            module
                .derived_equality_applications
                .iter()
                .map(|(_, application)| application.body.statements.as_slice()),
        )
        .chain(
            module
                .export_default_exprs
                .iter()
                .map(|(_, expression)| expression.statements.as_slice()),
        )
        .any(statements_contain_for)
        || module
            .class_constructors
            .iter()
            .any(|(_, constructor)| class_constructor_contains_for(constructor))
        || module
            .struct_constructors
            .iter()
            .any(|(_, constructor)| struct_constructor_contains_for(constructor));
    if contains_for {
        fail("an imported-core HIR graph cannot contain a local iteration plan")
    } else {
        Ok(())
    }
}

fn class_constructor_contains_for(constructor: &ClassConstructor) -> bool {
    match &constructor.kind {
        ClassConstructorKind::Primary {
            base,
            common_initialization,
            ..
        } => {
            base_initialization_contains_for(base)
                || class_initialization_contains_for(common_initialization)
        }
        ClassConstructorKind::Secondary { delegation, body } => {
            let delegation_contains_for = match delegation {
                ClassSecondaryDelegation::This { arguments, .. } => {
                    statements_contain_for(&arguments.statements)
                }
                ClassSecondaryDelegation::Terminal {
                    base,
                    common_initialization,
                } => {
                    base_initialization_contains_for(base)
                        || class_initialization_contains_for(common_initialization)
                }
            };
            delegation_contains_for || statements_contain_for(&body.statements)
        }
    }
}

fn struct_constructor_contains_for(constructor: &StructConstructor) -> bool {
    match &constructor.kind {
        StructConstructorKind::Primary => false,
        StructConstructorKind::Secondary {
            delegation, body, ..
        } => {
            statements_contain_for(&delegation.arguments.statements)
                || statements_contain_for(&body.statements)
        }
    }
}

fn base_initialization_contains_for(base: &BaseInitialization) -> bool {
    match base {
        BaseInitialization::Root => false,
        BaseInitialization::Super { arguments, .. } => {
            statements_contain_for(&arguments.statements)
        }
    }
}

fn class_initialization_contains_for(steps: &[ClassInitializationStep]) -> bool {
    steps.iter().any(|step| match step {
        ClassInitializationStep::StoredProperty { initializer, .. }
        | ClassInitializationStep::DelegatedProperty { initializer, .. } => {
            statements_contain_for(&initializer.statements)
        }
        ClassInitializationStep::InitBlock { body, .. } => statements_contain_for(&body.statements),
    })
}

fn statements_contain_for(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match &statement.kind {
        StatementKind::For(_) => true,
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => {
            statements_contain_for(then_body)
                || else_body.as_deref().is_some_and(statements_contain_for)
        }
        StatementKind::While {
            condition_setup,
            body,
            ..
        } => statements_contain_for(condition_setup) || statements_contain_for(body),
        StatementKind::When(when) => {
            when.arms.iter().any(|arm| {
                arm.guard
                    .as_ref()
                    .is_some_and(|guard| statements_contain_for(&guard.setup))
                    || statements_contain_for(&arm.body)
            }) || match &when.fallback {
                WhenFallback::Else(body) => statements_contain_for(body),
                WhenFallback::Impossible(_) => false,
            }
        }
        StatementKind::Try(try_) => {
            statements_contain_for(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| statements_contain_for(&catch.body))
                || try_
                    .finally_body
                    .as_deref()
                    .is_some_and(statements_contain_for)
        }
        StatementKind::Expr(_)
        | StatementKind::InitializationEnsure(_)
        | StatementKind::LocalFunction(_)
        | StatementKind::Return { .. }
        | StatementKind::ValDecl { .. }
        | StatementKind::Assign { .. }
        | StatementKind::Break { .. }
        | StatementKind::Continue { .. }
        | StatementKind::Throw(_) => false,
    })
}

fn default_expression_allows_suspend(
    module: &Module,
    expression: ExportDefaultExprId,
) -> Check<bool> {
    let mut contract = None;
    for interface in &module.source_parameter_interfaces {
        for parameter in &interface.parameters {
            let source = match parameter.calling {
                ExportParameterCalling::Default { source, .. }
                | ExportParameterCalling::Vararg {
                    omission: ExportVarargOmission::Default(source),
                    ..
                } => source,
                ExportParameterCalling::Required { .. }
                | ExportParameterCalling::Vararg {
                    omission: ExportVarargOmission::EmptyArray,
                    ..
                } => continue,
            };
            let source = checked_arena(&module.export_default_sources, source)
                .ok_or_else(|| invalid("source parameter has an invalid default source"))?;
            checked_arena(&module.export_default_exprs, source.expression)
                .ok_or_else(|| invalid("default source has an invalid expression"))?;
            if source.expression != expression {
                continue;
            }
            let allows_suspend = parameter_owner_allows_suspend(module, interface.owner)?;
            if contract.is_some_and(|contract| contract != allows_suspend) {
                return fail("export default has inconsistent owner suspend contracts");
            }
            contract = Some(allows_suspend);
        }
    }
    contract.ok_or_else(|| invalid("export default expression has no source-parameter owner"))
}

fn parameter_owner_allows_suspend(module: &Module, owner: ExportParameterOwner) -> Check<bool> {
    match owner {
        ExportParameterOwner::Function(function) => checked_arena(&module.functions, function)
            .map(|function| function.is_suspend)
            .ok_or_else(|| invalid("default expression has an invalid function owner")),
        ExportParameterOwner::StructConstructor(constructor) => {
            checked_arena(&module.struct_constructors, constructor)
                .ok_or_else(|| invalid("default expression has an invalid struct constructor"))?;
            Ok(false)
        }
        ExportParameterOwner::ClassConstructor(constructor) => {
            checked_arena(&module.class_constructors, constructor)
                .ok_or_else(|| invalid("default expression has an invalid class constructor"))?;
            Ok(false)
        }
        ExportParameterOwner::VariantConstructor(variant) => {
            if EnumVariantRef::checked(&module.enums, variant.enumeration(), variant.local_index())
                != Some(variant)
            {
                return fail("default expression has an invalid variant constructor");
            }
            Ok(false)
        }
    }
}

fn validate_class_constructor_regions(module: &Module, protocols: &DefinedCoreProtocols) -> Check {
    for (_, constructor) in module.class_constructors.iter() {
        let owner = checked_arena(&module.classes, constructor.owner)
            .ok_or_else(|| invalid("class constructor has an invalid owner"))?;
        let parameters = owner
            .type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        match &constructor.kind {
            ClassConstructorKind::Primary {
                base,
                common_initialization,
                ..
            } => {
                validate_base_initialization(module, protocols, base, &parameters)?;
                validate_class_initialization(
                    module,
                    protocols,
                    common_initialization,
                    &parameters,
                )?;
            }
            ClassConstructorKind::Secondary { delegation, body } => {
                validate_class_delegation(module, protocols, delegation, &parameters)?;
                Validator::new(module, protocols, &body.locals, parameters, false)
                    .validate(body.statements.as_slice())?;
            }
        }
    }
    Ok(())
}

fn validate_class_delegation(
    module: &Module,
    protocols: &DefinedCoreProtocols,
    delegation: &ClassSecondaryDelegation,
    parameters: &[TypeParamId],
) -> Check {
    match delegation {
        ClassSecondaryDelegation::This { arguments, .. } => {
            validate_constructor_arguments(module, protocols, arguments, parameters)
        }
        ClassSecondaryDelegation::Terminal {
            base,
            common_initialization,
        } => {
            validate_base_initialization(module, protocols, base, parameters)?;
            validate_class_initialization(module, protocols, common_initialization, parameters)
        }
    }
}

fn validate_base_initialization(
    module: &Module,
    protocols: &DefinedCoreProtocols,
    base: &BaseInitialization,
    parameters: &[TypeParamId],
) -> Check {
    match base {
        BaseInitialization::Root => Ok(()),
        BaseInitialization::Super { arguments, .. } => {
            validate_constructor_arguments(module, protocols, arguments, parameters)
        }
    }
}

fn validate_class_initialization(
    module: &Module,
    protocols: &DefinedCoreProtocols,
    steps: &[ClassInitializationStep],
    parameters: &[TypeParamId],
) -> Check {
    for step in steps {
        match step {
            ClassInitializationStep::StoredProperty { initializer, .. }
            | ClassInitializationStep::DelegatedProperty { initializer, .. } => {
                Validator::new(
                    module,
                    protocols,
                    &initializer.locals,
                    parameters.to_vec(),
                    false,
                )
                .validate(initializer.statements.as_slice())?;
            }
            ClassInitializationStep::InitBlock { body, .. } => {
                Validator::new(module, protocols, &body.locals, parameters.to_vec(), false)
                    .validate(body.statements.as_slice())?;
            }
        }
    }
    Ok(())
}

fn validate_constructor_arguments(
    module: &Module,
    protocols: &DefinedCoreProtocols,
    arguments: &ConstructorArguments,
    parameters: &[TypeParamId],
) -> Check {
    Validator::new(
        module,
        protocols,
        &arguments.locals,
        parameters.to_vec(),
        false,
    )
    .validate(arguments.statements.as_slice())
}

fn validate_struct_constructor_regions(module: &Module, protocols: &DefinedCoreProtocols) -> Check {
    for (_, constructor) in module.struct_constructors.iter() {
        let owner = checked_arena(&module.structs, constructor.owner)
            .ok_or_else(|| invalid("struct constructor has an invalid owner"))?;
        let parameters = owner
            .type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        if let StructConstructorKind::Secondary {
            delegation, body, ..
        } = &constructor.kind
        {
            validate_constructor_arguments(module, protocols, &delegation.arguments, &parameters)?;
            Validator::new(module, protocols, &body.locals, parameters, false)
                .validate(body.statements.as_slice())?;
        }
    }
    Ok(())
}

struct Validator<'a> {
    module: &'a Module,
    protocols: &'a DefinedCoreProtocols,
    locals: &'a Arena<Local>,
    parameters: Vec<TypeParamId>,
    allows_suspend: bool,
    loops: Vec<LoopId>,
    loop_identities: HashSet<u32>,
}

impl<'a> Validator<'a> {
    fn new(
        module: &'a Module,
        protocols: &'a DefinedCoreProtocols,
        locals: &'a Arena<Local>,
        parameters: Vec<TypeParamId>,
        allows_suspend: bool,
    ) -> Self {
        Self {
            module,
            protocols,
            locals,
            parameters,
            allows_suspend,
            loops: Vec::new(),
            loop_identities: HashSet::new(),
        }
    }

    fn validate(mut self, statements: &[Statement]) -> Check {
        self.validate_statements(statements)?;
        if !self.loops.is_empty() {
            return fail("iteration validation left an active loop target");
        }
        self.validate_iteration_definition_schedules(statements, std::iter::empty())
    }

    fn validate_with_predefined(
        mut self,
        statements: &[Statement],
        predefined: impl IntoIterator<Item = LocalId>,
    ) -> Check {
        let predefined = predefined.into_iter().collect::<Vec<_>>();
        self.validate_statements(statements)?;
        if !self.loops.is_empty() {
            return fail("iteration validation left an active loop target");
        }
        self.validate_iteration_definition_schedules(statements, predefined)
    }

    fn validate_statements(&mut self, statements: &[Statement]) -> Check {
        for statement in statements {
            self.validate_statement(statement)?;
        }
        Ok(())
    }

    fn validate_statement(&mut self, statement: &Statement) -> Check {
        match &statement.kind {
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                self.validate_statements(then_body)?;
                if let Some(else_body) = else_body {
                    self.validate_statements(else_body)?;
                }
            }
            StatementKind::While {
                target,
                condition_setup,
                body,
                ..
            } => {
                self.enter_loop(*target)?;
                self.validate_statements(condition_setup)?;
                self.validate_statements(body)?;
                self.leave_loop(*target)?;
            }
            StatementKind::For(plan) => self.validate_for(plan)?,
            StatementKind::Break { target } | StatementKind::Continue { target } => {
                if self.loops.last() != Some(target) {
                    return fail("break or continue does not target the innermost active loop");
                }
            }
            StatementKind::When(value) => {
                for arm in &value.arms {
                    if let Some(guard) = &arm.guard {
                        self.validate_statements(&guard.setup)?;
                    }
                    self.validate_statements(&arm.body)?;
                }
                if let WhenFallback::Else(body) = &value.fallback {
                    self.validate_statements(body)?;
                }
            }
            StatementKind::Try(value) => {
                self.validate_statements(&value.body)?;
                for catch in &value.catches {
                    self.validate_statements(&catch.body)?;
                }
                if let Some(body) = &value.finally_body {
                    self.validate_statements(body)?;
                }
            }
            StatementKind::Expr(_)
            | StatementKind::InitializationEnsure(_)
            | StatementKind::LocalFunction(_)
            | StatementKind::Return { .. }
            | StatementKind::ValDecl { .. }
            | StatementKind::Assign { .. }
            | StatementKind::Throw(_) => {}
        }
        Ok(())
    }

    fn validate_for(&mut self, plan: &ForIterationPlan) -> Check {
        let conformance = plan.conformance();
        let next = plan.next();
        let reserved = [
            plan.source(),
            conformance.source(),
            conformance.iterator(),
            next.result(),
            next.element(),
        ];
        for temporary in reserved {
            self.check_temporary(temporary, "for compiler temporary")?;
        }
        let distinct = reserved
            .iter()
            .map(|temporary| temporary.local.into_raw().into_u32())
            .collect::<HashSet<_>>();
        if distinct.len() != reserved.len() {
            return fail("for compiler temporaries are not pairwise distinct");
        }

        let source_definitions = self.setup_definitions(plan.source_setup())?;
        if source_definitions
            .iter()
            .any(|local| distinct.contains(local))
        {
            return fail("for source setup aliases a reserved temporary");
        }
        let mut occupied = distinct.clone();
        occupied.extend(source_definitions);
        self.validate_statements(plan.source_setup())?;
        if plan.source().ty != plan.source_init().ty {
            return fail("for source temporary and initializer types differ");
        }

        self.validate_statements(plan.iterator_setup())?;
        let derived = self.validate_receiver_setup(
            plan.iterator_setup(),
            plan.source(),
            &mut occupied,
            "iterator",
        )?;
        if conformance.source().ty != plan.iterator_call().ty {
            return fail("iterator call and result temporary types differ");
        }
        self.validate_protocol_call(
            plan.iterator_call(),
            &derived,
            OperatorKind::Iterator,
            "iterator",
        )?;

        let application = self.checked_interface_application(conformance.application())?;
        if application.template != self.protocols.iteration.iterator()
            || application.arguments.len() != 1
            || conformance.iterator().ty != application.canonical_type
        {
            return fail("for conformance does not name one exact Iterator<E> application");
        }
        let exact = self.exact_interface_applications(
            conformance.source().ty,
            self.protocols.iteration.iterator(),
        )?;
        if exact.as_slice() != [conformance.application()] {
            return fail("for conformance is not the unique exact Iterator application");
        }
        let element = application.arguments[0];

        if next.element().ty != element {
            return fail("iteration element temporary does not have Iterator<E>'s element type");
        }
        let method = checked_arena(&self.module.method_applications, next.callable())
            .ok_or_else(|| invalid("next plan has an invalid method application"))?;
        let next_function = checked_arena(
            &self.module.interface_methods,
            self.protocols.iteration.next(),
        )
        .ok_or_else(|| invalid("iteration core has an invalid next member"))?
        .function;
        if method.function != next_function
            || method.owner != MethodOwnerApplication::Interface(conformance.application())
        {
            return fail("next plan does not call the canonical Iterator<E>.next member");
        }
        self.validate_next_callable(next.callable(), conformance.iterator().ty, next.result().ty)?;

        let option = AppliedOptionCore::checked(
            &self.module.enums,
            &self.module.enum_applications,
            &self.module.types,
            self.protocols.option,
            element,
            next.option().some_payload(),
            next.option().none(),
        )
        .ok_or_else(|| invalid("next plan has an invalid applied Option relation"))?;
        if option != next.option() {
            return fail("next plan Option relation is not canonical");
        }
        let option_application =
            checked_arena(&self.module.enum_applications, option.application())
                .ok_or_else(|| invalid("next plan Option application is invalid"))?;
        if next.result().ty != option_application.canonical_type {
            return fail("next result temporary does not have exact Option<E> type");
        }
        if plan.binding().subject != next.element() {
            return fail("for binding subject is not the exact iteration element temporary");
        }

        self.enter_loop(plan.target())?;
        self.validate_binding_plan(plan.binding(), &occupied)?;
        self.validate_statements(plan.body())?;
        self.leave_loop(plan.target())
    }

    fn setup_definitions(&self, statements: &[Statement]) -> Check<HashSet<u32>> {
        let mut definitions = HashSet::new();
        self.collect_statement_definitions(statements, &mut definitions)?;
        Ok(definitions)
    }

    fn collect_statement_definitions(
        &self,
        statements: &[Statement],
        definitions: &mut HashSet<u32>,
    ) -> Check {
        for statement in statements {
            match &statement.kind {
                StatementKind::ValDecl { pattern, .. } => {
                    self.collect_pattern_definitions(pattern, definitions)?;
                }
                StatementKind::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    let mut then_definitions = definitions.clone();
                    self.collect_statement_definitions(then_body, &mut then_definitions)?;
                    let mut else_definitions = definitions.clone();
                    if let Some(else_body) = else_body {
                        self.collect_statement_definitions(else_body, &mut else_definitions)?;
                    }
                    definitions.extend(then_definitions);
                    definitions.extend(else_definitions);
                }
                StatementKind::While {
                    condition_setup,
                    body,
                    ..
                } => {
                    let mut loop_definitions = definitions.clone();
                    self.collect_statement_definitions(condition_setup, &mut loop_definitions)?;
                    self.collect_statement_definitions(body, &mut loop_definitions)?;
                    definitions.extend(loop_definitions);
                }
                StatementKind::For(plan) => {
                    for temporary in [
                        plan.source(),
                        plan.conformance().source(),
                        plan.conformance().iterator(),
                        plan.next().result(),
                        plan.next().element(),
                    ] {
                        self.record_definition(temporary.local, definitions)?;
                    }
                    self.collect_statement_definitions(plan.source_setup(), definitions)?;
                    self.collect_statement_definitions(plan.iterator_setup(), definitions)?;
                    self.collect_binding_definitions(plan.binding(), definitions)?;
                    self.collect_statement_definitions(plan.body(), definitions)?;
                }
                StatementKind::When(value) => {
                    let incoming = definitions.clone();
                    for arm in &value.arms {
                        let mut arm_definitions = incoming.clone();
                        self.collect_pattern_definitions(&arm.pattern, &mut arm_definitions)?;
                        if let Some(guard) = &arm.guard {
                            self.collect_statement_definitions(&guard.setup, &mut arm_definitions)?;
                        }
                        self.collect_statement_definitions(&arm.body, &mut arm_definitions)?;
                        definitions.extend(arm_definitions);
                    }
                    if let WhenFallback::Else(body) = &value.fallback {
                        let mut fallback_definitions = incoming;
                        self.collect_statement_definitions(body, &mut fallback_definitions)?;
                        definitions.extend(fallback_definitions);
                    }
                }
                StatementKind::Try(value) => {
                    let incoming = definitions.clone();
                    let mut body_definitions = incoming.clone();
                    self.collect_statement_definitions(&value.body, &mut body_definitions)?;
                    definitions.extend(body_definitions);
                    for catch in &value.catches {
                        let mut catch_definitions = incoming.clone();
                        self.record_definition(catch.local, &mut catch_definitions)?;
                        self.collect_statement_definitions(&catch.body, &mut catch_definitions)?;
                        definitions.extend(catch_definitions);
                    }
                    if let Some(body) = &value.finally_body {
                        self.collect_statement_definitions(body, definitions)?;
                    }
                }
                StatementKind::Expr(_)
                | StatementKind::InitializationEnsure(_)
                | StatementKind::LocalFunction(_)
                | StatementKind::Return { .. }
                | StatementKind::Assign { .. }
                | StatementKind::Break { .. }
                | StatementKind::Continue { .. }
                | StatementKind::Throw(_) => {}
            }
        }
        Ok(())
    }

    fn collect_pattern_definitions(
        &self,
        pattern: &Pattern,
        definitions: &mut HashSet<u32>,
    ) -> Check {
        match pattern {
            Pattern::Binding { local } => self.record_definition(*local, definitions),
            Pattern::Variant { fields, .. } | Pattern::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    self.collect_pattern_definitions(pattern, definitions)?;
                }
                Ok(())
            }
            Pattern::Tuple(elements) => {
                for pattern in elements {
                    self.collect_pattern_definitions(pattern, definitions)?;
                }
                Ok(())
            }
            Pattern::Wildcard | Pattern::Literal { .. } => Ok(()),
        }
    }

    fn collect_binding_definitions(
        &self,
        plan: &IrrefutableBindingPlan,
        definitions: &mut HashSet<u32>,
    ) -> Check {
        for action in &plan.actions {
            match action {
                IrrefutableBindingAction::Project { result, .. } => {
                    self.record_definition(result.local, definitions)?;
                }
                IrrefutableBindingAction::Component { result, setup, .. } => {
                    self.collect_statement_definitions(setup, definitions)?;
                    self.record_definition(result.local, definitions)?;
                }
                IrrefutableBindingAction::Bind { target, .. } => {
                    self.record_definition(target.local, definitions)?;
                }
            }
        }
        Ok(())
    }

    fn record_definition(&self, local: LocalId, definitions: &mut HashSet<u32>) -> Check {
        checked_arena(self.locals, local)
            .ok_or_else(|| invalid("iteration setup binds an invalid local"))?;
        if !definitions.insert(local.into_raw().into_u32()) {
            return fail("iteration setup reuses an already defined local");
        }
        Ok(())
    }

    fn enter_loop(&mut self, target: LoopId) -> Check {
        if !self.loop_identities.insert(target.into_raw()) {
            return fail("a callable reuses one loop identity");
        }
        self.loops.push(target);
        Ok(())
    }

    fn leave_loop(&mut self, target: LoopId) -> Check {
        if self.loops.pop() != Some(target) {
            return fail("loop target stack is not properly nested");
        }
        Ok(())
    }

    fn check_temporary(&self, temporary: BindingTemporary, role: &'static str) -> Check {
        let local = checked_arena(self.locals, temporary.local).ok_or_else(|| invalid(role))?;
        if !arena_contains(&self.module.types, temporary.ty)
            || local.ty != temporary.ty
            || local.mutable
        {
            return fail(role);
        }
        Ok(())
    }

    fn check_leaf(&self, leaf: BindingLeaf) -> Check {
        let local = checked_arena(self.locals, leaf.local)
            .ok_or_else(|| invalid("binding leaf has an invalid local"))?;
        if !arena_contains(&self.module.types, leaf.ty)
            || local.ty != leaf.ty
            || local.mutable != leaf.mutability.is_mutable()
        {
            return fail("binding leaf type or mutability differs from its local");
        }
        Ok(())
    }
}

fn invalid(message: &'static str) -> InvalidIterationPlan {
    InvalidIterationPlan { message }
}

fn fail<T>(message: &'static str) -> Check<T> {
    Err(invalid(message))
}

fn checked_arena<T>(arena: &Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    arena_contains(arena, id).then(|| &arena[id])
}
