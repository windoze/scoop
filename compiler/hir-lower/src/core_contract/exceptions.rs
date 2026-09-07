use super::*;

use crate::{
    call_resolution::{arguments::CandidateArgumentMap, candidates::NominalConstructorSource},
    defaults::SourceParameterOwner,
};

impl Lowerer {
    fn illegal_state_message_constructor(
        &mut self,
        class: ClassId,
    ) -> Option<hir::MessageClassConstructor> {
        let constructor = self.classes[class]
            .constructors
            .iter()
            .copied()
            .find(|constructor| {
                let parameters = &self.class_constructors[*constructor].parameters;
                parameters.len() == 1 && self.as_option(parameters[0].ty) == Some(self.string)
            });
        if constructor.is_none() {
            self.error(
                self.classes[class].span,
                "class `IllegalStateException` in scoop.core must provide a constructor whose parameter is `String?`"
                    .to_string(),
            );
        }
        constructor.map(|constructor| hir::MessageClassConstructor { class, constructor })
    }

    fn zero_source_argument_constructor(&self, class: ClassId) -> Option<hir::ClassConstructorId> {
        self.classes[class]
            .constructors
            .iter()
            .copied()
            .find(|constructor| {
                let declaration = &self.class_constructors[*constructor];
                let Some(callings) = self.class_parameter_calling.get(constructor) else {
                    return false;
                };
                if declaration.parameters.len() != callings.len() {
                    return false;
                }
                let owner = SourceParameterOwner::ClassConstructor(*constructor);
                callings
                    .iter()
                    .enumerate()
                    .all(|(index, calling)| match calling {
                        crate::FnParamCalling::Required => false,
                        crate::FnParamCalling::Default { .. } => {
                            self.default_templates.contains_key(&(owner, index as u32))
                        }
                        crate::FnParamCalling::Vararg { omission, .. } => match omission {
                            crate::FnVarargOmission::EmptyArray => true,
                            crate::FnVarargOmission::Default { .. } => {
                                self.default_templates.contains_key(&(owner, index as u32))
                            }
                        },
                    })
            })
    }

    /// Produce the physical zero-parameter callable needed by MIR's
    /// compiler-generated exception edges. A source constructor whose
    /// parameters are all omittable is adapted once in HIR, with its typed
    /// defaults materialized into a hidden `this` delegation.
    fn compiler_exception_callable(
        &mut self,
        source: hir::ClassConstructorId,
    ) -> hir::ClassConstructorId {
        if self.class_constructors[source].parameters.is_empty() {
            return source;
        }

        let class = self.class_constructors[source].owner;
        let view = self.nominal_constructor_view(NominalConstructorSource::Class(source));
        let argument_map = CandidateArgumentMap::source_nominal(&view, &[])
            .expect("a validated zero-source-argument constructor is callable without inputs");
        let span = self.class_constructors[source].span;
        let lowered = self
            .with_constructor_expression_context(
                source,
                "compiler exception zero-argument adapter",
                |this, sink| {
                    Some(this.materialize_nominal_arguments(
                        crate::argument_materialization::NominalArgumentMaterialization {
                            view: &view,
                            argument_map: &argument_map,
                            type_args: &[],
                            source_args: Vec::new(),
                            argument_sinks: Vec::new(),
                            call_span: span,
                        },
                        sink,
                    ))
                },
            )
            .expect("compiler exception adapter lowering always returns its arguments");
        let target =
            self.class_constructor_application(source, self.classes[class].self_application);
        let declaration = self.class_constructors[source].clone();
        let adapter = self.class_constructors.alloc(hir::ClassConstructor {
            owner: class,
            access: declaration.access,
            parameters: Vec::new(),
            kind: hir::ClassConstructorKind::Secondary {
                delegation: hir::ClassSecondaryDelegation::This {
                    target,
                    arguments: hir::ConstructorArguments {
                        locals: lowered.locals,
                        statements: lowered.statements,
                        args: lowered.value,
                    },
                },
                body: hir::Body {
                    locals: la_arena::Arena::new(),
                    statements: Vec::new(),
                },
            },
            span,
            origin: declaration.origin,
        });
        self.classes[class].constructors.push(adapter);
        self.class_parameter_calling.insert(adapter, Vec::new());
        adapter
    }
}

