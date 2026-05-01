# TODO（P0：并行主线脚手架与现状固化）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：建立可通过 `scoop` / `scoopc` 显式 CLI 参数激活的新 effect-refactor 并行主线；在不改变默认行为的前提下，完成 session/config bit、并行 dispatcher 壳层、共享/复制边界清单，以及一组锁定旧主线行为的 baseline parity 验证。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；实现过程中如果改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 是本阶段唯一计划基线；`docs/archive/plans/*` 只作历史参考，不回写旧 round。
- 本阶段只做 P0 对应的并行主线脚手架与现状固化，不提前实现 P1-P8 的业务语义。
- 本阶段绝对禁止在旧主线上打补丁式推进。
  - 允许新增并行入口；
  - 允许在 stage boundary 上由新路径整体委托到旧路径；
  - 不允许在旧业务模块内部掺入新旧线路分支逻辑。
- 对于新旧路线都需要用到的代码，只允许两种组织方式：
  1. 抽成独立模块，并提供**单一 API**同时供两边消费；该 API 中禁止包含“新旧线路标志”，模块自身也必须在**完全不了解自己是被哪条线调用**的前提下正常工作。
  2. 若上述条件无法满足，则必须将旧线路上的相关代码完整复制到新路线上来，确保两条线路逻辑上完全独立。
- 绝对禁止把两条线路的业务逻辑混在同一个实现函数/同一个业务模块里。
  - 明确禁止：`if new_pipeline { ... } else { ... }`、`PipelineMode` 开关、或等价标志出现在 HIR lowering、MIR lowering、effect analysis、late lowering、LLVM codegen 这类业务层实现函数中。
  - 允许出现线路分叉的地方，只能是最上层 dispatcher / session 路由层。
- 新主线在 P0-P6 期间必须通过 `scoop` / `scoopc` 的显式 CLI 参数激活；不能只靠内部测试开关、环境变量或临时代码路径进入。
- 本阶段不做 full regression。
  - 只做任务内要求的定向验证；
  - 不执行 `cargo test --all`；
  - 不执行 `cargo run -p scoop -- test` 的全量 fixture 扫描。
- 所有验证若需要触发新主线，必须统一通过本阶段建立的 CLI 参数进入，而不是通过替换默认值或测试专用入口进入。
- 每个任务完成后，必须把完成记录回写到当前 TODO 文件对应条目下的“完成记录”位置，供下一任务与 review 任务引用。

## P0-T01：建立新旧主线共享的 CLI / Session pipeline selector

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P0
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 顶部“管线原则”“闭包原则”，以及 §4.10, §4.11
- 目标：
  - 在 `scoop` 与 `scoopc` 上建立同一个可显式选择 `legacy` / `refactor` 的 pipeline config bit；
  - 该选择必须进入 `scoopc::session`，成为后续阶段统一可见的 session 配置，而不是散落在 CLI 层的临时布尔值。
- 必须实现的内容：
  1. 在 `crates/scoopc/src/session/` 下新增一个显式的 pipeline mode 定义。
     - 推荐新增：`EffectPipelineMode { Legacy, Refactor }`
     - 推荐新增：`SessionOptions` 或等价 builder/config 结构，把 pipeline mode 收口到 session 构造参数中。
     - 要求：默认值必须保持 `Legacy`，以确保仓库默认行为不变。
  2. 调整 `scoopc::session::Session` 的构造 API。
     - 保留一个 legacy 默认构造入口（例如继续保留 `Session::new()`）；
     - 同时新增一个显式传入 `SessionOptions` 的构造入口（名称可定，但必须稳定且测试可直接调用）。
     - 约束：不能在 `Session` 的后续业务 API 上再单独散发 pipeline mode 参数；pipeline mode 必须在 session 构造阶段收口。
  3. 在 `crates/scoop/src/cli.rs` 上为 `scoop` 增加全局 pipeline 参数。
     - 该参数必须能作用于所有会创建 `Session` 的命令：至少包括 `dump-ast`、`dump-hir`、`dump-mir`、`dump-ir`、`build`、`run`、`test`。
     - 推荐形态：`--effect-pipeline <legacy|refactor>`。
     - 若采用其它名字，必须满足：语义清晰、可双向选择、且后续 P7/P8 能继续沿用。
  4. 在 `crates/scoopc/src/bin/scoopc.rs` 上为 `scoopc` 增加同一语义的参数。
     - 参数取值、默认值、错误处理与 `scoop` 保持一致；
     - 两端必须最终归一到同一个 `EffectPipelineMode` / `SessionOptions`。
  5. 让 `scoop` 与 `scoopc` 的所有 `Session::new()` 调用点改为：
     - 默认沿用 legacy；或
     - 显式通过新参数构造 session。
     - 本任务完成后，任何需要进入新主线的命令，都必须能够只靠 CLI 参数改变 session mode，而不修改其它代码。
