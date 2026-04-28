# 当前问题记录：MIR materialization 与 LLVM HIR 兼容边界

调研时间：2026-04-28

背景判断：production build 当前已经不是纯 HIR eager monomorphization。主路径大致是：

1. typecheck 收集 `MonomorphKey`。
2. MIR materializer 根据这些请求和 request-root 可达扫描生成 `InstanceKey` 集合、materialized MIR、summary/pass artifacts。
3. HIR compatibility lowering 再按 `InstanceKey` 恢复当前 LLVM 仍需要消费的 monomorphic HIR fun/member。
4. LLVM production emit 要求 `LoweredHir::materialized_pass_view()` 存在，但普通 body emission 仍主要走 HIR，只有 pass 显式 override 的 body 走 MIR bridge。

这个半切换边界已有不少保护，但实例根、可达性和 side table 传递仍存在几个不一致点。

## 已修复（2026-04-28）：build frontend 的实例根范围比 single-file frontend 宽，可能过度物化

修复记录：

- `scoop build` frontend 现显式区分 MIR request sources 与 support sources：
  - 单文件 build 只有用户入口源贡献 initial `MonomorphKey` 与 request-root 可达扫描；
  - cone build 暂按保守策略让当前 consumer cone 的全部 source 作为 request roots；
  - stdlib / sysroot support sources 仍完整参与 typecheck / lowering / fun index，但不再贡献 initial monomorph seeds。
- `lower_main_hir_for_build(...)` 已改用 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，不再通过 wrapper 把全部 `files_to_lower` 自动提升为 request roots。
- 新增回归覆盖：
  - `build_frontend_single_file_request_roots_exclude_stdlib_support_sources`
  - `build_frontend_cone_request_roots_exclude_stdlib_support_sources`

原问题记录：

`crates/scoopc/src/llvm/frontend.rs` 的 single-file frontend 只让入口文件调用：

- `check_file_exprs_with_monomorph_keys(...)`
- `request_source_paths = [entry_source.path()]`

support sources 只作为可被调用的实现体参与普通 typecheck/lowering，不贡献初始 monomorphization 请求根。

相关位置：

- `crates/scoopc/src/llvm/frontend.rs:160`
- `crates/scoopc/src/llvm/frontend.rs:202`

但 `scoop build` frontend 当前对 `front.input.sources` 中所有文件都调用 `check_file_exprs_with_monomorph_keys(...)`，包括：

- `stdlib/*.scoop`
- compilable sysroot sources
- cone 当前源集中所有文件

相关位置：

- `crates/scoop/src/commands/build.rs:709`

之后 `lower_main_hir_for_build` 调用的是不带 request-source 参数的 wrapper：

- `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_opt_level(...)`

这个 wrapper 会把 `files_to_lower` 全部转成 `request_source_paths`：

- `crates/scoopc/src/hir/lower/mod.rs:2682`

这和 `with_request_sources` 接口的设计目标冲突。该接口注释明确说：

- `files_to_lower` 决定哪些文件进入 HIR 兼容输出；
- `request_source_paths` 决定哪些源文件可以贡献 monomorphization 请求与 request-root 可达扫描；
- 这样 support sources 可以留在 lowering/fun_index 中，同时避免其内部未被入口触达的 generic 调用被提升为实例根。

相关位置：

- `crates/scoopc/src/hir/lower/mod.rs:2705`
- `crates/scoopc/src/hir/lower/mod.rs:2710`

影响：

- build 主路径可能把 support file 内部不可达的 generic 调用当作实例根；
- 这会增加 materialized MIR/HIR 兼容输出中的实例集合；
- 更严重时，support 文件中后端尚不支持的 generic body 可能被不必要地推到 LLVM 边界。

建议：

- 单文件 build 与 `crates/scoopc/src/llvm/frontend.rs` 对齐：只有用户入口文件贡献 `MonomorphKey`，stdlib/sysroot support sources 只普通 typecheck。
- cone build 需要明确 root 策略：
  - 保守方案：当前 consumer cone 全源文件都是 request roots，但不让 stdlib/sysroot support sources 贡献 initial `MonomorphKey`。
  - 更精确方案：从选定 `entry_main_fqn` 或 entry package 计算 request roots。
- `lower_main_hir_for_build` 应改用 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，显式传入 request roots。

## 已修复（2026-04-28）：`MonomorphKey` 没有 call-site source，后置过滤能力不足

修复记录：

- `MonomorphKey` 现在继续只表示被请求实例身份；新增 `MonomorphRequest` 记录：
  - `key`
  - `request_source_path`
  - `call_span`
