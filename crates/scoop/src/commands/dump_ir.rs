//! `scoop dump-ir` 子命令。
//!
//! 当前阶段：输出“monomorphic MIR instances”的 Debug 视图，用于验证：
//! - `InstanceKey` 是否独立于最终 backend 符号名；
//! - generic MIR template 是否在 MIR 层 materialize 成稳定实例；
//! - direct-call fixed-point / nested closure family 重写是否成立。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

fn render_codegen_route_preflight(materialized: &scoopc::mir::MaterializedMir) -> String {
    let pass_view = materialized.pass_view();
    let facts = pass_view.instances().map(|family| {
        let abi = scoopc::mir::MirCodegenAbiPublication {
            callable_abi_kind: scoopc::mir::MirCallableAbiKind::DeferredToEffectFacts,
            resolved_outward_cases: Vec::new(),
            impl_plan: scoopc::mir::MirCallableImplPlan::DeferredToEffectFacts,
            adapter_required: false,
            step_schema_published: false,
        };
        if let Some(fun) = family.root_body() {
            scoopc::mir::MirCodegenRoutingFact::from_materialized_fun(fun, abi)
        } else {
            scoopc::mir::MirCodegenRoutingFact::declaration_only(
                family.root_fqn().to_string(),
                family.key().template.decl_span,
                abi,
            )
        }
    });
    scoopc::mir::MirCodegenRoutingFacts::new(facts).stable_dump()
}

fn load_input_source(input: PathBuf) -> Result<scoopc::source::SourceFile> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    scoopc::source::SourceFile::load(&input)
}

fn load_materialized_mir_for_dump(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> Result<scoopc::mir::MaterializedMir> {
    scoopc::pipeline::materialize_direct_style_mir_for_dump(session, source)
        .map_err(|err| miette::Report::from(*err))
}

/// 读取输入文件并打印实例化后的 MIR Debug 输出。
pub(super) fn render_dump_output(
    input: PathBuf,
    session_options: SessionOptions,
) -> Result<String> {
    let file = load_input_source(input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    let lowered = load_materialized_mir_for_dump(&session, &file)?;
    Ok(format!(
        "{:#?}\n{}",
        lowered,
        render_codegen_route_preflight(&lowered)
    ))
}

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    print!("{}", render_dump_output(input, session_options)?);
    Ok(())
}