- 必须遵从的约束：
  - 不允许先用环境变量、线程局部、全局静态变量把 pipeline mode 偷渡进 session。
  - 不允许在 `scoop` 与 `scoopc` 各自发明两套不兼容的 flag / parser 语义。
  - 不允许在业务层通过再次解析命令行或直接读环境变量补 pipeline mode。
- 验证：
  1. 新增/更新 `crates/scoop/src/cli.rs` 的 CLI parse 单元测试，至少覆盖：
     - `--effect-pipeline legacy`
     - `--effect-pipeline refactor`
     - 缺省时为 legacy
     - 非法取值报错
  2. 为 `crates/scoopc/src/bin/scoopc.rs` 新增参数解析测试或可测试的 parse helper，并覆盖与 `scoop` 同样的合法/非法组合。
  3. 为 `crates/scoopc/src/session/mod.rs` 新增 session 构造测试，至少覆盖：
     - 默认 `Session::new()` 为 legacy
     - 显式 `SessionOptions` 可构造成 refactor mode
  4. 运行定向验证命令：
     - `cargo test -p scoop --no-default-features cli`
     - `cargo test -p scoopc --no-default-features session`
     - 若为 `scoopc` CLI 单独抽出 parse helper/module，则加对应 `cargo test -p scoopc --no-default-features <cli_parse_test_name>`
- 完成条件：
  - `scoop` 与 `scoopc` 都可以通过同义 CLI 参数把 pipeline mode 传入 `Session`；
  - 默认行为保持 legacy；
  - 后续任务不需要再为“怎么进入新主线”补充新机制。
- 依赖：无
- 完成记录：
  - 2026-05-02：完成 `scoop` / `scoopc` 共享的 effect pipeline selector 接入。
  - 新增 `scoopc::session::EffectPipelineMode` 与 `SessionOptions`，保留 `Session::new()` 默认走 `Legacy`，并新增 `Session::with_options(...)` 显式构造入口。
  - `scoop` 新增全局 `--effect-pipeline <legacy|refactor>`；`scoopc` 通过新抽出的 `crates/scoopc/src/driver_cli.rs` 复用同一 mode 语义与默认值，并由二进制入口统一落到 `SessionOptions`。
  - `dump-ast`、`dump-hir`、`dump-mir`、`dump-ir`、`build`、`run`、`test` 以及当前其它会创建 `Session` 的 driver 命令均已切到统一的 `SessionOptions` 构造路径，后续进入新主线不再需要额外机制。
  - 新增/更新测试：`crates/scoop/src/cli.rs`、`crates/scoopc/src/session/mod.rs`、`crates/scoopc/src/driver_cli.rs`。
  - 验证通过：`cargo test -p scoop --no-default-features cli`、`cargo test -p scoopc --no-default-features session`、`cargo test -p scoopc --no-default-features driver_cli`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P0-T01R：Review CLI / Session selector，确认新主线入口对两端一致且默认行为稳定

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P0
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 顶部“管线原则”
- 重点：
  - `scoop` 与 `scoopc` 是否已经共享同一 pipeline mode 语义，而不是两个名字相似但实现分叉的入口；
  - `SessionOptions` / pipeline mode 是否已经是后续阶段唯一读取入口；
  - 默认行为是否仍然是 legacy；
  - 是否还残留通过环境变量、全局静态或临时参数偷偷切主线的旁路。