- typecheck 新增 `check_file_exprs_with_monomorph_requests(...)`，在记录泛型调用请求时保留当前源文件和调用点 span。
- build frontend 与 single-file LLVM frontend 已改为传递 `MonomorphRequest`，不再把裸 `MonomorphKey` 作为 production MIR materialization 的初始请求输入。
- MIR materializer 的 `seed_requests(...)` 现在按 `request_source_paths` 过滤 request source；即使 support source 的 request 被上游收集到，也不会在非 request-root 模式下成为 initial seed。
- 新增回归 `materializer_filters_initial_monomorph_requests_by_call_site_source`，覆盖：
  - support source 中收集到的 request 在 main-only request roots 下被过滤；
  - 同一个 request 来自 request source 时仍正常物化实例。

原问题记录：

`MonomorphSymbol` 只记录被实例化声明的：

- `fqn`
- `decl_file`
- `decl_span`

`MonomorphKey` 只补充：

- `type_args`
- `eff_args`

相关位置：

- `crates/scoopc/src/monomorph/mod.rs:28`
- `crates/scoopc/src/monomorph/mod.rs:56`

所以一旦 build frontend 把某个 support source 的 `MonomorphKey` 放进 `front.monomorph_keys`，materializer 的 `seed_requests(...)` 无法判断这个请求来自哪个调用点/source，只能无条件作为初始实例根处理。

相关位置：

- `crates/scoopc/src/mir/materialize.rs:2537`

影响：

- request-source 过滤只能控制 `collect_request_root_fun_keys(...)` 和 HIR direct-call fallback 扫描；
- 对已经收集进来的 `MonomorphKey`，materializer 缺少来源信息，无法准确过滤非 request source 请求；
- 这使 P1 的 build frontend 过度收集更难在下游修正。

原建议：

- 短期：在 frontend 收集阶段按 source 分流，不让 support source 贡献 `MonomorphKey`。
- 中期：为 monomorph request 增加来源信息，例如：
  - `request_source_path`
  - `call_span`
  - 或新增 `MonomorphRequest { key, source_path, call_span }`，保留 `MonomorphKey` 作为实例身份。
- materializer 入口可接收带来源的请求，并在 `request_source_paths` 之外过滤 initial seeds。

## 已修复（2026-04-28）：request-root 当前是“源文件级”，不是 entry-main 可达级

修复记录：

- 新增 `MaterializeRequestRootMode`：
  - dump / 调试路径继续使用 source-file rooted 模式；
  - production build / single-file LLVM frontend 使用 entry-main rooted 模式。
- entry-main 模式下，materializer 的 request roots 只来自：
  - 精确选定的 entry `main`；
  - `Index` 中显式登记的 export entry points。
- production build 的单文件 package main 现在会计算显式 entry FQN，避免 materializer 与 LLVM entry 选择使用不同入口身份。
- entry-main 模式下，initial `MonomorphRequest` seed 还必须位于已扫描到的 entry 可达函数体内；同一 request source 中未从入口触达的 helper 不再直接贡献实例。
- HIR-only synthetic direct-call fallback 现在随实际可达 MIR function body 消费，而不是预先只扫描初始 source roots；这保留了 async task lowering 中 `__task_step_ready<T>` 等 HIR synthetic generic helper 的实例发现。
- MIR direct-call 实例推断现在同时使用赋值目标 local 的结果类型，补齐只从返回类型才能恢复 type 参数的 helper 调用。
- 修复过程中同时补上零参数顶层 direct-call 的 MIR lowering 缺口：
  - `entry()` 不再误落为 `Todo("dispatch receiver lowering pending")`；
  - 新增 `tests/fixtures/mir/direct_zero_arg_call.{scoop,mir}`。
- 新增 / 调整回归覆盖：
  - `build_frontend_entry_roots_skip_same_file_unreachable_generic_helper`
  - `build_frontend_entry_roots_skip_unreachable_cone_source_generic_helper`
  - 既有 effect-row 与 owner-specialized getter build 回归现在从 `main` 真实触达被测实例，不再依赖源文件级 root。

保留说明：

- 当前 initial request 过滤仍以“entry 可达函数体 span”为粒度；MIR block 级精确过滤另见后续 P2“request-root 可达扫描不使用 MIR CFG reachable-block 过滤”。

原问题记录：

`collect_request_root_fun_keys(...)` 会把 request source 中的所有顶层函数和 member fun 都作为 request-root fun。

相关位置：

