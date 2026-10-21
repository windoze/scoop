//! Ordinary generated bodies for the current Cone's checked test directory.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{FnSig, Lowerer};

mod bindings;
mod calls;
mod cases;
mod selection;

#[derive(Clone)]
pub(crate) struct TestCompilation {
    pub(crate) entry: Option<hir::FunctionId>,
    bindings: Vec<hir::DirectNamedPublicBindingGroup>,
}

impl TestCompilation {
    pub(crate) fn new(world: &hir::ImportedSemanticWorld<'_>) -> Self {
        let package = scoop_identity::PackagePath::from_segments(
            ["scoop", "test", "execution"]
                .map(|name| scoop_identity::CanonicalIdentifier::new(name).unwrap())
                .to_vec(),
        );
        Self {
            entry: None,
            bindings: world
                .direct_package(&package)
                .map_or_else(Vec::new, |package| {
                    package.snapshot_filtered(|provider| provider == bindings::test_cone())
                }),
        }
    }
}

pub(crate) fn output_kind(
    module: &hir::ExportHir,
) -> Result<hir::ConeOutputKind, Vec<ast::Diagnostic>> {
    let function = module
        .function_identities
        .iter()
        .find_map(|(function, identity)| {
            matches!(identity, hir::HirFunctionIdentity::TestEntry(_)).then_some(function)
        })
        .expect("successful test lowering creates its entry");
    hir::LocalExecutableEntry::try_new(module, function)
        .map(|entry| hir::ConeOutputKind::Executable {
            local_entry: Box::new(entry),
        })
        .map_err(|error| {
            vec![ast::Diagnostic::at(
                ast::Span::new(0, 0),
                format!("invalid generated test entry: {error}"),
            )]
        })
}

impl Lowerer {
    pub(crate) fn generate_test_entry(&mut self) {
        let file = self
            .intrinsic_sources
            .iter()
            .position(|source| source.identity.cone() == self.current_cone())
            .expect("current lowering has a nonempty source set");
        self.current_file = file;
        let span = ast::Span::new(0, 0);
        let int = self.integer_type(hir::IntegerKind::SIGNED_32);
        let arguments_ty = self.array_type(crate::types::ArrayKind::Immutable, self.string);
        let entry = self.functions.alloc(hir::Function {
            signature: hir::CallableSignature {
                context_parameters: Vec::new(),
                release_callability: Default::default(),
                name: "$test_entry".into(),
                is_suspend: false,
                modifiers: Default::default(),
                params: Vec::new(),
                return_ty: int,
                attributes: Default::default(),
                span,
            },
            access: self.local_declaration_access(),
            genericity: hir::FunctionGenericity::Plain,
            kind: hir::FunctionKind::User(hir::Body {
                locals: la_arena::Arena::new(),
                statements: Vec::new(),
            }),
            method: None,
        });
        self.test_compilation.as_mut().unwrap().entry = Some(entry);
        self.record_function_file(entry, file);
        self.signatures.insert(
            entry,
            FnSig {
                result_pending: false,
                parameter_pending: false,
                context_parameters: Vec::new(),
                is_suspend: false,
                modifiers: Default::default(),
                attributes: Default::default(),
                owner_type_param_count: 0,
                type_params: Vec::new(),
                params: vec![crate::FnParam {
                    name: ast::Ident {
                        text: "$test_arguments".into(),
                        span,
                    },
                    calling: crate::FnParamCalling::Required,
                    ty: arguments_ty,
                }],
                return_ty: int,
            },
        );
        self.top_level.push(entry);
        let body = self.lower_synthesized_body(entry, |state| {
            let arguments = hir::Expr {
                kind: hir::ExprKind::Local(state.functions[entry].params[0].local),
                ty: arguments_ty,
                span,
                origin: state.expression_origin(span),
            };
            let mut statements = Vec::new();
            let status = state.alloc_synthetic_local(
                "$test_status".into(),
                int,
                true,
                scoop_identity::SyntheticLocalRole::Temporary,
            );
            statements.push(hir::Statement {
                span,
                kind: hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { local: status },
                    init: state.test_integer(0, span),
                },
            });
            let mut index = 0;
            for container in state.test_containers.clone() {
                state.current_file = container.origin.file as usize;
                state.set_source_context(hir::SourceContextSubject::Function(entry));
                for case in &container.cases {
                    let selected = state.test_case_selected(arguments.clone(), index, case, &mut statements);
                    index += 1;
                    let Some(selected) = selected else {
                        continue;
                    };
                    let mut case_body = Vec::new();
                    if let Some(result) =
                        state.generate_test_case(entry, index - 1, &container, case, &mut case_body)
                    {
                        let span = case.method.origin.span;
                        let Some(passed) = state.named_enum_variant(result.ty, "Passed") else {
                            state.error(span, "test execution requires runReportedTestCase to return TestResult with a Passed variant".into());
                            continue;
                        };
                        case_body.push(hir::Statement {
                            span,
                            kind: hir::StatementKind::When(hir::When::Subject {
                                subject: result,
                                arms: vec![hir::WhenArm {
                                    condition: hir::WhenCondition::Case(hir::Pattern::Variant {
                                        application: passed,
                                        fields: Vec::new(),
                                    }),
                                    guard: None,
                                    body: vec![hir::Statement {
                                        span,
                                        kind: hir::StatementKind::Expr(state.unit_expression(span)),
                                    }],
                                    span,
                                }],
                                fallback: hir::WhenFallback::Else(vec![hir::Statement {
                                    span,
                                    kind: hir::StatementKind::Assign {
                                        target: hir::AssignTarget::Local(status),
                                        value: state.test_integer(1, span),
                                    },
                                }]),
                            }),
                        });
                        statements.push(hir::Statement {
                            span,
                            kind: hir::StatementKind::If {
                                cond: selected,
                                then_body: case_body,
                                else_body: None,
                            },
                        });
                    }
                }
            }
            state.current_file = file;
            state.set_source_context(hir::SourceContextSubject::Function(entry));
            if index > 0 && state.diagnostics.is_empty()
                && let Some(completed) = state.test_api_call("completeTestRun", Vec::new(), span, &mut statements)
            {
                if !state.types_equal(completed.ty, state.unit) {
                    state.error(span, "test execution requires completeTestRun to return Unit".into());
                } else {
                    statements.push(hir::Statement { span, kind: hir::StatementKind::Expr(completed) });
                }
            }
            statements.push(hir::Statement {
                span,
                kind: hir::StatementKind::Return {
                    value: hir::Expr {
                        kind: hir::ExprKind::Local(status),
                        ty: int,
                        span,
                        origin: state.expression_origin(span),
                    },
                },
            });
            statements
        });
        self.functions[entry].kind = hir::FunctionKind::User(body);
    }

    fn test_integer(&mut self, value: u32, span: ast::Span) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(value)),
            ty: self.integer_type(hir::IntegerKind::SIGNED_32),
            span,
            origin: self.expression_origin(span),
        }
    }
}