- 必须检查的文件/位置：
  - `crates/scoop/src/cli.rs`
  - `crates/scoop/src/commands/**/*.rs`
  - `crates/scoopc/src/bin/scoopc.rs`
  - `crates/scoopc/src/session/mod.rs`
  - 新增的 session options / pipeline mode 定义位置
- 验证：
  - 重新运行 P0-T01 的所有测试与命令；
  - 额外执行最小 smoke：
    - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`
    - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop`
  - 本 review 不要求 `scoopc --emit-llvm` 端到端 smoke；LLVM/toolchain 相关验证留到后续确实进入 LLVM 阶段的任务再覆盖。
- 完成条件：
  - review 结论能够明确写出：两端共用同一 session bit，默认不变，新路径必须经 CLI 进入；
  - 可进入 P0-T02。
- 依赖：P0-T01
- 完成记录：
  - 2026-05-02：完成 `P0-T01` review，确认 `scoop` 与 `scoopc` 共享同一 `scoopc::session::EffectPipelineMode` / `SessionOptions` 语义，且缺省值仍为 `Legacy`。
  - `scoop` 侧在 `crates/scoop/src/cli.rs` 解析全局 `--effect-pipeline <legacy|refactor>`，并在 `crates/scoop/src/commands/mod.rs` 统一收口为 `SessionOptions::new(effect_pipeline)` 后分发给 `dump-ast`、`dump-hir`、`dump-mir`、`dump-ir`、`build`、`run`、`test` 等会创建 `Session` 的命令路径。
  - `scoopc` 侧在 `crates/scoopc/src/driver_cli.rs` 复用同一 `EffectPipelineMode` parser，并在 `crates/scoopc/src/bin/scoopc.rs` 统一通过 `Session::with_options(cli.session_options)` 落到 session；未发现第二套不兼容 flag 语义。
  - 代码搜索确认 pipeline selector 未经环境变量、全局静态或线程局部旁路进入 session；`EffectPipelineMode` / `effect_pipeline` 命中集中在 CLI、session、driver glue 与测试辅助中，未渗入 parser 或其它低层业务模块。
  - smoke 验证通过且 `legacy` / `refactor` 的 `dump-ast tests/fixtures/parse/hello.scoop` 输出一致。
  - 复验通过：`cargo test -p scoop --no-default-features cli`、`cargo test -p scoopc --no-default-features session`、`cargo test -p scoopc --no-default-features driver_cli`、`cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P0-T02：建立并行 pipeline dispatcher 壳层，禁止新路径直接侵入旧业务模块

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P0
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §8
- 目标：
  - 建立一套明确的并行 pipeline 壳层，让新路径能从总入口一路走到 AST/HIR/MIR/late lowering/LLVM 各阶段的独立 dispatcher；
  - 在 P0 阶段，这些 dispatcher 可以整体委托到旧主线，但委托必须发生在**阶段边界**，不能散落到低层业务实现中。
- 必须实现的内容：
  1. 在 `scoopc` 中新增一个专门承载“新旧主线分流”的顶层模块。
     - 推荐新模块：`crates/scoopc/src/effect_refactor_pipeline/` 或功能等价的位置；
     - 模块内至少包含：`legacy` 与 `refactor` 两侧的 stage entry，或功能等价的 dispatcher 结构。
  2. 明确划出 P0 阶段的 stage boundary API。
     - 必须至少预留以下概念边界：
       - AST / parse
       - HIR / typecheck
       - direct-style MIR
       - effect facts
       - late lowering
       - LLVM codegen
     - P0 不要求这些 stage 都真正有独立实现，但 dispatcher API 必须已经存在。
  3. 让 `scoop` 的相关命令（`dump-ast` / `dump-hir` / `dump-mir` / `dump-ir` / `build` / `run` / `test` 中实际会触发编译的路径）先经过新的 dispatcher，再委托到当前 legacy 实现。
  4. 让 `scoopc` 的 `--emit-llvm` / `--emit-obj` 路径同样先经过新的 dispatcher / session route，而不是绕过新路径直接落回旧实现。
  5. 明确禁止在低层业务函数中出现 pipeline mode 分支。
     - 如确有复用需求，只能通过“抽中立模块”或“复制旧代码到新路径”满足，不能在同一业务函数里写双分支。

- 必须遵从的约束：
  - dispatcher 允许在 P0 整体委托 legacy stage，但不允许在具体 HIR lowering / MIR lowering / LLVM codegen 函数体里靠 pipeline flag 分支。
  - 新路径当前即使完全委托 legacy，也必须从自己的 dispatcher 入口走完整条链，不能在命令层直接偷跳回旧函数。
  - 若某个旧模块无法中立共享，则先复制一份到新路径命名空间，再由 refactor dispatcher 调用复制体；不要在原模块里混写新逻辑。

- 验证：
  1. 新增/更新定向单元测试，证明：
     - 新 CLI 参数在命令层能够路由到 refactor dispatcher；
     - legacy 与 refactor dispatcher 都可独立构造和调用。
  2. 在不改默认行为的前提下，至少验证以下命令在 `--effect-pipeline refactor` 下可成功到达 dispatcher 并产出结果：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`
  3. 对以上命令分别跑 `legacy` 与 `refactor` 两种 mode，比对：
     - 退出状态相同；
     - 若输出是稳定文本（dump 命令），输出内容一致；
     - 若输出中含有显式“当前主线”调试标记，则禁止混入用户可见输出。