- `crates/scoopc/src/mir/materialize.rs:539`

因此，只要某个函数定义在 request source 内，即使它并未从选定 `main` 真正可达，也会参与 request-root 可达扫描。

这解释了现有测试中常见的模式：

```scoop
fun entry(): Int {
    return wrap<Int>(1)
}

fun main(): Int / Pure! {
    val thunk: () -> Int = entry
    return 0
}
```

即使 `main` 没有调用 `entry()`，`entry` 内部的 generic 实例仍会被 materialize。当前这可能是有意的“源文件级 request root”策略，但它比 production executable 的真实入口可达图更粗。

影响：

- 单文件 build 或 cone entry file 中的未调用 helper 仍可能贡献实例；
- 如果这些 helper 只作为库 API 或实验代码存在，会扩大实例集合；
- 与 LLVM `collect_reachable_top_level_funs(hir_main, ...)` 的入口语义不完全一致。

建议：

- 明确当前语义是否是设计选择。
- 若目标是 executable 精确构建，应新增 entry-main rooted materialization mode：
  - 以 `entry_main_fqn` 为初始 root；
  - 通过 MIR/HIR call graph 扩展；
  - 保留显式 export/native entry points 作为额外 roots。

## 已修复（2026-04-28）：MIR materializer 的 request-root 可达扫描不使用 MIR CFG reachable-block 过滤

修复记录：

- 新增 `reachable_body_block_indices(...)`，materializer request-root 扫描现在与 LLVM reachability 使用同一口径：
  - `body.reachable_blocks()` 成功时只扫描可达 blocks；
  - CFG 验证失败时保守扫描全部 blocks。
- `scan_reachable_non_generic_fun(...)` 不再直接遍历 `body.blocks`，不可达 block 中的 generic direct-call / top-level ref 不再贡献额外实例。
- entry-main 模式下 initial `MonomorphRequest` seed 的 fallback 已从“可达函数 span”收口到“可达语句 span”。
- request-root caller-side pass candidate rewrite 也同步限制在 reachable blocks 内，避免不可达 block 在 rewrite 阶段绕过扫描过滤并 enqueue 泛型实例。
- 修复过程中暴露出既有 MIR CFG 边界问题：`TerminatorKind::Handle` 曾把 handler body / arms / finally 保形降低到独立 block，却没有把这些 block 暴露为 CFG successor，导致 `reachable_blocks()` 会把语义上可执行的 handle 内部 block 判为不可达；现已为 handle terminator 增加保守 successor targets，并更新 `handle_perform.mir` golden。
- 完整 fixture 验证继续暴露顶层 immutable `val` initializer 的可达性缺口：入口路径读取的顶层值会在运行时 lazy init，因此其 initializer 中的 generic call 也必须参与 request-root 实例过滤。materializer 现在在可达 MIR `TopLevelRef` 命中顶层 immutable value 时，会递归标记该 initializer span 及其引用的顶层值 initializer span。
- 新增回归 `request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks`：
  - 测试手动向 `main` MIR 追加结构不可达的 `id<Int>` direct-call；
  - 断言该 call 不会进入 initial requests、不会生成 `InstanceKey`，也不会物化 `id::<Int>` callable body。
- 既有 run-pass-cone 回归 `cross_file_generic_top_level_val_basic` 继续覆盖跨文件顶层 `val` initializer 中的 `id<Int>` 与被入口调用的 helper 中的 `id<String>` 都能被保留。

原问题记录：

materializer 在 `scan_reachable_non_generic_fun(...)` 中直接遍历 `body.blocks` 的所有 block：

- `crates/scoopc/src/mir/materialize.rs:2617`

LLVM reachability 在扫描 pass MIR body 时则先调用 `body.reachable_blocks()`，失败后才退回全块扫描：

- `crates/scoopc/src/llvm/reachability.rs:274`

影响：

- materializer 可能从 MIR 不可达 block 中发现 generic direct-call，并物化额外实例；
- LLVM 后续 reachability 未必按同一标准认为这些 call 可达；
- 这是另一个“实例集合”和“最终 codegen 可达集合”口径不一致点。

建议：

- 将 `scan_reachable_non_generic_fun(...)` 改成与 LLVM reachability 一致：
  - `body.reachable_blocks()` 成功时只扫描可达 blocks；
  - 失败时再保守扫描全部 blocks。
- 增加回归：不可达 MIR block 中存在 `id<Int>`，materializer 不应产生 `id::<Int>`，除非 CFG 验证失败触发保守回退。

