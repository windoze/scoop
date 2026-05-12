# TODO（Stable ID 落地）

> 生成时间：2026-05-13  
> 设计基线：[`STABLE_ID.md`](./STABLE_ID.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 格式参考：`docs/archive/plans/TODO.md`、`docs/archive/plans/TODO-P0.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 当前状态：当前 `dump-*`、RTTI、LLVM/object/linker visible symbol、以及部分 JSON/cache surface 仍直接或间接泄漏 dense id、`source_path + decl_span`、raw `Debug` 文本与 pretty-printer 文本；本文件的任务不是重写语言语义，而是按 `STABLE_ID.md` 与 `PLAN.md` 的规则，把这些外部 surface 收口到统一 stable key / mangling / renderer / linkage 体系上，同时保持功能不漂移。

## 全局约束

- [`STABLE_ID.md`](./STABLE_ID.md) 是本轮唯一设计基线。若任务实现过程中需要改变 stable key、hash、mangling、linkage 或“什么算外部 surface”的主张，必须先更新该文档，再继续编码。
- [`PLAN.md`](./PLAN.md) 是本轮唯一计划基线。当前文件只负责把 `PLAN.md` 的 P0-P7 拆成严格顺序执行的任务；`docs/archive/plans/*` 仅作格式与历史参考，不回写旧 round。
- 本轮默认不得引入功能漂移。以下内容必须保持等价：
  - typecheck 结论
  - 程序运行结果
  - effect / continuation 语义
  - GC / runtime 行为
  - callable ABI 的语义合同
- 本轮允许发生变化的只有 identity surface：
  - `dump-*` 文本与相关 fixture expect
  - `dump-rtti` 文本与 RTTI id
  - `.cone` / cache / JSON 中的 identity 字段
  - LLVM IR / object / linker 可见符号名
  - compiler-private helper 的 linkage
- `TypeId`、`SourceId`、`ConeId`、`SymbolId`、`ClosureId`、`BasicBlockId`、`LocalId`、`SiteId`、`StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`ResumeInterfaceId`、`ContinuationObjectId`、`StateId`、`BoundaryId`、`FrameSlotId` 仍可继续作为内部实现 handle，但不得直接出现在外部协议中，见 `STABLE_ID.md` §5.1。
- 严禁继续把 raw `Debug` 当作最终协议。
  - CLI dump、fixture、JSON、RTTI 不能直接 `format!("{:#?}")`。
  - `Debug` impl 可以继续用于内部调试，但必须与对外 dump / fixture surface 脱钩。
- 严禁继续让以下来源承担唯一性责任：
  - `source_path + decl_span`
  - `TypeStore::display()` 文本
  - `sanitize_llvm_ident()` 输出
  - allocator 顺序或 pass-local dense id
- 导出 ABI symbol、private LLVM helper symbol、dump label、RTTI id 必须明确区分 identity 来源，不得再混用。
- `main`、`@Extern` 指定的 native symbol、宿主/平台固定入口属于显式例外；不得被统一 mangler 误改，见 `STABLE_ID.md` §5.1.10、§7.4。
- 以下 active schema 视为健康基线，只做防回归审计，不做无必要格式重写：
  - `crates/scoopc/src/cone/scoopir/schema.rs`
  - `crates/scoopc/src/cone/pre_specialize.rs`
  - `crates/scoopc/src/cone/visibility.rs`
  - `crates/scoopc/src/cone/annotations.rs`
- 每个任务完成后，必须在对应条目的“完成记录”下回写：
  - 改动范围
  - 核心决策
  - 验证结果
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合的目标或验收项

## P0：冻结基线与审计脚手架

### [DONE] P0-T01：建立 stable-id 外部 surface 审计脚手架

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P0
  - [`STABLE_ID.md`](./STABLE_ID.md) §1、§3、§10、§11、§12
- 目标：
  - 在正式改命名规则前，先建立“看得见 symbol / linkage / dense-id 泄漏”的审计入口。
  - 让后续各阶段不再依赖临时 grep 或手工读 IR 判断是否回归。
- 必须实现的内容：
  1. 在 `crates/scoopc/src/llvm/tests.rs` 增加 stable-id 专用 object / symbol 审计 helper。
     - 直接复用仓库现有 `object::File::parse(...)` 路径，参考 `crates/scoopc/src/llvm/tests.rs:2995`、`3033`、`3136`。
     - helper 至少要能：
       - 生成 object 文件
       - 读取 external symbol 集
       - 区分 runtime/native import、用户 ABI symbol、compiler-private helper
  2. 基于该 helper 增加一组 stable-id 审计测试骨架，样例至少覆盖：
     - source-level top-level function
     - materialized generic callable
     - closure body / closure resume / closure env
     - effect helper shell / continuation outcome helper
     - object init bridge / object init function / top-level init bridge
  3. 把 `STABLE_ID.md` §11 的 grep 审计点固化为实现期常驻清单。
     - 最少覆盖搜索域：`crates/scoop/src`、`crates/scoopc/src`、`tests/fixtures`
     - 当前重点 pattern 包括：
       - `TypeId\(`
       - `ClosureId\(`
       - `module\.add_function\(.*None\)`
       - `stable_template_symbol_suffix`
       - `source_path.*decl_span`
       - `scoop\.lambda\$[0-9]+`
       - `__schema[0-9]+`
       - `t[0-9]+__`
  4. 在测试注释或测试 helper 中写清“允许变化的 surface”和“禁止漂移的行为”。
     - 允许变化：symbol 文本、linkage、dump 文本、fixture expect、RTTI id、JSON identity 字段
     - 禁止漂移：语义、运行结果、typecheck、effect / continuation / GC 行为
- 必须遵从的约束：
  - 本任务不改 symbol 命名规则，不刷新 fixture。
  - 本任务建立的是审计地基，不是把当前错误命名固定成永久基线。
  - 审计测试必须围绕“identity 来源是否正确”建模，而不是继续锁死今天的旧名字拼写。
- 验证：
  1. 推荐新增定向测试命名：`stable_id_audit_*`、`external_symbol_*`。
  2. 运行：
     - `cargo test -p scoopc stable_id_audit -- --nocapture`
     - `cargo test -p scoopc external_symbol -- --nocapture`
  3. 对 `STABLE_ID.md` §11 的核心 grep 清单执行一次基线审计，并在完成记录中附命中摘要。
- 完成条件：
  - 后续任务已经拥有稳定可复用的 object / symbol / grep 审计入口。
- 依赖：无
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/effect_facts/builder.rs`
    - `crates/scoopc/src/effect_lowered/ir.rs`
    - `crates/scoopc/src/llvm/codegen/call/lowering.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - 核心决策：
    - 在 `llvm/tests.rs` 新增 stable-id object/symbol 审计 helper，基于 `object::File::parse(...)` 提取 external symbol 集，并按 `runtime/native import`、`fixed external exception`、`user ABI`、`compiler-private helper` 分类。
    - 新增 `stable_id_audit_*` / `external_symbol_*` 测试骨架：
      - `external_symbol_audit_top_level_and_materialized_generic_smoke` 覆盖 source-level top-level function 与 materialized generic callable。
      - `external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke` 覆盖 closure body/resume/env、effect helper shell / continuation outcome helper、object init bridge / object init function / top-level init bridge。
    - 把 `STABLE_ID.md` §11 的 grep 清单固化为测试常量与 repo 扫描 helper，固定扫描 `crates/scoop/src`、`crates/scoopc/src`、`tests/fixtures`。
    - 在测试常量中显式声明“允许变化的 surface”与“禁止漂移的行为”，避免后续任务把 symbol/linkage/dump 文本变化误判为语义回归。
    - 为满足本任务要求的 `clippy -D warnings` 验证，补齐了若干既有 `too_many_arguments` 精确 lint 豁免并修复一处 `needless_borrow`；未改动语义路径。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - grep 基线审计摘要（`stable_id_audit_grep_inventory_scans_repo_roots`）：
      - `TypeId\(` 2583 命中，`BasicBlockId\(` 41 命中，`module\.add_function\(.*None\)` 101 命中。
      - 当前重点 pattern 命中：`stable_template_symbol_suffix` 7、`source_path.*decl_span` 5、`scoop\.lambda\$[0-9]+` 2、`scoop\.lambda_resume\$[0-9]+` 1、`scoop\.lambda_env\$[0-9]+` 1、`__schema[0-9]+` 2、`__k[0-9]+` 4、`t[0-9]+__` 0。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P0 的 object-level 验证基线与 grep 审计入口要求，后续阶段已可复用统一 external symbol/object/grep 审计入口。
    - 对应 `STABLE_ID.md` §10/§11：已把 dense-id/path/span 泄漏的主要 grep 入口固定下来，并把 external symbol 分类与允许变化/禁止漂移边界落到常驻测试中。

### [DONE] P0-T02：固化现有测试基线，移除对旧命名字符串的强绑定

- 参考：
  - [`PLAN.md`](./PLAN.md) §2、§4 / P0
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.1、§3.4、§10
- 目标：
  - 把当前测试里“锁死旧 symbol 形状”的断言改成“锁定 visibility / linkage / 稳定性语义”的断言。
  - 同时明确健康 `.cone` / JSON surface 只是防回归基线，不是重写对象。
- 必须实现的内容：
  1. 复核并迁移 `crates/scoopc/src/llvm/tests.rs` 中对旧命名形状的强绑定断言。
     - 当前已知入口：`1436`、`2402`、`2434`、`3403`
     - 迁移方向：
       - 不再断言 `scoop.lambda$0`、`__scoop_object_init__...` 等旧具体字符串
       - 改为断言 symbol 是否 external / private、是否具备稳定 hash 主体、是否落在正确 namespace
  2. 复核与 `Step__schema*`、`lambda`、`object_init` 等相关的 IR 断言，避免继续把旧的 dense-id 命名形状当作正确性标准。
  3. 对以下健康 schema 增加“仍未泄漏 dense id / 绝对路径”的基线断言：
     - `crates/scoopc/src/cone/scoopir/schema.rs:16-155`
     - `crates/scoopc/src/cone/pre_specialize.rs:44-84`
     - `crates/scoopc/src/cone/visibility.rs:70-100`
     - `crates/scoopc/src/cone/annotations.rs:36-64`
  4. 在测试面明确区分两类验证：
     - 对 stable-id 来说应当变化的 textual surface
     - 对 stable-id 来说不得变化的行为语义
- 必须遵从的约束：
  - 不得在本任务中把旧名字“重新包装一下”继续当金标准。
  - 不得借基线整理顺手重写 `.cone` / JSON schema 结构。
  - 不得把“文本断言放松”当成掩盖真实行为漂移的手段。
- 验证：
  1. `cargo test -p scoopc`
  2. 针对 `crates/scoopc/src/llvm/tests.rs` 中已知旧命名断言的新增/更新测试全部通过。
  3. `.cone` / JSON 相关定向测试继续通过，且无新增格式 churn。
- 完成条件：
  - 测试面不再强绑定旧命名形状，后续可以安全推进 symbol / linkage 迁移。
- 依赖：P0-T01
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/cone/scoopir/schema.rs`
    - `crates/scoopc/src/cone/pre_specialize.rs`
    - `crates/scoopc/src/cone/visibility.rs`
    - `crates/scoopc/src/cone/annotations.rs`
  - 核心决策：
    - 在 `llvm/tests.rs` 新增 `stable_id` 语义 helper（含 `function_ir_matching`、hidden-init call 检查、hash-suffix 检查），把一组已知旧命名强绑定从“锁死旧字符串”迁移成“锁定 tuple payload 结构、surface-resume 路径、private helper 家族与 namespace 语义”的断言。
    - 对 `Step__schema*`、`surface_resume__k*`、`scoop.lambda$0`、`__scoop_object_init__...` 等已知旧名字入口做了定向迁移，并补了 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests`，防止行为测试回流到旧拼写金标准。
    - 对 `api.scoopir`、`PRE_SPECIALIZE.json`、`SYMBOL_VISIBILITY.json`、`ANNOTATION_CLASSES.json` 各自新增示例序列化基线测试，只验证“仍未泄漏 dense id / path / span 文本”，不重写 schema 结构。
    - 为 future stable-id 命名预留测试语义：external symbol 分类 helper 现在显式接受 `__scoop_abi0_*` 作为 user ABI、`__scoop_priv0_*` 作为 compiler-private helper，避免后续迁移时仍被旧分类假设卡住。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P0 的“允许变化 textual surface / 禁止漂移行为语义”边界收口：现有 `llvm` 行为测试不再把若干旧 dense-id/private helper 拼写当作正确性标准。
    - 对应 `PLAN.md` P0 与 `STABLE_ID.md` §3.1 / §10：四个健康 `.cone` / JSON schema 已有常驻基线测试，后续阶段可在不引入无谓 schema churn 的前提下继续推进 stable-id 迁移。
    - 对应 `STABLE_ID.md` §3.4：已把 `Step__schema*`、`surface resume kN`、`lambda$0`、`object_init` 等当前最容易把旧 identity 形状锁进测试的入口改成语义断言，后续 P1-P4 可安全调整 symbol/linkage。

### [DONE] P0-T02A：清理剩余 stable-id 敏感 LLVM 测试中的旧 private helper 名字绑定

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P0
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4、§8.5、§10
- 背景：
  - `P0-T02` 已迁移一批高风险旧命名断言，但 `P0-T02R` review 复核时发现 `crates/scoopc/src/llvm/tests.rs` 仍有若干 stable-id 敏感断言直接锁死旧 private helper spelling，例如：
    - `__scoop_refactor_resume__...`
    - `__scoop_refactor_direct_invoke__...`
    - `__scoop_refactor_surface_resume_owner_dispatch__...`
    - `__scoop_top_level_val_init__...`
  - 这些 symbol/descriptor 属于后续 P2/P3 会继续调整的 compiler-private naming surface；若不先清理，后续任务会被非语义文本噪音主导。
- 目标：
  - 补齐 P0 测试基线，让 stable-id 相关 LLVM 回归只锁定语义、namespace、linkage 与 helper family，而不再把旧 private helper 拼写当金标准。
- 必须实现的内容：
  1. 复核并迁移 `crates/scoopc/src/llvm/tests.rs` 中仍直接绑定旧 private helper spelling 的 stable-id 相关断言，至少覆盖：
     - `default_single_file_ir_helper_lowers_handle_main_without_hir_fallback`
     - `direct_call_with_real_outward_effect_uses_step_boundary_and_surface_resume_dispatch`
     - `closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`
     - `top_level_immutable_init_emits_explicit_root_frame_descriptor`
     - `effect_state_machine_functions_emit_explicit_root_frame_descriptors`
  2. 为 direct-invoke / resume / surface-resume owner-dispatch / explicit-root descriptor 路径补齐稳定 helper 或等价语义断言，验证：
     - 仍发布正确 family 的 private helper / descriptor
     - object/internal surface 与调用关系不漂移
     - 不把旧具体 spelling 当作唯一正确答案
  3. 扩充 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests` 或等价 source inventory，覆盖本轮新增清理的旧 private helper spelling。
- 必须遵从的约束：
  - 不得把断言放松成“只要出现 resume/descriptor 文本就算通过”；必须保持语义验证力度。
  - 不得把 `main`、runtime import、`@Extern` 这类显式 external 例外误纳入清理范围。
  - 不得把这类测试基线清理拖到 P3 再做；P0 必须先把 review 发现的噪音收口。
- 验证：
  1. `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  2. 针对上述迁移点的新增/更新 LLVM 定向测试全部通过。
  3. `cargo test -p scoopc`
- 完成条件：
  - P2/P3 将调整的 compiler-private helper naming surface 不再被现有行为测试直接锁死。
- 依赖：P0-T02
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`
  - 核心决策：
    - 在 `llvm/tests.rs` 增加少量 IR 语义匹配 helper（symbol/header 解析、descriptor 发布提取、函数计数），并把 `function_ir_matching` 的失败信息升级为输出可见函数头，便于后续 stable-id 迁移继续按语义定位 helper。
    - 把 `default_single_file_ir_helper_lowers_handle_main_without_hir_fallback`、`direct_call_with_real_outward_effect_uses_step_boundary_and_surface_resume_dispatch`、`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`top_level_immutable_init_emits_explicit_root_frame_descriptor`、`effect_state_machine_functions_emit_explicit_root_frame_descriptors` 等 review 指向入口，连同同类 direct-invoke / payload / tuple-carrier / virtual / interface / funptr 路径，一并从旧 private helper 全字符串断言迁移为 helper family、step/descriptor 发布、payload reload 与调用面语义断言。
    - 扩充 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests`，把本轮清理掉的 `__scoop_refactor_resume__...`、`__scoop_refactor_direct_invoke__...`、`__scoop_refactor_surface_resume_owner_dispatch__...`、`__scoop_top_level_val_init__...` 相关旧绑定固化成回归检查。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc real_outward_effect -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc explicit_root_frame -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc default_single_file_ir_helper_lowers_handle_main_without_hir_fallback -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc effect_step_single_tuple_param_closure_carrier_preserves_tuple_args_payload -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc effectful_funptr_call_uses_explicit_outcome_boundary -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc boxed_effect_payload_rebuilds_aggregate_from_explicit_frame_after_safepoint -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P0 的测试基线收口目标：后续 P2/P3 将修改的 compiler-private helper naming surface，已不再被现有 LLVM 行为测试直接锁死到旧拼写。
    - 对应 `STABLE_ID.md` §3.4 / §8.5 / §10：direct-invoke、resume、surface-resume owner-dispatch、top-level init descriptor 等 private surface 现在通过 helper family、descriptor 发布和调用关系建模；source inventory 也已阻止旧 private helper spelling 回流到行为测试。

### [TODO] P0-T02B：清理剩余 stable-id 敏感 LLVM / pipeline 测试中的当前 callable symbol 字符串绑定

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P0
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4、§7.3、§7.4、§10
- 背景：
  - `P0-T02R` 复核时发现，仓库里仍有一批 stable-id 敏感测试直接用当前 callable symbol 文本定位 LLVM IR 函数或调用点；这类断言虽然不一定再锁死 dense-id/private helper，但仍会在后续 user ABI / private symbol 命名迁移时把无关语义测试一起打断。
  - 当前已确认的入口至少包括：
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - 当前已确认的直接字符串绑定样例至少包括：
    - `@a.id64(`、`@a.id32(`、`@a.choose(`
    - `@a.entry(`、`@a.hidden(`、`@a.latent(`、`@a.keep(`、`@a.label(`、`@a.make(`、`@a.run(`、`@a.take(`、`@a.bounce(`
    - `@sample.effectEntry(`、`@"sample.effectEntry"`
    - `__scoop_refactor_direct_invoke__sample_effectEntry`
- 目标：
  - 在进入 P1/P2 之前，把 remaining stable-id 敏感行为测试从“依赖当前 callable symbol 拼写”迁移到“依赖语义、调用关系、namespace 角色、IR 结构”的断言模型。
- 必须实现的内容：
  1. 复核并迁移 `crates/scoopc/src/llvm/tests.rs` 中仍直接使用当前 callable symbol 文本定位函数/调用点的 stable-id 敏感断言，至少覆盖：
     - `float_builtin_types_lower_to_llvm_scalars`
     - `direct_effectful_signature_without_outward_effect_stays_on_direct_call_surface`
     - `direct_call_with_uncalled_effectful_higher_order_param_stays_on_direct_call_surface`
     - `closure_call_without_outward_effect_stays_on_direct_call_surface`
     - `indirect_gc_aggregate_param_syncs_explicit_frame_home_slot_on_entry`
     - `managed_function_emits_explicit_root_frame_tls_lifecycle_and_slot_clear`
     - `zero_slot_managed_function_still_emits_explicit_root_frame_lifecycle`
     - `managed_function_reloads_direct_gc_local_from_explicit_frame_after_safepoint`
     - `object_property_init_access_stays_plain_without_effect_boundary`
     - `class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval`
     - `deferred_call_arg_reloads_from_explicit_frame_after_later_safepoint`
     - `aggregate_call_arg_rebuilds_from_explicit_frame_after_safepoint`
     - `hidden_sret_aggregate_result_rebuilds_from_explicit_frame_slots`
     - `direct_hir_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref`
  2. 复核并迁移 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中仍把当前 callable symbol / direct-entry helper spelling 当作定位锚点的 stable-id 敏感断言，至少覆盖 `refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry`。
  3. 为上述测试补齐稳定 IR 查询 helper 或等价语义断言，使其可以通过以下信息定位目标，而不是依赖当前 symbol 拼写：
     - source role（例如 top-level entry、helper、wrapper）
     - signature/ABI shape
     - 调用关系与被调用者 family
     - explicit root frame / effect boundary / wrapper forwarding 的结构性特征
  4. 扩充 source inventory 或等价回归检查，防止 stable-id 敏感行为测试重新把当前 callable symbol 文本当作金标准。
- 必须遵从的约束：
  - 不得把断言弱化成“出现某个泛化子串即可”；迁移后仍必须能验证目标函数、调用面和行为语义。
  - 不得把 `main`、runtime/native import、`@Extern` 指定的 native symbol 误当作需要清理的 current callable symbol 绑定。
  - 不得只修一两个样例而已；要覆盖本次 review 已确认的同类入口。
- 验证：
  1. `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  2. `cargo test -p scoopc explicit_root_frame -- --nocapture`
  3. `cargo test -p scoopc direct_call_ -- --nocapture`
  4. `cargo test -p scoopc refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry -- --nocapture`
  5. `cargo test -p scoopc`
- 完成条件：
  - 后续 P1-P7 调整 callable symbol / linkage / namespace 时，现有 LLVM 与 pipeline 行为测试不会再因为当前 symbol 文本变化而误报回归。
- 依赖：P0-T02A
- 完成记录：
  - 待填。

### [TODO] P0-T02R：Review 审计脚手架与测试基线，确认后续任务不会被旧字符串绑定卡住

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P0
  - [`STABLE_ID.md`](./STABLE_ID.md) §10、§11、§12
- 重点：
  - object / symbol 审计入口是否已经能真实看见 external namespace 污染；
  - 测试是否还围绕 `scoop.lambda$0`、`__schema3`、`__scoop_object_init__...` 之类旧名字建模；
  - `.cone` / JSON 是否已被明确列为健康基线，而不是“下一步顺手重写”的对象。
- 必须检查的文件/位置：
  - `crates/scoopc/src/llvm/tests.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - 与新增 stable-id 测试 helper 对应的文件
  - `crates/scoopc/src/cone/scoopir/schema.rs`
  - `crates/scoopc/src/cone/pre_specialize.rs`
  - `crates/scoopc/src/cone/visibility.rs`
  - `crates/scoopc/src/cone/annotations.rs`
- 验证：
  - 重新运行 P0-T01 / P0-T02 的全部测试与 grep 审计。
  - 在完成记录中明确说明：后续任务将基于哪些测试入口验证 symbol、linkage、path-stability 和 dense-id 泄漏。
- 完成条件：
  - 可以明确写出：P1-P7 已有稳定审计基线，不会被旧名字断言或 schema churn 噪音主导。
- 依赖：P0-T02B
- 完成记录：
  - 待填。

## P1：建立统一 `stable_id` 基础设施

### [TODO] P1-T01：新增共享 `stable_id` 模块，收口 canonical encoder 与 shared hash helper

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P1
  - [`STABLE_ID.md`](./STABLE_ID.md) §6、§7、§8.1、§9 Phase 1
- 目标：
  - 在 `scoopc` 内建立单一 stable-id 基础设施入口，禁止后续每个子系统再各自维护一套 hash / mangling / label 逻辑。
  - 先把 canonical type/effect encoder 与 shared hash helper 定型。
- 必须实现的内容：
  1. 在 `crates/scoopc/src/lib.rs` 增加 `pub mod stable_id;`。
  2. 新增 `crates/scoopc/src/stable_id.rs`，至少承载：
     - canonical type / effect encoder
     - shared hash helper
     - 版本化 hash 前缀约定
     - dump label generator 的基础 API
  3. canonical encoder 必须覆盖 `STABLE_ID.md` §7.1：
     - nominal：`N(pkg.Name<...>)`
     - builtin value / ref：`V(Unit)`、`R(String)`
     - type param：`P(<owner-def-key>#<index>)`
     - function：`F(recv?; params... -> ret / row)`
     - tuple：`T(...)`
     - union：`U(...)`
     - effect row：排序去重后的 term 编码
  4. shared hash helper 必须收口到 `STABLE_ID.md` §7.2：
     - 使用 `SHA-256`
     - 使用固定版本前缀，例如 `abi0:`、`priv0:`、`rtti0:`
     - linker-visible symbol 截断为 128 bit hex
     - 仅在 runtime 固定要求 64 bit id 的场景允许 64 bit 截断
  5. 明确禁止直接复用 `TypeStore::display()`、raw `Debug` 文本或 path/span 文本作为 canonical 输入。
- 必须遵从的约束：
  - P1-T01 只建立基础设施，不大规模改调用点行为。
  - 不得新增第二份 `stable_hash64` / `stable_*suffix` / `Sha256::digest(...)` 私有工具函数。
  - 不得让 `sanitize_llvm_ident()` 进入 shared hash 主体。
- 验证：
  1. 为 `stable_id` 模块新增单元测试，至少覆盖：
     - 相同语义输入在不同顺序下编码一致
     - 不同 surface 前缀 hash 不冲突
     - pretty text 不参与 canonical 主体
  2. 运行：
     - `cargo test -p scoopc stable_id -- --nocapture`
     - `cargo test -p scoopc canonical_ -- --nocapture`
- 完成条件：
  - 仓库中已经存在唯一的 stable-id 基础模块，后续 P2-P6 可直接复用。
- 依赖：P0-T02R
- 完成记录：
  - 待填。

### [TODO] P1-T02：落地 stable key / mangler / label API，并收口仓库内分叉 hash 实现

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P1
  - [`STABLE_ID.md`](./STABLE_ID.md) §6、§7.3、§8.1
- 目标：
  - 在共享模块中补齐 stable key、`AbiMangler`、`PrivateSymbolMangler` 与 stable label API。
  - 删除仓库内分叉的 `stable_hash64` 实现，改为统一调用 shared helper。
- 必须实现的内容：
  1. 在 `crates/scoopc/src/stable_id.rs` 中落地以下 key 与 API：
     - `StableConeKey`
     - `StableDefKey`
     - `StableTemplateKey`
     - `StableInstanceKey`
     - `StableClosureKey`
     - `StableCallSiteKey`
     - `StableEffectSchemaKey`
     - `StableContinuationSchemaKey`
     - `StableBoundaryKey`
     - `StableStateKey`
     - `StableFrameSlotKey`
     - `AbiMangler`
     - `PrivateSymbolMangler`
     - stable local label API
  2. `StableConeKey` 的最小实现必须来自 cone 名称 / 版本，而不是 `ConeId`，参考 `STABLE_ID.md` §6.1 与 `crates/scoopc/src/frontend.rs:378-401`。
  3. `StableTemplateKey` / `StableInstanceKey` 不得直接复用 `TemplateKey { fqn, source_path, decl_span }` 或 `TypeId` 作为 exported identity，参考 `crates/scoopc/src/mir/materialize.rs:53-134`。
  4. 删除或迁移以下分叉 hash helper：
     - `crates/scoopc/src/rtti/mod.rs:819`
     - `crates/scoopc/src/rtti/type_desc.rs:1742-1745`
     - `crates/scoopc/src/llvm/codegen/mod.rs:8505-8510`
     - `crates/scoopc/src/itable.rs:970-975`
  5. 为 `AbiMangler` 与 `PrivateSymbolMangler` 固定名字模式，至少符合 `STABLE_ID.md` §7.3：
     - `__scoop_abi0_fun__...__h<hash128>`
     - `__scoop_abi0_global__...__h<hash128>`
     - `__scoop_abi0_type__...__h<hash128>`
     - `__scoop_priv0__<role>__h<hash128>`
- 必须遵从的约束：
  - `TemplateKey`、`InstanceKey`、`CallSite` 等旧结构可继续保留给内部实现使用，但不得继续直接承担对外协议 identity。
  - 不得简单把旧 dense id 包一层 hash 继续用作 stable key。
  - `sanitize_llvm_ident()` 只能参与可读前缀，不能进唯一性主体。
- 验证：
  1. `cargo test -p scoopc stable_id -- --nocapture`
  2. 精确搜索：
     - `fn stable_hash64`
     - `Sha256::digest`
     - `stable_template_symbol_suffix`
     确认新实现已向 shared helper 收口。
- 完成条件：
  - 后续命名 / linkage / RTTI / dump 任务不再需要自行定义 key / hash / mangling 规则。
- 依赖：P1-T01
- 完成记录：
  - 待填。

### [TODO] P1-T02R：Review `stable_id` 基础设施，确认后续阶段已有唯一 authoritative API

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P1
  - [`STABLE_ID.md`](./STABLE_ID.md) §6、§7、§8.1
- 重点：
  - `stable_id` 模块是否已经覆盖后续 P2-P6 所需的 key / hash / mangler / label API；
  - 仓库内是否仍残留多份自带 `stable_hash64` 或临时 digest helper；
  - `StableConeKey` / `StableDefKey` / `StableInstanceKey` 是否已明确脱离 `ConeId`、path/span 与 `TypeId`。
- 必须检查的文件/位置：
  - `crates/scoopc/src/stable_id.rs`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/src/rtti/mod.rs`
  - `crates/scoopc/src/rtti/type_desc.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/itable.rs`
- 验证：
  - 重新运行 P1-T01 / P1-T02 的单元测试与精确搜索。
  - 在完成记录中明确列出剩余允许保留的旧结构与其职责边界。
- 完成条件：
  - 可以明确写出：P2-P6 的所有 identity 逻辑都必须只通过 `stable_id` 模块接入。
- 依赖：P1-T02
- 完成记录：
  - 待填。

## P2：收紧 linkage，先处理 external namespace 污染

### [TODO] P2-T01：分类 `module.add_function(..., None)` 调用点并建立统一 declaration/linkage helper

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P2
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4.1、§3.4.4、§3.4.5、§7.4、§8.6
- 目标：
  - 在改名字前先把 LLVM function declaration 的可见性规则分类清楚。
  - 明确区分三类函数：
    - 真正导出 ABI symbol
    - runtime / native import
    - compiler-private helper
- 必须实现的内容：
  1. 审计 `crates/scoopc/src/llvm/**` 中所有 `module.add_function(name, fn_ty, None)` 调用点，并按上面三类建立归档。
  2. 为 function declaration 抽统一 helper 或等价封装，要求调用方必须显式说明：
     - 是否 external/exported
     - 是否 runtime/native import
     - 是否 private helper
  3. 优先覆盖以下高风险入口：
     - `crates/scoopc/src/llvm/codegen/mod.rs:2321-2423`
     - `crates/scoopc/src/llvm/codegen/mir_body.rs:306-365`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6190-6193`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:1641-2856`、`8078`
     - `crates/scoopc/src/llvm/codegen/object_init.rs:101-182`
     - `crates/scoopc/src/llvm/codegen/closure/mod.rs:156`
     - `crates/scoopc/src/llvm/emit.rs:683`
     - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  4. 在 helper 注释里写明 `main`、`malloc`、`exit`、runtime ABI entry、`@Extern` symbol 属于显式 external 例外。
- 必须遵从的约束：
  - 本任务建立的是 declaration / linkage 决策边界，不要求名字文本已迁成最终 stable 形态。
  - 不得把 runtime import、`main` 或 `@Extern` symbol 误接到 private linkage。
  - 不得让 helper 成为“ exported 与 private 逻辑大杂烩”的分支黑盒。
- 验证：
  1. `cargo test -p scoopc`
  2. 精确搜索 `module\.add_function\(.*None\)`，确认剩余命中都能解释为真正 external / import 场景。
  3. 新增/更新围绕 declaration 分类的定向测试。
- 完成条件：
  - LLVM function declaration 的可见性分类已经统一收口，不再依赖调用点自行决定。
- 依赖：P1-T02R
- 完成记录：
  - 待填。

### [TODO] P2-T02：把 compiler-private helper 从 external namespace 收回 internal/private

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P2
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4.1、§3.4.4、§3.4.5、§8.5、§8.6
- 目标：
  - 在名字文本尚未完全稳定化之前，先消除最危险的 linker 冲突风险。
  - 让 compiler-private helper 不再进入 object / linker 外部符号空间。
- 必须实现的内容：
  1. 把以下 compiler-private helper 明确改为 `InternalLinkage` 或 `PrivateLinkage`：
     - object init bridge：`crates/scoopc/src/llvm/codegen/object_init.rs:101-117`
     - object init function（若仅本模块内部消费）：`crates/scoopc/src/llvm/codegen/object_init.rs:165-182`
     - top-level init bridge：`crates/scoopc/src/llvm/codegen/mod.rs:3702-3718`、`8305-8306`
     - closure body / resume / env / effect helper shell 等当前仍 default external 的 helper
  2. 对 source-level top-level function、materialized plain callable、effect-lowered plain callable 的 declaration path 明确区分：
     - 真正导出的符号保留 external
     - 仅模块内使用的实现体必须 internal/private
  3. 复核当前已正确 internal 的 global，只保留、不回退：
     - `ensure_struct_anchor()` / `ensure_case_tag_constant()`
     - 多个 descriptor global
  4. 更新 `crates/scoopc/src/llvm/tests.rs`，使 object-level 验证直接检查 private helper 不再进入 external symbol 集。
- 必须遵从的约束：
  - 不得因 internalize 而改变 callable 语义或调用关系。
  - 不得把真正用户可见 ABI symbol 错误隐藏。
  - 不得以“后面名字还要改”为由继续保留 private helper external 化。
- 验证：
  1. `cargo test -p scoopc`
  2. 运行新增的 external symbol 审计测试，确认 private helper 不再出现于 external symbol 集。
  3. 对 `module\.add_function\(.*None\)` 做再次审计，剩余命中必须可解释。
- 完成条件：
  - compiler-private helper 已不再污染外部符号空间。
- 依赖：P2-T01
- 完成记录：
  - 待填。

### [TODO] P2-T02R：Review linkage 收口，确认 namespace 风险已经先于命名迁移被压住

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P2
  - [`STABLE_ID.md`](./STABLE_ID.md) §7.4、§8.6
- 重点：
  - external symbol 集中是否仍残留明显的 compiler-private helper；
  - `main`、runtime/native import、`@Extern` 是否仍保持正确 external 行为；
  - 本轮 linkage 调整是否没有引入任何 ABI 语义漂移。
- 验证：
  - 重新运行 P2-T01 / P2-T02 的 object symbol 审计与 `cargo test -p scoopc`。
  - 在完成记录中明确列出仍允许 external 的固定例外清单。
- 完成条件：
  - 可以明确写出：后续即使名字继续演进，也不会再因为 private helper external 化导致主要 linker 风险。
- 依赖：P2-T02
- 完成记录：
  - 待填。

## P3：迁移 private LLVM naming source

### [TODO] P3-T01：用 `StableClosureKey` 替换 closure private naming，并清理旧 alias 兼容层

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P3
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4.3、§6.5、§8.5
- 目标：
  - 让 closure body / resume / env / carrier alias 的名字不再由 `ClosureId` 分配顺序驱动。
  - 为后续 RTTI closure env canonical name 提供同一份 `StableClosureKey`。
- 必须实现的内容：
  1. 迁移以下 closure 命名入口：
     - `crates/scoopc/src/llvm/codegen/closure/mod.rs:79`、`156`
     - `crates/scoopc/src/llvm/codegen/closure/mod.rs:508`、`523`
     - `crates/scoopc/src/llvm/codegen/closure/mod.rs:697-733`
     - `crates/scoopc/src/llvm/codegen/closure/mod.rs:767-770`
     - `crates/scoopc/src/llvm/codegen/ordinary_callee.rs:365-394`
     - `crates/scoopc/src/llvm/codegen/gc.rs:1722-1726`
  2. `StableClosureKey` 必须至少由以下信息构成：
     - owner callable 的 `StableDefKey` 或 `StableInstanceKey`
     - lambda 在 owner 内的语义路径
  3. 清理 closure carrier alias 兼容层，避免继续把 direct HIR closure alias 映射回 `scoop.lambda$<n>`：
     - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:6980-6985`
     - `crates/scoopc/src/llvm/codegen/mod.rs:783-819`
  4. 新 private closure name 必须统一通过 `PrivateSymbolMangler` 生成，并与 P2 的 internal/private linkage 配套。
- 必须遵从的约束：
  - 不得改变 closure 的 callable ABI、capture 语义或 ordinary callee 语义。
  - 不得简单把 `ClosureId` 包成 hash 继续使用。
  - 若保留可读前缀，只能来自稳定语义文本，不得让其承担唯一性。
- 验证：
  1. `cargo test -p scoopc`
  2. 精确搜索：
     - `scoop\.lambda\$[0-9]+`
     - `scoop\.lambda_resume\$[0-9]+`
     - `scoop\.lambda_env\$[0-9]+`
     目标是在 linker-visible naming 路径中不再命中。
  3. 更新 object / IR 定向测试，改为检查 private closure symbol 的稳定 hash 主体与 internal/private linkage。
- 完成条件：
  - closure private naming 已彻底脱离 `ClosureId` 分配顺序。
- 依赖：P2-T02R
- 完成记录：
  - 待填。

### [TODO] P3-T02：用 stable schema key / canonical type hash 替换 effect helper 与 transport type 的 private naming

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P3
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4.4、§3.4.6、§6.7、§6.8、§8.5
- 目标：
  - 让 effect helper、continuation helper、transport box/type 的 private naming 脱离 `StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`TypeId`。
- 必须实现的内容：
  1. 迁移以下 effect helper 命名入口：
     - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:144-156`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:569-572`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:1137-1140`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:2514-2580`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:100-105`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:2766-2830`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:7947-7955`
  2. 迁移 transport box/type 命名入口：
     - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:10885-10906`
  3. `StableEffectSchemaKey` / `StableContinuationSchemaKey` 的输入必须来自 authoritative semantic contents，而不是 allocator id 包 hash。
  4. transport type / box 名字不得再包含 `TypeId` 或 pretty-printer 文本作为唯一性主体；唯一性必须来自 canonical type hash。
  5. 所有新 private effect helper name 必须统一走 `PrivateSymbolMangler`。
- 必须遵从的约束：
  - 不得改变 effect-lowered step contract、continuation contract 或 runtime ABI 语义。
  - 不得把 `sanitize_llvm_ident()` 或 `types.display(...)` 留在唯一性主路径上。
  - 不得留下“部分 helper 已 stable、部分 helper 仍 `__schema3` / `k2`”的半迁移状态。
- 验证：
  1. `cargo test -p scoopc`
  2. 精确搜索：
     - `__schema[0-9]+`
     - `__k[0-9]+`
     - `t[0-9]+__`
     目标是在 linker-visible naming 路径中不再命中。
  3. 新增或更新 effect helper object / IR 测试，确认 helper 名字来自 stable hash，且仍保持 internal/private。
- 完成条件：
  - effect helper 与 transport type 的 private naming 已彻底脱离 allocator 顺序和 pretty text。
- 依赖：P3-T01
- 完成记录：
  - 待填。

### [TODO] P3-T02R：Review private naming 迁移，确认 dense id 已退出 LLVM private symbol 控制路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P3
  - [`STABLE_ID.md`](./STABLE_ID.md) §6.5-§6.8、§10
- 重点：
  - `ClosureId`、`StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`TypeId` 是否仍直接决定任何 private helper symbol 文本；
  - closure / effect helper / transport type 是否都已统一通过 `PrivateSymbolMangler`；
  - private naming 改造是否与 P2 linkage 收口保持一致。
- 验证：
  - 重新运行 P3-T01 / P3-T02 的 object / IR / grep 审计。
  - 在完成记录中明确列出剩余允许保留数字的场景，例如 LLVM block label、SSA 临时名等。
- 完成条件：
  - 可以明确写出：dense id 已退出 private LLVM naming 的 authoritative path。
- 依赖：P3-T02
- 完成记录：
  - 待填。

## P4：迁移 exported ABI naming

### [TODO] P4-T01：重写 overload / template / instance 的 exported identity 来源

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P4
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4.2、§6.2-§6.4、§8.3
- 目标：
  - 把 overload suffix 与 instance exported identity 从 `source_path + decl_span`、`TypeId`、pretty text 中解耦。
  - 明确分离“调试显示名”和“导出 ABI 名”。
- 必须实现的内容：
  1. 重写 `stable_template_symbol_suffix()` 的输入来源：
     - 当前入口：`crates/scoopc/src/mir/materialize.rs:8638-8647`
     - 当前同类逻辑：`crates/scoopc/src/hir/lower/util.rs:3721-3769`
     - 目标：改为 `StableDefKey + canonical signature key`
  2. 重写 `instance_fqn()` 的 exported 语义：
     - 当前实现：`crates/scoopc/src/mir/materialize.rs:8505-8525`
     - 要求：
       - 保留 display 名给 dump / debug
       - 新增 exported symbol 路径走 `StableInstanceKey + AbiMangler`
  3. 明确 `TemplateKey { fqn, source_path, decl_span }` 与 `InstanceKey { template, type_args, eff_args }` 的边界：
     - 旧结构可以保留给内部 materialization 使用
     - 但 exported naming 必须派生自 stable key，而不是直接消费旧结构内容
  4. 为 generic / overload 补 path-stability 测试样例，覆盖：
     - 同一源码不同 checkout 根路径
     - 同名 overload
     - generic instance materialization
- 必须遵从的约束：
  - 不得原地把所有 `fqn` 字段重写成 mangled symbol；`fqn` 仍需保留源级语义。
  - 不得继续把 `source_path`、`decl_span`、`TypeStore::display()`、`sanitize_llvm_ident()` 混入 exported 唯一性主体。
  - 不得引入长期 alias 兼容层，除非确认存在现实外部消费者且用户明确要求。
- 验证：
  1. `cargo test -p scoopc`
  2. 精确搜索：
     - `stable_template_symbol_suffix`
     - `source_path.*decl_span`
     - `instance_fqn\(`
     目标是在 exported naming 路径中不再依赖旧逻辑。
  3. 新增/更新 path-stability 定向测试。
- 完成条件：
  - overload / template / instance 的 exported identity 已经脱离 path/span / pretty text / `TypeId`。
- 依赖：P3-T02R
- 完成记录：
  - 待填。

### [TODO] P4-T02：把 `AbiMangler` 接入 exported declaration path，并验证跨路径稳定性

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P4
  - [`STABLE_ID.md`](./STABLE_ID.md) §5.2、§7.3、§7.4、§8.5
- 目标：
  - 让真正导出的 function/global/type metadata symbol 全部由统一 `AbiMangler` 生成。
  - 证明同一输入在不同 checkout 路径下 external symbol 集不变。
- 必须实现的内容：
  1. 在以下 declaration path 中接入 `AbiMangler`：
     - source-level top-level function：`crates/scoopc/src/llvm/codegen/mod.rs:2321-2423`
     - materialized plain callable：`crates/scoopc/src/llvm/codegen/mir_body.rs:306-365`
     - effect-lowered plain callable：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:1219-1265`
  2. 为 exported function/global/type metadata 固定 namespace 模式，至少符合 `STABLE_ID.md` §7.3。
  3. 确保 `StableConeKey` 真正进入 exported naming 路径；若构建层发现 key 冲突，必须显式失败而不是继续生成碰撞 symbol。
  4. 为不同 checkout 根路径下的 external symbol 集增加自动化对比测试。
     - 建议通过临时目录复制同一 cone，再比较 object 外部符号表。
  5. 同时保留固定例外：
     - `main`
     - `@Extern` 指定 symbol
     - runtime/native 固定入口
- 必须遵从的约束：
  - 不得把 private helper 错接到 `AbiMangler`。
  - 不得把导出 ABI name 的稳定性依赖回 `sanitize_llvm_ident()` 或 display 文本。
  - 不得改变 exported symbol 的语义可见性决策，只改命名来源和 namespace 规则。
- 验证：
  1. `cargo test -p scoopc`
  2. 运行新增的跨路径 symbol 对比测试。
  3. 对 multi-cone / generic / overload 样例做链接级 collision smoke，确认 external symbol 不碰撞。
- 完成条件：
  - 同一份输入在不同 checkout 路径下导出的 external symbol 集完全一致。
- 依赖：P4-T01
- 完成记录：
  - 待填。

### [TODO] P4-T02R：Review exported ABI naming，确认 exported 与 private namespace 已完全分家

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P4
  - [`STABLE_ID.md`](./STABLE_ID.md) §5.2、§7.3、§10
- 重点：
  - exported symbol 是否已统一走 `AbiMangler`；
  - `main` / `@Extern` / runtime 固定入口是否仍作为显式例外正常工作；
  - `fqn` 是否仍保留源级语义而未被 whole-sale 改写成 mangled symbol。
- 验证：
  - 重新运行 P4-T01 / P4-T02 的 path-stability、multi-cone collision 与 `cargo test -p scoopc`。
  - 在完成记录中明确列出 exported 与 private namespace 的最终分类规则。
- 完成条件：
  - 可以明确写出：exported ABI naming 已与 private LLVM helper naming 完全分家。
- 依赖：P4-T02
- 完成记录：
  - 待填。

## P5：重写 dump / fixture renderer

### [TODO] P5-T01：重写 HIR / MIR / materialized IR dump renderer，并刷新相关 fixture

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P5
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.2.1-§3.2.3、§8.2
- 目标：
  - 先把 HIR、MIR、`dump-ir` 从 raw `Debug` 与字符串补丁迁到稳定 renderer。
  - 一次性刷新相关 fixture expect，使这些 surface 不再建立在 allocator 顺序上。
- 必须实现的内容：
  1. HIR dump 改造：
     - `crates/scoopc/src/pipeline/hir_stage.rs:1217-1223`
     - `crates/scoop/src/fixtures/mod.rs:1373-1380`
     - `crates/scoop/src/commands/dump_hir.rs`
     - 目标：不再直接输出 `format!("{:#?}\n", lowered.file)`；`TypeId`、`SymbolId`、`ClosureId` 退出对外文本协议。
  2. MIR dump 改造：
     - `crates/scoopc/src/pipeline/mir_stage.rs:142-174`
     - `crates/scoop/src/fixtures/mod.rs:1402-1411`
     - `crates/scoop/src/commands/dump_mir.rs`
     - 目标：删除 `TypeId` 文本 canonicalize 字符串补丁；`bb` / `local` / `site` label 改由 stable local key 派生。
  3. materialized IR dump 改造：
     - `crates/scoop/src/commands/dump_ir.rs:14-18`
     - `crates/scoopc/src/mir/materialize.rs`
     - 目标：实例显示名来自 stable instance display / local label，而不是 `tN`。
  4. 刷新相关 fixture：
     - `tests/fixtures/hir/**`
     - `tests/fixtures/mir/**`
     - `tests/fixtures/mir_refactor/**`
  5. 确保 renderer 自己负责排序与 label 分配，不依赖 `Vec` / `IndexMap` 的自然遍历顺序。
- 必须遵从的约束：
  - 不得通过修改 `Debug` impl 的语义来“顺带修复” dump surface。
  - 不得再做字符串后处理式 canonicalize。
  - 不得把 fixture refresh 当作吞掉真实行为变化的手段。
- 验证：
  1. `cargo test -p scoopc`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor`
  5. 精确搜索刷新后的 dump / fixture 输出，不再包含：
     - `TypeId(`
     - `S0`
     - `C0`
     - `bb0`
     - `site0`
- 完成条件：
  - HIR / MIR / `dump-ir` 及其 fixture 已彻底脱离 raw `Debug` 协议。
- 依赖：P4-T02R
- 完成记录：
  - 待填。

### [TODO] P5-T02：重写 effect facts / effect lowered dump renderer，并刷新相关 snapshot

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P5
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.2.4、§3.2.5、§8.2
- 目标：
  - 把 effect facts 与 effect lowered 的对外文本协议从 allocator-derived label 迁到语义 label / stable local label。
- 必须实现的内容：
  1. effect facts dump 改造：
     - `crates/scoopc/src/effect_facts/dump.rs:77-170`、`306-374`、`665-711`、`756-765`
     - `step_schema#N`、`continuation_schema#N`、`case#N`、`bbN`、`siteN` 必须全部迁出。
  2. effect lowered dump 改造：
     - `crates/scoopc/src/effect_lowered/dump.rs:116-216`、`565-665`、`1066-1145`、`1425-1718`、`1782-1936`
     - `crates/scoopc/src/effect_lowered/ir.rs:155`
     - `t/s/k/c/ri/ko/st/bd/fs/local/bb/site` 全部迁出。
  3. 复核与这些 dump surface 相关的 stage output 测试入口：
     - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
     - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
     - `crates/scoopc/src/effect_lowered/ir.rs`
  4. 刷新对应 snapshot / unit test 期待值，并在完成记录中说明主要 textual 变化类别。
- 必须遵从的约束：
  - schema / continuation / state / boundary / frame slot label 可以改成 stable key 或语义 label，但不得再直接由 allocator 顺序决定。
  - 不得在 renderer 中保留 “如果 label 冲突就回退到旧 dense id” 的兼容旁路。
  - 不得借 dump 改造改变 effect facts 或 effect lowered 的实际语义结构。
- 验证：
  1. `cargo test -p scoopc`
  2. 定向运行 `stable_dump()` 相关测试与 stage tests。
  3. 精确搜索 dump 输出中不再包含：
     - `step_schema#0`
     - `k0`
     - `ri0`
     - `ko0`
     - `st0`
     - `bd0`
     - `fs0`
- 完成条件：
  - effect facts / effect lowered dump 已彻底脱离 allocator-derived 协议。
- 依赖：P5-T01
- 完成记录：
  - 待填。

### [TODO] P5-T02R：Review dump / fixture 迁移，确认所有 textual surface 已与 raw `Debug` 脱钩

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P5
  - [`STABLE_ID.md`](./STABLE_ID.md) §5.1、§5.2、§10
- 重点：
  - HIR / MIR / IR / effect facts / effect lowered 是否都已有独立稳定 renderer；
  - fixture 更新是否只反映 identity surface 变化；
  - 是否仍残留“先 `Debug` 再字符串补丁”的路径。
- 验证：
  - 重新运行 P5-T01 / P5-T02 的全部测试与文本搜索。
  - 在完成记录中给出仍允许保留 `Debug` 的内部用途边界。
- 完成条件：
  - 可以明确写出：所有 active dump / fixture surface 均已脱离 raw `Debug` 协议。
- 依赖：P5-T02
- 完成记录：
  - 待填。

## P6：收尾 RTTI / JSON / shared hash helper

### [TODO] P6-T01：统一 RTTI / interface hash helper，并修复 closure env identity 来源

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P6
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.1、§3.3、§8.4
- 目标：
  - 让 RTTI / interface id / type id 的 hash 规则全面收口到 shared `stable_id` helper。
  - 修掉 closure env 仍然由 `ClosureId` 决定 canonical name 与 `type_id` 的最后一处显式外泄。
- 必须实现的内容：
  1. 迁移 `dump-rtti` closure env identity 入口：
     - `crates/scoopc/src/rtti/type_desc.rs:323-328`
     - 目标：`StableClosureKey -> canonical name -> shared hash helper`
  2. 统一 RTTI / interface hash helper：
     - `crates/scoopc/src/rtti/mod.rs`
     - `crates/scoopc/src/rtti/type_desc.rs`
     - `crates/scoopc/src/itable.rs`
  3. 复核 interface/runtime-match 相关 type id 与 interface id 的输入前缀，确保同类 surface 统一规范。
  4. 对以下健康 schema 做一次最后的 dense-id / 路径审计，而不是结构重写：
     - `crates/scoopc/src/cone/scoopir/schema.rs`
     - `crates/scoopc/src/cone/pre_specialize.rs`
     - `crates/scoopc/src/cone/visibility.rs`
     - `crates/scoopc/src/cone/annotations.rs`
- 必须遵从的约束：
  - 不得把 closure env 的 `ClosureId` 简单包成 hash 继续使用；必须先改 canonical name 来源。
  - 不得为了“统一风格”顺手重写健康 `.cone` / JSON schema。
  - 同一类 RTTI / interface identity 必须共用同一份 helper，不得保留分叉实现。
- 验证：
  1. `cargo test -p scoopc`
  2. 重点复核 `crates/scoopc/src/rtti/type_desc.rs` 中 `dump_rtti_*` 相关测试。
  3. 精确搜索：
     - `fn stable_hash64`
     - `ClosureId`
     - `scoop.lambda_env$`
     目标是在 RTTI identity path 中不再命中旧 closure env 逻辑。
- 完成条件：
  - RTTI / interface / type identity 已统一收口，closure env 不再依赖 `ClosureId` 分配顺序。
- 依赖：P5-T02R
- 完成记录：
  - 待填。

### [TODO] P6-T01R：Review RTTI 与 JSON 收口，确认剩余外部 surface 已只需最终验收

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P6
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.1、§3.3、§10
- 重点：
  - closure env canonical name / `type_id` 是否已完全脱离 `ClosureId`；
  - RTTI / interface id 是否已统一走 shared helper；
  - `.cone` / JSON 是否保持“防回归基线”而无无谓格式变动。
- 验证：
  - 重新运行 P6-T01 的 RTTI tests、`cargo test -p scoopc` 与 JSON baseline 审计。
  - 在完成记录中明确列出：哪些 JSON / cache surface 被证明无需再改。
- 完成条件：
  - 可以明确写出：除最终全量审计外，stable-id 方案的技术迁移面已经闭合。
- 依赖：P6-T01
- 完成记录：
  - 待填。

## P7：全量审计、fixture refresh 与无功能漂移验收

### [TODO] P7-T01：运行最终审计矩阵，刷新快照，并验证路径稳定性 / 多 cone / 无功能漂移

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P7
  - [`STABLE_ID.md`](./STABLE_ID.md) §10、§11、§12
- 目标：
  - 在所有迁移完成后，做一次完整的“identity 已稳定、语义未漂移”收口。
  - 刷新所有应变更的 textual surface，并证明这些变化仅限 identity 层。
- 必须实现的内容：
  1. 运行并记录 `STABLE_ID.md` §11 的完整 grep 审计清单；对每个残余命中分类说明：
     - 合法内部 handle
     - 仍需整改的外部泄漏
     - 测试数据 / 注释 / 误报
  2. 刷新并复核所有受影响的 fixture / snapshot：
     - `tests/fixtures/hir/**`
     - `tests/fixtures/mir/**`
     - `tests/fixtures/mir_refactor/**`
     - RTTI / stage dump 相关 snapshot
  3. 运行路径稳定性验证：
     - 同一输入复制到两个不同 checkout 根目录下编译
     - external symbol 集、RTTI identity、dump label 结果应保持一致
  4. 运行多 cone / collision 验证：
     - 两个 cone 即使内部 closure / site / schema 编号都从 0 开始，也不应因 helper 名字碰撞而链接失败
  5. 运行无功能漂移验证矩阵：
     - `cargo test -p scoopc`
     - `cargo test -p scoop_runtime`
     - `cargo run -p scoop -- test`
     - `cargo run -p scoop_tools -- spec-fixtures check`
     - 若环境允许，`cargo test --all`
  6. 清理过渡期 helper、双轨 name builder、旧 alias 兼容层，确保最终树上不再保留临时旁路。
- 必须遵从的约束：
  - fixture refresh 不能只看文本 diff；必须逐项确认没有吞掉真实语义漂移。
  - 若 full audit 暴露 ABI / runtime / effect 语义回归，必须回到对应阶段修正，不能靠更新 snapshot 通过。
  - 不得在最终阶段重新引入任何 dense-id identity 兼容层。
- 验证：
  1. `cargo test -p scoopc`
  2. `cargo test -p scoop_runtime`
  3. `cargo run -p scoop -- test`
  4. `cargo run -p scoop_tools -- spec-fixtures check`
  5. 若环境允许，`cargo test --all`
  6. 运行 `STABLE_ID.md` §11 的完整 grep 清单并附结果摘要。
- 完成条件：
  - `PLAN.md` §6 与 `STABLE_ID.md` §10 的验收标准全部可以明确陈述为已满足。
- 依赖：P6-T01R
- 完成记录：
  - 待填。

### [TODO] P7-T01R：Review 全量收口结果，确认 stable-id 方案已闭合且未带来功能漂移

- 参考：
  - [`PLAN.md`](./PLAN.md) §5、§6
  - [`STABLE_ID.md`](./STABLE_ID.md) §10、§11、§12
- 重点：
  - external surface 是否已全部脱离 dense id / path / raw `Debug` / pretty text 的直接控制；
  - compiler-private helper 是否已全部 internal/private；
  - exported ABI symbol 是否已统一走 `AbiMangler`；
  - dump / fixture / RTTI / JSON / object surface 的变化是否全部属于预期 identity 变化；
  - 是否没有引入语言语义、运行结果、effect / continuation / GC 行为漂移。
- 验证：
  - 重新检查 P7-T01 的全部审计输出与回归结果。
  - 在完成记录中给出一份最终结论清单，对应 `PLAN.md` §6 的 8 条完成标准逐项签收。
- 完成条件：
  - 可以明确写出：stable-id 整改已经闭合，后续若还有工作，只属于增量优化或新需求，而不再是本轮 identity 治理主线。
- 依赖：P7-T01
- 完成记录：
  - 待填。