- 完成条件：
  - 仓库中已经存在一套明确的新路径 dispatcher 壳层；
  - 所有后续 P1-P6 实现都可以只改 refactor dispatcher 及其下属模块，不需要再侵入旧业务主线；
  - 默认主线行为不变。
- 依赖：P0-T01R
- 完成记录：
  - 2026-05-02：完成并行 pipeline dispatcher 壳层接入，新增 `crates/scoopc/src/effect_refactor_pipeline/` 顶层模块，并在其中固定 `legacy` / `refactor` 两侧 stage entry 与 `AST` / `typed HIR` / `direct-style MIR` / `effect facts` / `late lowering` / `LLVM codegen` 六个阶段边界。
  - `refactor` 路径当前仍按 P0 目标在阶段边界整体委托到 legacy 闭包；pipeline mode 的读取点仍只停留在 CLI / session / dispatcher 层，未把分支渗入 `hir/`、`mir/`、`effect/`、`llvm/` 等低层业务实现。
  - `scoop` 侧 `dump-ast`、`dump-hir`、`dump-mir`、`dump-ir` 已统一改走 `scoopc::effect_refactor_pipeline` wrapper；`build` 路径中的 parse 与 production LLVM 发射也改为先经过 dispatcher，再委托当前 legacy frontend/codegen。
  - `scoop test` 相关 fixture runner 中直连的 parse/HIR/MIR 路径已统一收口到 AST / HIR / MIR stage wrapper；`run` 与 `test` 继续通过 `build` 路径继承同一 dispatcher 壳层。
  - `scoopc` 二进制入口 `--emit-llvm` / `--emit-obj` 已改为通过 `effect_refactor_pipeline` 的 LLVM stage wrapper 进入；未启用 LLVM feature 时会给出显式错误，而不是绕过新路径。
  - 新增/更新测试：`crates/scoopc/src/effect_refactor_pipeline/mod.rs`（legacy/refactor dispatcher 构造与调用）、`crates/scoop/src/commands/dump_ast.rs`（命令层 refactor 路由）。
  - 输出比对结果：`dump-ast` / `dump-hir` / `dump-mir` 在 `legacy` 与 `refactor` 下输出一致；`dump-ir` 当前 `MaterializedMir` Debug 文本跨进程本身不稳定（`legacy` 对 `legacy` 重跑同样漂移），因此本任务仅核对其 legacy/refactor 退出状态一致且都能成功产出结果。
  - 验证通过：`cargo test -p scoop --no-default-features cli`、`cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`、`cargo test -p scoopc --no-default-features session`、`cargo test -p scoopc --no-default-features driver_cli`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/handle_perform.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P0-T02R：Review 并行 dispatcher 壳层，确认没有把新旧业务逻辑混写在一起