impl Lowerer {
    pub(crate) fn compiler_exception(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
        throwable: ClassId,
    ) -> Option<hir::CompilerException> {
        let candidate = match self.core_unit_nominal(name) {
            Some(crate::NominalTarget::Class(id)) => Some(id),
            _ => None,
        };
        let Some(id) = candidate else {
            self.current_file = 0;
            self.error(
                files[0].span,
                format!("scoop.core must define class `{name}`"),
            );
            return None;
        };
        self.current_file = self.class_files[&id];
        let declaration = &self.classes[id];
        let zero_arg = self.zero_source_argument_constructor(id);
        let valid = declaration.modifier == hir::ClassModifier::Final
            && declaration.type_params.is_empty()
            && declaration.is_declared()
            && zero_arg.is_some()
            && self.class_descends_from(id, throwable);
        if !valid {
            self.error(
                declaration.span,
                format!(
                    "class `{name}` in scoop.core must be a non-generic final subtype of `Throwable` callable with zero source arguments"
                ),
            );
        }
        zero_arg.map(|constructor| {
            let constructor = self.compiler_exception_callable(constructor);
            hir::CompilerException {
                constructor: hir::ZeroArgClassConstructor {
                    class: id,
                    constructor,
                },
            }
        })
    }

    pub(crate) fn validate_exception_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::CompilerExceptionCore> {
        let throwable = self.throwable.map(|(id, _)| id)?;
        self.current_file = self.class_files[&throwable];
        let declaration = &self.classes[throwable];
        let zero_arg = declaration
            .constructors
            .iter()
            .copied()
            .find(|constructor| self.class_constructors[*constructor].parameters.is_empty());
        let valid_throwable = declaration.modifier == hir::ClassModifier::Open
            && declaration.type_params.is_empty()
            && declaration.is_declared()
            && zero_arg.is_some();
        if !valid_throwable {
            self.error(
                declaration.span,
                "class `Throwable` in scoop.core must be a non-generic open class with a zero-argument constructor"
                    .to_string(),
            );
        }
        let throwable = hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor {
                class: throwable,
                constructor: zero_arg?,
            },
        };
        let illegal_state =
            self.compiler_exception("IllegalStateException", files, throwable.class())?;
        let illegal_state_message_constructor =
            self.illegal_state_message_constructor(illegal_state.class())?;
        Some(hir::CompilerExceptionCore {
            throwable,
            unwrap_exception: self.compiler_exception(
                "UnwrapException",
                files,
                throwable.class(),
            )?,
            class_cast_exception: self.compiler_exception(
                "ClassCastException",
                files,
                throwable.class(),
            )?,
            arithmetic_exception: self.compiler_exception(
                "ArithmeticException",
                files,
                throwable.class(),
            )?,
            index_out_of_bounds_exception: self.compiler_exception(
                "IndexOutOfBoundsException",
                files,
                throwable.class(),
            )?,
            illegal_state_exception: illegal_state,
            illegal_state_message_constructor,
        })
    }

    pub(crate) fn validate_throwable(&mut self, files: &[ast::SourceFile]) {
        let Some(&candidate) = self.throwable_candidates.first() else {
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define a class `Throwable`".to_string(),
            );
            return;
        };
        self.throwable = Some(candidate);
    }

    /// The `Throwable` reference type of `scoop.core`, when validated.
    /// `throw` / catch lowering skips its subtype check when this is
    /// `None` (the misconfigured core was already diagnosed, so the
    /// module is rejected anyway).
    pub(crate) fn throwable_ty(&self) -> Option<TypeId> {
        self.throwable.map(|(_, ty)| ty)
    }
}