## 已修复（2026-04-28）：LLVM production body emission 默认消费可支持的 materialized MIR body

修复记录：

- production body emission 现在通过 `canonical_materialized_callable_body(...)` 读取
  `MaterializedMirPassView` 中的 canonical callable body：
  - pass view 中存在的 materialized instance raw body 默认进入 `codegen_top_level_mir_fun(...)`；
  - 显式 pass-rewritten body 继续进入 `codegen_top_level_mir_fun(...)`；
  - pass view 明确移除 body 的 callable 不再由 HIR 兼容 body 重新发射。
- 对未被 pass override 的 raw materialized body，production emit 新增 bridge 支持性预检：
  - 当前 MIR bridge 已支持的纯 scalar / direct-call / 基础控制流 body 默认走 MIR；
  - effect/state-machine body、函数值 `TopLevelRef`、closure/fun-value/dynamic dispatch、
    tuple/member/capture/pattern/perform 等尚未支持的 MIR 节点继续走 HIR 兼容发射边界。
- 显式 pass override 不使用上述 HIR 兼容回退：
  - 如果 pass 发布了当前 MIR bridge 仍不支持的 body，production LLVM 会继续暴露结构化
    `UnsupportedMainBody`；
  - 这样 pass rewrite 不会被静默吞回旧 HIR body。
- 新增回归 `production_codegen_lowers_raw_materialized_mir_body_without_pass_override`，确认
  O0 下未被 pass override 的 `wrap::<Int>` raw materialized MIR body 会通过 MIR bridge 发射，
  并直接调用 materialized `id::<Int>`。

原问题记录：

production emit 已经要求 `LoweredHir::materialized_pass_view()` 存在，并把 pass view 传给
reachability、body presence 判断与 suspendability summary cache，但实际 body emission 只有
`pass_view.callable_body_is_overridden(fun.fqn)` 为真时才走 `codegen_top_level_mir_fun(...)`；
否则继续走 `codegen_top_level_fun(...)` 的 HIR codegen。这使 raw materialized MIR body 默认还
不是 LLVM body source of truth，只有 pass 显式发布 override 后才影响 production body emission。

保留边界：

- HIR compatibility lowering 仍需为当前 MIR bridge 尚未支持的 raw body 形状提供兼容发射边界；
- 后续仍应继续扩大 `codegen_top_level_mir_fun(...)` 支持面，最终减少 HIR compatibility body
  对 production correctness 的责任。

## 已有保护和回归覆盖

已有保护：

- `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)` 已支持 `files_to_lower` 与 `request_source_paths` 分离。
- materializer 能通过 request-root 扫描发现跨文件 helper 中实际触达的 generic/effect-generic 实例。
- HIR compatibility lowering 已经按 materializer 产出的 `InstanceKey` 生成 monomorphic fun/member，而不是 legacy eager HIR 自行决定实例集合。
- production LLVM 入口会拒绝没有 materialized pass view 的 legacy lowering。
- pass override body、pass summary override、caller-side non-generic pass rewrite，以及 bridge
  已支持形状的 raw materialized instance body 已经能影响 production LLVM。

本次验证过的测试：

```bash
cargo test -p scoop build_frontend_ -- --nocapture
cargo test -p scoopc mir::materialize -- --nocapture
cargo test -p scoopc production_codegen -- --nocapture
cargo test -p scoopc llvm::tests -- --nocapture
cargo test --all
cargo clippy --all-targets -- -D warnings
```

结果：

- `cargo test -p scoop build_frontend_ -- --nocapture`：8 passed。
- `cargo test -p scoopc mir::materialize -- --nocapture`：19 passed。
- `cargo test -p scoopc production_codegen -- --nocapture`：9 passed。
- `cargo test -p scoopc llvm::tests -- --nocapture`：62 passed。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 建议收口顺序

1. [DONE 2026-04-28] 修 build frontend 的 request-source 接线，先避免 stdlib/sysroot support sources 贡献 initial `MonomorphKey`。
2. [DONE 2026-04-28] 为 monomorph request 增加 call-site/source 来源，或在 frontend 侧保留带来源 wrapper。
3. [DONE 2026-04-28] 统一 materializer 与 LLVM reachability 的 MIR reachable-block 扫描口径。
4. [DONE 2026-04-28] 明确 production request-root 粒度为 entry-main reachable roots。
5. [DONE 2026-04-28] 让 production LLVM body emission 默认消费 bridge 已支持形状的 materialized MIR body，并保留明确 unsupported MIR boundary 的 HIR 兼容发射。