- 参考：
  - [`PLAN.md`](./PLAN.md) §0（尤其是“共享模块 vs 复制实现”约束）
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11
- 重点：
  - pipeline mode 的读取点是否只停留在 CLI / session / dispatcher 层；
  - 是否有业务模块已经出现 `if refactor { ... } else { ... }` 这类双分支混写；
  - refactor dispatcher 是否真的是“独立入口 + 整体委托 legacy stage”，而不是在业务代码内部零散插桩。
- 必须检查的目录：
  - `crates/scoopc/src/effect_refactor_pipeline/` 或等价新模块
  - `crates/scoopc/src/hir/`
  - `crates/scoopc/src/mir/`
  - `crates/scoopc/src/effect/`
  - `crates/scoopc/src/llvm/`
- 验证：
  - 重新运行 P0-T02 的所有定向命令；
  - 额外执行一次仓库搜索，确认 pipeline mode / selector 没有渗入低层业务目录。
    - 允许命中：CLI、session、dispatcher、测试辅助；
    - 不允许命中：HIR lowering、MIR lowering、effect analysis、late lowering、LLVM codegen 具体业务实现。
  - 可接受的搜索命令（任选其一，需在完成记录中附输出摘要）：
    - `rg "EffectPipelineMode|effect_pipeline|refactor pipeline|legacy pipeline" crates/scoopc/src`
    - 或等价内容搜索

- 完成条件：
  - review 能明确证明：当前新旧主线只在最上层 dispatcher 分流，没有在业务实现层混线；
  - 可进入 P0-T03。
- 依赖：P0-T02
- 完成记录：
  - 2026-05-02：完成 `P0-T02R` review，确认当前新旧主线仍只在 CLI / session / dispatcher 层分流，未在 `hir/`、`mir/`、`effect/`、`llvm/` 业务实现层读取 pipeline selector。
  - 代码抽查结论：`crates/scoop/src/commands/mod.rs` 仅把 CLI selector 收口为统一 `SessionOptions`；`dump-ast` / `dump-hir` / `dump-mir` / `dump-ir` 通过 `scoopc::effect_refactor_pipeline` wrapper 进入阶段边界；`run` 与 `test` 继续经由 `build` 继承同一路由；`crates/scoopc/src/bin/scoopc.rs` 的 `--emit-llvm` / `--emit-obj` 也统一经 `effect_refactor_pipeline::emit_single_file_llvm_artifact_to_file(...)` 进入 LLVM stage wrapper。
  - dispatcher 抽查结论：`crates/scoopc/src/effect_refactor_pipeline/mod.rs` 仍是唯一根据 `EffectPipelineMode` 选择 `legacy` / `refactor` stage entry 的位置；`legacy.rs` 与 `refactor.rs` 只在阶段边界整体委托 legacy 闭包，没有把 selector 下沉到 HIR lowering、MIR lowering、effect analysis 或 LLVM codegen 具体实现中。
  - 搜索摘要：对 `crates/scoopc/src/hir`、`crates/scoopc/src/mir`、`crates/scoopc/src/effect`、`crates/scoopc/src/llvm` 执行 `EffectPipelineMode|effect_pipeline|effect_pipeline_mode` 搜索均为 0 命中；selector 相关命中仍集中在 `session/`、`driver_cli.rs`、`effect_refactor_pipeline/`、driver 命令层与 fixture wrapper。
  - 复验通过：`cargo test -p scoop --no-default-features cli`、`cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`、`cargo test -p scoopc --no-default-features session`、`cargo test -p scoopc --no-default-features driver_cli`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/handle_perform.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P0-T03：建立“共享模块 vs 复制实现”边界清单，并把它固化为仓库文档

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P0
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 顶部“闭包原则”，以及 §4.11, §4.13.1a, §5.4
- 目标：
  - 把后续 P1-P6 会触碰的主要子系统，明确划分为：
    - 可中立共享
    - 必须复制到新路径
    - 当前不在 P0 结论范围内、需在后续任务时单独判断
  - 避免后续 agent 在实现中临时决定“这里先混一下，后面再整理”。

- 必须实现的内容：
  1. 新建一份仓库文档，推荐文件名：`EFFECT_REFACTOR_BOUNDARY_INVENTORY.md`。
  2. 文档至少要覆盖以下子系统，并逐项标明“共享 / 复制 / 后续再判定”的决定：
     - `crates/scoop/src/cli.rs` / driver command frontends
     - `crates/scoopc/src/session/`
     - `crates/scoopc/src/parser/`
     - `crates/scoopc/src/source.rs`, `span.rs`, `sysroot/`, `target/`, `ty/`
     - `crates/scoopc/src/hir/`
     - `crates/scoopc/src/mir/`
     - `crates/scoopc/src/effect/`
     - `crates/scoopc/src/llvm/`
     - 与 effect/continuation 直接相关的 runtime ABI helper 模块
  3. 对每个“共享”的条目，必须明确写出：
     - 单一 API 是什么；
     - 为什么它可以在完全不知道自己被哪条线调用的前提下正常工作；
     - 禁止哪些线路特化参数泄漏进 API。
  4. 对每个“复制”的条目，必须明确写出：
     - 复制体将从哪个阶段开始真正分叉；
     - 为什么当前旧实现无法通过“中立 API”共享；
     - 后续任务应该从哪个新模块入口继续推进。
  5. 若某个条目当前无法定性，必须把“不确定的原因”写清楚，并限定为“后续阶段进入该模块前必须先决策”，不能含糊写成“以后再看”。

- 必须遵从的约束：
  - 这份清单不是泛泛说明，而是后续 P1-P6 的执行约束；因此必须具体到模块/目录，而不是只写抽象名词。
  - 不能把“现阶段先共享，后面再视情况混写”写成允许路线。
  - 不能把明显带 effect/continuation 业务语义的模块标成共享，却不给出真正中立的单一 API。

- 验证：
  1. 文档存在并已纳入仓库根目录；
  2. 其覆盖面至少包含上述所有子系统；
  3. 用一次代码搜索验证当前仓库中 pipeline mode / legacy/refactor 分叉没有渗入被标记为“共享中立模块”的实现代码。
  4. 运行：
     - `cargo test -p scoop --no-default-features cli`
     - `cargo test -p scoopc --no-default-features session`
     - 确保新增文档/注释不会影响现有测试。

- 完成条件：
  - 后续阶段若要进入某个模块，已经能从这份边界清单直接知道“共享还是复制”；
  - P1-P6 不需要再为共享/复制原则重新开讨论。
- 依赖：P0-T02R
- 完成记录：
  - （执行时填写）

## P0-T03R：Review 边界清单，确认后续实现不会再靠临时判断混线

- 参考：
  - [`PLAN.md`](./PLAN.md) §0
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.11, §4.13.1a
- 重点：
  - 是否所有后续会碰到的关键目录都已被清单覆盖；
  - 标为“共享”的模块是否真的有中立单一 API 依据；
  - 标为“复制”的模块是否给出了明确的分叉入口和理由；
  - 是否还残留“以后实现时临时判断”的空洞条目。
- 验证：
  - 重新复读 `EFFECT_REFACTOR_BOUNDARY_INVENTORY.md`；
  - 重新执行 P0-T03 的搜索检查；
  - 抽查至少 3 个关键模块分类是否可信：
    - 一个底层中立模块（如 `source` / `span` / `parser`）
    - 一个中层 effect 业务模块（如 `mir/` 或 `effect/`）
    - 一个后端业务模块（如 `llvm/`）

- 完成条件：
  - review 能明确说明：后续任务不需要再为“这里能不能共享”做临时架构判断；
  - 可进入 P0-T04。
- 依赖：P0-T03
- 完成记录：
  - （执行时填写）

## P0-T04：建立 P0 baseline parity 验证矩阵，锁定“新路径壳层不改变旧语义”

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P0
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.12, §4.15
- 目标：
  - 在新路径尚未真正替换任何业务语义之前，建立一组可重复执行的 baseline parity 验证；
  - 证明在 P0 结束时：`--effect-pipeline refactor` 虽已走新壳层，但对当前样本输入仍与 legacy 产出一致。

- 必须实现的内容：
  1. 新增一组自动化 parity 测试或等价的稳定测试入口。
     - 形式可为 Rust 测试、fixture harness、或 driver 级 snapshot tests；
     - 但必须是**自动执行**的，不接受只把命令写在文档里。
  2. baseline 样本至少覆盖以下 4 类入口：
     - AST：`tests/fixtures/parse/handle_expr_minimal.scoop`
     - HIR：`tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
     - MIR：`tests/fixtures/mir/handle_perform.scoop`
     - IR/单态化视图：`tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`
  3. 对每个样本，都必须在 `legacy` 与 `refactor` 两种 CLI mode 下执行，并比较：
     - 退出状态
     - 标准输出 / dump 内容
     - 若有标准错误中的稳定文本，也应一并比较
  4. 如果当前构建启用了 LLVM feature，再额外加入一条 LLVM smoke parity：
     - 推荐样本：`tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`
     - 命令形态：`build --emit-llvm` 或 `scoopc --emit-llvm`
     - 对比要求：至少命令成功与模块头/关键稳定片段一致；若能做到完整 IR 相等，则以完整相等为准。

- 必须遵从的约束：
  - baseline parity 的目标是锁定“P0 新壳层不改语义”，不是提前验证 P1-P6 设计。
  - 不允许把 parity 验证降级成“新路径能跑就算通过”。
  - 不允许用修改默认主线的方式让测试通过。
  - 若某个输出包含明显不稳定字段（绝对路径、时间戳等），必须在测试里做正规化处理，而不是直接放弃该样本。

- 验证：
  1. 运行新增的 parity 测试入口；
  2. 额外执行以下人工 smoke 命令，确认 CLI 层与自动化验证一致：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/handle_expr_minimal.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ir tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`
  3. 不执行 full regression。

- 完成条件：
  - 仓库中已经存在一组自动化 baseline parity 验证；
  - P0 结束时，新壳层在选定样本上的可观察行为与 legacy 一致；
  - 后续 P1-P6 的每一步都可以先跑这组 baseline，快速确认“没把 P0 壳层搞坏”。
- 依赖：P0-T03R
- 完成记录：
  - （执行时填写）

## P0-T04R：Review baseline parity 与 P0 退出条件

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P0，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.15
- 重点：
  - parity 矩阵是否真的覆盖了 AST / HIR / MIR / IR 至少一条代表性路径；
  - 新路径是否只经 CLI 参数激活，而不是悄悄更改默认值；
  - baseline 是否足以在后续阶段快速判定“问题来自新实现，还是 P0 壳层本身”。
- 必须检查的产物：
  - 新增的自动化 parity 测试代码 / harness
  - P0-T03 产出的边界清单文档
  - `scoop` / `scoopc` CLI 参数与 dispatcher 路径
- 验证：
  - 重新运行 P0-T04 的全部自动化 parity 测试；
  - 再跑一次 `cargo test -p scoop --no-default-features cli` 与 `cargo test -p scoopc --no-default-features session`，确认新增测试与文档整理未破坏当前阶段的定向验证入口；
  - 不执行 full regression。

- 完成条件：
  - review 能明确确认：P0 产物已经足以支撑 P1-P6 只在新路径推进；
  - 旧主线默认行为稳定；
  - 新路径可以通过 CLI 参数稳定进入；
  - baseline parity 已足以作为后续各阶段的“壳层未漂移”守门测试。
- 依赖：P0-T04
- 完成记录：
  - （执行时填写）
