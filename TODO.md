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

### [DONE] P0-T02B：清理剩余 stable-id 敏感 LLVM / pipeline 测试中的当前 callable symbol 字符串绑定

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
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - 核心决策：
    - 在 `llvm/tests.rs` 补齐一组稳定 IR 查询 helper：direct call target 解析、defined global 查询、explicit-root descriptor -> offsets 解析、actual symbol 提取、user-callable 角色判定、测试侧 LLVM ident sanitize。行为测试现在先用 `function_ir_matching(...)` 依 source role / ABI 形状 / explicit-frame / effect-boundary / ctor-root / aggregate rebuild 等结构特征锁定目标函数，再只把“运行时实际解析出的 symbol”用于调用关系断言，不再把某个固定 callable symbol spelling 当金标准。
    - 按“同根问题成组清理”扩展了迁移范围：除任务列出的最小入口外，也一并迁移了同类 `a.helper` / `a.main` / explicit-root descriptor / frame type 派生名字绑定，以及 `object_member_call_uses_gc_managed_singleton_receiver`、`managed_function_emits_explicit_root_frame_descriptor`、`explicit_frame_layout_flattens_indirect_gc_aggregate_params` 等相邻 stable-id 敏感行为测试，避免后续 P1-P7 再被 sibling case 卡住。
    - `pipeline/llvm_codegen_stage.rs` 中新增本地 IR matcher / symbol / defined-call-target helper；`refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry` 现在通过 defined-function call graph 识别 direct-entry shell 与 dynamic shell，并断言 `main`/dynamic wrapper 都只转发到该 direct-entry shell，而不是依赖 `sample.effectEntry` / `__scoop_refactor_*sample_effectEntry` 当前 spelling。
    - 扩充 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests`，现在同时扫描 `llvm/tests.rs` 与 `pipeline/llvm_codegen_stage.rs`，把本轮移除的 callable symbol / explicit-root 派生命名硬编码固化为防回流清单。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc direct_call_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc explicit_root_frame -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P0 的“测试面先解除对现有命名形状的强绑定”要求：LLVM 与 pipeline 行为测试现在主要锁定语义、调用关系、namespace 角色与 IR 结构，不再把当前 callable symbol 文本当成正确性标准。
    - 对应 `STABLE_ID.md` §3.4 / §7.3 / §7.4 / §10：top-level callable、direct-entry shell、derived explicit-root descriptor / frame type 等 identity surface 已从当前 spelling 断言中解耦；后续 user ABI / private symbol / linkage 迁移时，这些测试将继续验证行为语义而非旧名字。

### [DONE] P0-T02C：清理 review 发现的剩余 stable-id 敏感 LLVM / pipeline 测试字符串绑定

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P0
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.4、§8.5、§10
- 背景：
  - `P0-T02R` 预检查发现，`crates/scoopc/src/llvm/tests.rs` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 里仍有一批 stable-id 敏感行为测试直接绑定当前 private / descriptor / callable symbol 文本；这些断言没有再使用旧 dense-id 样式，但仍会在后续 P3/P4 迁移 private naming source 与 exported/user ABI mangling 时把无关语义测试一起打断。
  - 当前已确认的直接字符串绑定至少包括：
    - `crates/scoopc/src/llvm/tests.rs`
      - `__scoop_refactor_closure_step_adapter__a_main__lambda0__`
      - `__scoop_refactor_closure_dynamic_entry__a_main__lambda0`
      - `__scoop_refactor_plain_adapter__a_choose__lambda0__`
      - `__scoop_refactor_closure_step_adapter__a_choose__lambda1__`
      - `__scoop_refactor_closure_dynamic_entry__a_choose__lambda1`
      - `__scoop_type_desc_class__a_Box_String_`
      - `__scoop_object_instance__a.Helper`
      - `__scoop_type_desc_runtime__enum_boxed_payload__a_Result__Ok`
      - `__scoop_type_desc_runtime__enum_boxed_payload__a_Result__Msg`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
      - `@__scoop_composite_transport_desc__inline__sample_Named`
      - `@__scoop_composite_transport_desc__erased__sample_Named`
      - `@__scoop_type_desc_mir_value_box__sample_Named`
      - `@__scoop_composite_transport_desc__inline__sample_Outer`
      - `@__scoop_type_desc_runtime__enum_boxed_payload__sample_Outer__UnitPair`
      - `@__scoop_type_desc_runtime__enum_boxed_payload__sample_Outer__Nested`
      - `@__scoop_composite_transport_desc__erased__sample_Outer`
      - `@__scoop_type_desc_mir_value_box__sample_Outer`
      - `__scoop_type_desc_mir_capture_box__sample_Point`
      - `__scoop_refactor_thread_resume_transport__`
      - `@sample.main`
      - `@sample.classifyValue`
- 目标：
  - 在完成 `P0-T02R` 之前，把 review 已确认的剩余 stable-id 敏感行为测试从“依赖当前 private / descriptor / callable symbol 拼写”迁移到“依赖语义、family、角色、调用关系、布局与 IR 结构”的断言模型。
- 必须实现的内容：
  1. 复核并迁移 `crates/scoopc/src/llvm/tests.rs` 中剩余的 stable-id 敏感行为测试，至少覆盖：
     - `effectful_closure_dynamic_fallback_uses_schema_aware_carrier_adapter`
     - `higher_order_effectful_function_value_uses_schema_aware_carrier_adapter`
     - `refactor_class_ctor_uses_concrete_generic_instance_layout`
     - `object_member_call_uses_gc_managed_singleton_receiver`
     - `enum_single_field_non_scalar_payload_uses_boxed_variant_path`
  2. 复核并迁移 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中剩余的 stable-id 敏感行为测试，至少覆盖：
     - `refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals`
     - `refactor_llvm_value_boxing_transport`
     - `refactor_llvm_enum_payload_transport`
     - `refactor_llvm_closure_env_transport`
     - `refactor_llvm_cross_thread_resume_payload_transport`
     - `refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
     - `refactor_llvm_runtime_type_primitives`
  3. 为上述测试补齐稳定 IR 查询 helper、global/descriptor 角色识别或等价语义断言，使它们可以依赖以下信息定位目标，而不是依赖当前 symbol spelling：
     - source role / published surface（user callable、compiler-private helper、descriptor、singleton slot）
     - 调用关系、函数体结构、payload/layout/GC slot 元数据
     - direct-entry / wrapper / transport thunk 的角色关系
  4. 扩充 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests` 或等价 source inventory，覆盖本轮清理掉的剩余字符串绑定，至少包括上述 closure adapter / descriptor / transport / callable spellings。
- 必须遵从的约束：
  - 不得把断言弱化成“出现某个通用子串即可”；迁移后仍必须验证 helper / descriptor 的 family、角色与结构语义。
  - 不得把 `main`、runtime / native import、`@Extern` 指定的 native symbol 这类显式例外误纳入需要清理的 callable symbol 绑定。
  - 要按同根问题成组处理，不能只修 review 提到的一两个样例而留下 sibling case。
- 验证：
  1. `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  2. `cargo test -p scoopc closure_step_adapter -- --nocapture`
  3. `cargo test -p scoopc refactor_class_ctor_uses_concrete_generic_instance_layout -- --nocapture`
  4. `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  5. `cargo test -p scoopc enum_single_field_non_scalar_payload_uses_boxed_variant_path -- --nocapture`
  6. `cargo test -p scoopc composite_transport -- --nocapture`
  7. `cargo test -p scoopc closure_env_transport -- --nocapture`
  8. `cargo test -p scoopc cross_thread_resume_payload_transport -- --nocapture`
  9. `cargo test -p scoopc runtime_type_primitives -- --nocapture`
  10. `cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry -- --nocapture`
  11. `cargo test -p scoopc`
- 完成条件：
  - `llvm/tests.rs` 与 `pipeline/llvm_codegen_stage.rs` 中 review 已确认的剩余 stable-id 敏感行为测试，不再直接把当前 private / descriptor / callable symbol 文本当作正确性标准。
- 依赖：P0-T02B
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - 核心决策：
    - 在 `llvm/tests.rs` 补充了少量 IR 解析 helper（store/global symbol 抽取、按 symbol 回查函数体、GC slot load 识别），把 `effectful_closure_dynamic_fallback_uses_schema_aware_carrier_adapter`、`higher_order_effectful_function_value_uses_schema_aware_carrier_adapter`、`refactor_class_ctor_uses_concrete_generic_instance_layout`、`object_member_call_uses_gc_managed_singleton_receiver`、`enum_single_field_non_scalar_payload_uses_boxed_variant_path` 从“锁死当前 symbol spelling”迁移到“检查 helper family、typed alloc、concrete object/payload type、singleton slot 角色与 payload GEP/materialize 结构”的断言模型。
    - 在 `pipeline/llvm_codegen_stage.rs` 增加 global-definition / symbol-mention helper，并把 `refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals`、`refactor_llvm_value_boxing_transport`、`refactor_llvm_enum_payload_transport`、`refactor_llvm_closure_env_transport`、`refactor_llvm_cross_thread_resume_payload_transport`、`refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`、`refactor_llvm_runtime_type_primitives` 迁移为依赖 descriptor/global 角色、direct-entry / wrapper / thunk 调用关系、typed alloc marker、payload/layout/GC slot 元数据与分支/phi 结构的断言，而不再直接绑定当前 private / descriptor / callable symbol 文本。
    - 扩充 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests`，把本轮清理掉的 closure adapter、class/object/type-desc、transport thunk、`@sample.main`、`@sample.classifyValue` 等剩余硬编码 spelling 固化为防回流清单。
    - 针对验证暴露出的不稳定局部名差异（如 `*_desc_i8`、`enum_boxed_payload_obj_ptr`、object receiver SSA 名），统一收敛到在定向/全量路径下都稳定的结构信号，避免继续把临时 IR 命名细节当作正确性标准。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc closure_step_adapter -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_class_ctor_uses_concrete_generic_instance_layout -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc enum_single_field_non_scalar_payload_uses_boxed_variant_path -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composite_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc closure_env_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc cross_thread_resume_payload_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc runtime_type_primitives -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P0 的“测试基线先解除对剩余当前命名形状的强绑定”要求：review 已确认的剩余 LLVM / pipeline stable-id 敏感行为测试，已不再直接把当前 private / descriptor / callable symbol 文本当作正确性标准。
    - 对应 `STABLE_ID.md` §3.4 / §8.5 / §10：closure adapter、class/object/type descriptor、composite transport、value box、thread resume transport、main wrapper entry、runtime type primitive helper 等 identity surface 已迁移为角色/结构语义断言，并有 source inventory 防止旧 spelling 回流。

### [DONE] P0-T02R：Review 审计脚手架与测试基线，确认后续任务不会被旧字符串绑定卡住

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
- 依赖：P0-T02C
- 完成记录：
  - 改动范围：
    - `TODO.md`
    - 复核并验证（无代码改动）：
      - `crates/scoopc/src/llvm/tests.rs`
      - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
      - `crates/scoopc/src/cone/scoopir/schema.rs`
      - `crates/scoopc/src/cone/pre_specialize.rs`
      - `crates/scoopc/src/cone/visibility.rs`
      - `crates/scoopc/src/cone/annotations.rs`
  - 核心决策：
    - 以 review 任务要求重查 stable-id 审计脚手架与测试基线，不新增实现性改动；只有在发现会阻塞后续 P1-P7 的真实缺口时才插入新前置任务。本轮复核未发现此类 blocker，因此直接闭合 `P0-T02R`。
    - 明确后续阶段的 authoritative 验证入口：
      - `stable_id_audit_*` / `external_symbol_*` 负责 object/symbol/linkage 审计；
      - `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests` 负责旧字符串绑定回流防护；
      - 四个 `*_path_free` JSON 基线测试负责 `.cone` / schema path-stability 与 dense-id 泄漏审计；
      - `cargo test -p scoopc` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 负责全量行为与质量收口。
    - 对 review 重点文件的结论：
      - `llvm/tests.rs` 中与旧 `lambda` / hidden-init / direct-invoke / descriptor 字符串相关的剩余命中，均位于审计 helper、分类器示例或 source inventory 防回流清单；未发现新的 stable-id 敏感行为测试强绑定。
      - `pipeline/llvm_codegen_stage.rs` 中 `P0-T02C` 指向的 `@sample.main`、`@sample.classifyValue`、closure adapter / transport / descriptor 旧字符串绑定已无残留。
      - `schema.rs`、`pre_specialize.rs`、`visibility.rs`、`annotations.rs` 的 dense-id/path 相关命中均位于禁止词清单或内部实现变量，不构成对外 `.cone` / JSON surface 泄漏。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composite_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc runtime_type_primitives -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - grep 审计摘要（来自 `stable_id_audit_grep_inventory_scans_repo_roots`）：
      - `module\.add_function\(.*None\)` 101 命中，`stable_template_symbol_suffix` 7 命中，`source_path.*decl_span` 5 命中。
      - `scoop\.lambda\$[0-9]+` 2 命中、`scoop\.lambda_resume\$[0-9]+` 1 命中、`scoop\.lambda_env\$[0-9]+` 1 命中，均来自审计测试自身的分类器/防回流清单。
      - `__schema[0-9]+`、`__k[0-9]+`、`t[0-9]+__` 均为 0 命中。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P0 review 验收：已确认 P1-P7 可继续依赖现有 object/symbol 审计、source inventory 与 `.cone` / JSON path-free 基线，而不会被旧字符串绑定或 schema churn 噪音主导。
    - 对应 `STABLE_ID.md` §10 / §11 / §12：审计入口、grep 清单与健康 schema 基线均已重新复核并通过，无需在进入 P1 前再插入新的 prerequisite task。

## P1：建立统一 `stable_id` 基础设施

### [DONE] P1-T01：新增共享 `stable_id` 模块，收口 canonical encoder 与 shared hash helper

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
  - 改动范围：
    - `crates/scoopc/src/lib.rs`
    - `crates/scoopc/src/stable_id.rs`
    - `TODO.md`
  - 核心决策：
    - 新增共享 `stable_id` 模块作为单一 authoritative 入口，先集中收口 canonical type/effect encoder、版本化 SHA-256 helper、128-bit/64-bit 截断与 dump label 基础 API；本任务不迁移大批现有调用点，把跨子系统替换留给 `P1-T02` 继续推进。
    - canonical encoder 明确不接受 `TypeStore::display()`、raw `Debug` 或 path/span 作为 type-param canonical 输入；对 `TypeKind::Param` 改为要求调用方显式提供 `StableTypeParamResolver`，缺失时直接报错，避免把当前 `(decl_file, decl_span)` 或 pretty name 重新包装成“稳定”文本。
    - effect row / union 的 canonical 文本在编码时按 canonical term 文本重新排序去重，而不是沿用 `TypeId` 或当前 pretty 顺序，确保不同 intern 顺序下仍得到相同 canonical 主体。
    - 版本化 hash helper 固定使用 `SHA-256` + scope prefix（`abi0:`、`priv0:`、`rtti0:`、`dump0:`）；linker-visible surface 使用前 128 bit lowercase hex，runtime-only 场景保留 64 bit 截断 helper 供后续统一接入。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc canonical_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P1 / `STABLE_ID.md` §8.1、§9 Phase 1：仓库内已存在统一 `stable_id` 基础设施入口，后续 P2-P6 可以直接复用 canonical encoder、shared hash helper 与 dump label 基础 API。
    - 对应 `STABLE_ID.md` §7.1 / §7.2：canonical type/effect text 已覆盖 nominal、builtin ref/value、type param、function、tuple、union、effect row，并固定采用版本化 `SHA-256` 规则；未再让 pretty/path/span 文本承担 canonical 输入责任。

### [DONE] P1-T02：落地 stable key / mangler / label API，并收口仓库内分叉 hash 实现

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
  - 改动范围：
    - `crates/scoopc/src/stable_id.rs`
    - `crates/scoopc/src/frontend.rs`
    - `crates/scoopc/src/hir/mod.rs`
    - `crates/scoopc/src/hir/lower/mod.rs`
    - `crates/scoopc/src/hir/lower/util.rs`
    - `crates/scoopc/src/hir/lower/patterns.rs`
    - `crates/scoopc/src/mir/mod.rs`
    - `crates/scoopc/src/mir/materialize.rs`
    - `crates/scoopc/src/rtti/mod.rs`
    - `crates/scoopc/src/rtti/type_desc.rs`
    - `crates/scoopc/src/itable.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
    - `crates/scoopc/src/cone/pre_specialize.rs`
    - `crates/scoopc/src/cone/scoopir/export.rs`
    - `crates/scoopc/src/cone/scoopir/tests.rs`
    - `crates/scoopc/src/cone/archive.rs`
    - `TODO.md`
  - 核心决策：
    - 在 `stable_id.rs` 中补齐统一 authoritative API：`StableCanonicalKey` / `StableSymbolKey` trait、`StableConeKey`、`StableDefKey`、`StableTemplateKey`、`StableInstanceKey`、`StableClosureKey`、`StableCallSiteKey`、`StableEffectSchemaKey`、`StableContinuationSchemaKey`、`StableBoundaryKey`、`StableStateKey`、`StableFrameSlotKey`、`AbiMangler`、`PrivateSymbolMangler`、`stable_local_label`，并新增 shared `stable_template_symbol_suffix` helper。
    - `StableConeKey` 现在显式来源于 `Cone.toml` 的 `name/version`；build/frontend、`.cone` export、pre-specialize 等 manifest-aware 路径会传递真实 cone key，单文件 dump / 测试 helper 则使用“文件 stem + 0.0.0”的虚拟 cone key，避免再回退到 `ConeId` 或 checkout path。
    - `StableTemplateKey` / `StableInstanceKey` 明确脱离 `TemplateKey { fqn, source_path, decl_span }` 与 `TypeId`：overload suffix 改为哈希 shared stable template key，实例 stable key 则基于 canonical type/effect text 构造；`TemplateKey` / `InstanceKey` 注释也改成“仅内部实现键”。
    - RTTI / itable / LLVM codegen 里的分叉 `stable_hash64` 已全部删除，统一改走 `stable_hash64(StableHashScope::RttiV0, ...)`；`hir/lower/patterns.rs` 的 top-level pattern synthetic name 也改走 shared private hash helper，避免 `crates/scoopc/src` 内继续残留 ad hoc `Sha256::digest` 稳定性逻辑。
    - HIR generic template suffix 与 MIR materialization 的 overload-aware suffix 现已统一复用 shared `stable_template_symbol_suffix`；与此对应的 manifest-aware call chain 也已打通到 `frontend`、`mir::materialize`、`cone::pre_specialize`、`cone::scoopir::export`。
    - `.cone` 的 `SOURCES_SHA256` 继续保持原有内容哈希语义，但内部改为流式 hasher，避免与 stable-id 审计 grep 混淆。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc canonical_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 精确搜索摘要（`crates/scoopc/src`）：
      - `fn stable_hash64`：仅剩 `crates/scoopc/src/stable_id.rs`
      - `Sha256::digest`：0 命中
      - `stable_template_symbol_suffix`：仅剩 shared helper、本轮两个调用点（`hir/lower/util.rs`、`mir/materialize.rs`）以及 stable-id 审计测试引用
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P1：仓库内已经存在后续 P2-P6 可复用的唯一 stable key / hash / mangler / local-label API，后续命名与 linkage 任务不必再自带一套规则。
    - 对应 `STABLE_ID.md` §6 / §7.3 / §8.1：`StableConeKey` 已脱离 `ConeId`，`StableTemplateKey` / `StableInstanceKey` 已脱离 path/span 与 `TypeId` exported identity，ABI/private symbol 模式固定为 `__scoop_abi0_*__h<hash128>` / `__scoop_priv0__<role>__h<hash128>`，且 shared hash helper 已覆盖此前分叉实现与 overload suffix 来源。

### [DONE] P1-T02R：Review `stable_id` 基础设施，确认后续阶段已有唯一 authoritative API

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
  - 改动范围：
    - `TODO.md`
  - 核心决策：
    - 本任务按 review 范围复核了 `crates/scoopc/src/stable_id.rs`、`lib.rs`、`rtti/mod.rs`、`rtti/type_desc.rs`、`llvm/codegen/mod.rs`、`itable.rs`，并补查了 `hir/lower/util.rs` 与 `mir/materialize.rs` 的 stable key 构造路径；结论是 `stable_id.rs` 已经是后续 P2-P6 唯一可复用的 key/hash/mangler/label authoritative API。
    - `stable_id.rs` 当前已集中提供 `StableCanonicalKey` / `StableSymbolKey`、`StableConeKey`、`StableDefKey`、`StableTemplateKey`、`StableInstanceKey`、`StableClosureKey`、`StableCallSiteKey`、`StableEffectSchemaKey`、`StableContinuationSchemaKey`、`StableBoundaryKey`、`StableStateKey`、`StableFrameSlotKey`、`AbiMangler`、`PrivateSymbolMangler`、`stable_hash64` / `stable_hash128_hex` / `stable_digest`、`stable_local_label` / `stable_dump_label`；`lib.rs` 也已公开 `pub mod stable_id;`，后续阶段无需再自带第二套 API。
    - `StableConeKey` 的 production 路径已经显式走 manifest：`frontend`、`.cone` export、`pre_specialize` 等入口都使用 `StableConeKey::from_manifest(...)`；`hir/lower/util.rs` 与 `mir/materialize.rs` 中构造 `StableTemplateKey` 的逻辑只再依赖 `stable_cone_key + fqn + namespace + declaration_kind + signature_key`，未把 `ConeId`、`source_path`、`decl_span` 或 `TypeId` 回灌进 exported identity。
    - 剩余允许保留的旧结构与职责边界如下：
      - `mir/materialize.rs` 中的 `TemplateKey` / `InstanceKey` 继续只作为 materialization 内部查找键，可保留 `source_path`、`decl_span` 与 `TypeId`；它们不再承担 exported identity，真正对外 key 已由 `StableTemplateKey` / `StableInstanceKey` 预留给后续 P4 接入。
      - `StableConeKey::for_virtual_source_path(...)` 仅保留给单文件 dump、测试 helper 与 manifest-less 虚拟源路径；它不属于 build/frontend 的 production cone identity 来源。
      - `rtti/mod.rs`、`rtti/type_desc.rs`、`itable.rs`、`llvm/codegen/mod.rs` 已统一通过 `stable_hash64(StableHashScope::RttiV0, ...)` 接入 shared hash helper；其中 `rtti/type_desc.rs` 的 closure env canonical name / `type_id` 仍暂时使用 `ClosureId` 形状 `scoop.lambda_env$N`，这是 `P6-T01` 明确要收尾的剩余旧结构，不构成当前 review 的新前置阻塞。
      - `crates/scoopc/src/cone/archive.rs` 仍保留 `SOURCES_SHA256` 的内容摘要实现；该路径只负责归档内容 fingerprint，不参与 stable-id 的 key/hash/mangling 协议，因此允许继续独立使用 SHA-256。
      - `AbiMangler` / `PrivateSymbolMangler` 已经是唯一 authoritative 命名 API，但实际 LLVM 命名调用点迁移仍按计划留在 P2-P4，不在本 review 中提前改动。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc canonical_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 精确搜索摘要（`crates/scoopc/src`）：
      - `fn stable_hash64`：仅剩 `crates/scoopc/src/stable_id.rs`
      - `Sha256::digest`：0 命中
      - `stable_template_symbol_suffix`：仅剩 shared helper、本轮两个调用模块（`hir/lower/util.rs`、`mir/materialize.rs`）以及 stable-id 审计/单元测试引用
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P1 review 验收：已确认后续 P2-P6 需要的新 identity 入口均已在 `stable_id` 模块中集中定义；继续推进 linkage、private naming、ABI naming、dump/RTTI 收口时，不需要再新建第二套 key/hash/mangler 规则。
    - 对应 `STABLE_ID.md` §6 / §7 / §8.1：`StableConeKey` 已明确区分 manifest-aware production 来源与 virtual-source 测试来源，`StableDefKey` / `StableTemplateKey` / `StableInstanceKey` 的语义边界已与 `ConeId`、path/span、`TypeId` 脱钩；剩余 closure env RTTI 旧输入边界也已明确记账到后续 P6，而不是继续模糊地留在 shared API 之外。

## P2：收紧 linkage，先处理 external namespace 污染

### [DONE] P2-T01：分类 `module.add_function(..., None)` 调用点并建立统一 declaration/linkage helper

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
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
    - `crates/scoopc/src/llvm/codegen/object_init.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
    - `crates/scoopc/src/llvm/emit.rs`
    - `crates/scoopc/src/llvm/tests.rs`
  - 核心决策：
    - 在 `codegen/mod.rs` 新增统一 declaration/linkage helper：显式区分 `ExportedAbi`、`RuntimeOrNativeImport`、`CompilerPrivateHelper` 三类 surface，并在 helper 注释里固定 `main`、`malloc`、`exit`、runtime ABI entry、`@Extern` symbol 这些显式 external 例外。
    - 把 source-level top-level callable、materialized plain callable、host `main` 收口到 exported ABI path；把 `runtime_abi.rs`、`scoop_runtime_init`、`scoop_entry_argv_array`、`malloc`、`exit`、`@Extern` 声明收口到 runtime/native import path。
    - 把 closure body / callee resume、effect helper shell/surface resume/outcome/transport thunk、object/top-level init bridge 与 init function、materialized closure body 等收口到 compiler-private helper path；本任务先把它们的“角色”和“linkage 决策入口”显式化，仍保留当前 `External` 以避免提前越过 `P2-T02` 的 internal/private 收口任务。
    - 在 `llvm/tests.rs` 增加两类定向测试：一类直接验证 helper 可显式生成 external/internal/private linkage；一类对 `crates/scoopc/src/llvm` 做 source inventory，阻止 raw `module.add_function(..., None)`（含多行形式）回流。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc function_declaration_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - `rg -n "module\.add_function\(.*None\)" crates/scoopc/src/llvm` 返回 0 命中；`function_declaration_inventory_eliminates_raw_add_function_none_callsites` 进一步覆盖了多行 `add_function(..., None)` 回流检查。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P2 的第一步：LLVM function declaration 已先按 exported ABI / runtime-native import / compiler-private helper 三类统一收口，后续 `P2-T02` 可直接在 compiler-private helper path 上做 internal/private internalize，而不再由各调用点自行决定。
    - 对应 `STABLE_ID.md` §3.4.1 / §3.4.4 / §3.4.5 / §7.4 / §8.6：顶层 callable、effect helper、object/top-level init bridge、closure/materialized helper 的 declaration surface 已显式建模；runtime import 与 fixed external 例外也不再混入“默认 `None`”隐式路径。

### [DONE] P2-T02：把 compiler-private helper 从 external namespace 收回 internal/private

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
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/object_init.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `TODO.md`
  - 核心决策：
    - 把 production 代码里所有仍以 `CompilerPrivateHelper + Linkage::External` 声明的 helper，统一收口到显式 `Linkage::Internal`，覆盖 object init bridge/init function、top-level immutable init bridge/init function、callee resume entry、materialized MIR closure/plain helper、closure body、effect helper shell/trampoline/outcome/owner-core、thread resume thunk、task transport resume 等路径；不改 `main`、runtime/native import、`@Extern` 这类显式 external 例外。
    - 把 `effect_lowered/body.rs` 与 `effect_lowered/value.rs` 中残留的裸 `module.add_function(..., None)` helper 声明全部改走统一 declaration helper，确保 compiler-private function 不再绕过 linkage 分类入口；复查后 `crates/scoopc/src/llvm/codegen` 里仅剩 `declare_classified_llvm_function(...)` 内部的 `module.add_function(name, fn_ty, Some(linkage))`。
    - 同步收紧 source-level / materialized plain callable 的模块内实现体：在当前 `minimal main` 产物里，`a.helper`、`a.id`、`a.entry` 这类仅模块内消费的实现体不再泄漏到 object external symbol 集，而宿主固定入口 `main` 与 runtime/native import 继续保留 external surface。
    - 更新 `llvm/tests.rs` 的 object/IR 审计模型：`external_symbol_audit_*` 现在直接断言 closure/effect/hidden-init helper 与模块内 source-level/materialized callable 不再进入 external symbol 集，并额外用 IR 检查这些 helper/实现体仍存在且使用 `internal/private` linkage。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc function_declaration_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - `rg -n "add_function\(" crates/scoopc/src/llvm/codegen` 仅剩 `crates/scoopc/src/llvm/codegen/mod.rs:531`（统一 declaration helper 内部的 `Some(linkage)` 调用）
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P2 的 linkage 卫生目标：compiler-private helper 已不再以 external linkage 污染 object / linker namespace，后续 P3 只需继续迁移 private naming source，而不再冒 linker 冲突风险。
    - 对应 `STABLE_ID.md` §7.4 / §8.5 / §8.6：closure/effect helper、object/top-level init bridge、thread/task resume thunk、模块内 source/materialized callable 实现体现在都显式使用 `InternalLinkage`，`main`/runtime/native import 例外保持 external，且 helper 声明不再绕开统一 declaration/linkage 分类入口。

### [DONE] P2-T02R：Review linkage 收口，确认 namespace 风险已经先于命名迁移被压住

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
  - 改动范围：
    - `TODO.md`
    - 复核并验证（无代码改动）：
      - `crates/scoopc/src/llvm/codegen/mod.rs`
      - `crates/scoopc/src/llvm/codegen/object_init.rs`
      - `crates/scoopc/src/llvm/codegen/mir_body.rs`
      - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
      - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
      - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
      - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
      - `crates/scoopc/src/llvm/tests.rs`
  - 核心决策：
    - 本任务按 review 范围复核 `P2-T01` / `P2-T02` 的 declaration/linkage 收口结果，不新增实现性改动；只有在发现会阻塞后续 P3 private naming 迁移的真实 namespace/ABI 问题时才插入前置任务。本轮未发现此类 blocker，因此直接闭合 `P2-T02R`。
    - 复核结论确认：compiler-private helper 已经退出 object external symbol 集，`external_symbol_audit_*` 继续能在 object 层直接看见 `main` / runtime-native import / user ABI surface，同时确认 closure/effect/hidden-init helper 与仅模块内使用的 source-level/materialized callable 实现体保持 `internal/private` linkage。
    - 明确仍允许 external 的固定例外清单：
      - 宿主固定入口 `main`
      - runtime/native import（含 `scoop_runtime_init`、`scoop_entry_argv_array`、LLVM intrinsic、`malloc` / `free` / `exit` 等）
      - `@Extern` 指定的 native function/global symbol（含 thread-local extern global）
    - 除上述固定例外外，真正用户可见的 exported ABI callable 仍归入 `ExportedAbi`/user-ABI surface；未发现 compiler-private helper 伪装成 external user ABI 的残留路径。
    - 额外静态复核结果：production 代码中未发现新的 `CompilerPrivateHelper + Linkage::External` 组合；`module.add_function(..., None)` 在 `crates/scoopc/src/llvm` 中未回流到实现代码，当前命中仅剩测试断言文本。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc function_declaration_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_extern_global -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 静态审计摘要：
      - `CompilerPrivateHelper + Linkage::External`：0 个 production 命中
      - `module\.add_function\(.*None\)`：`crates/scoopc/src/llvm` 中仅剩 `tests.rs` 的断言文本命中
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P2 review 验收：已确认 namespace 风险先于 P3/P4 命名迁移被压住，后续即使 private/user ABI symbol 文本继续演进，也不会再因 compiler-private helper external 化而把主要 linker 风险重新带回 object/linker surface。
    - 对应 `STABLE_ID.md` §7.4 / §8.6：LLVM function declaration 现已稳定区分 exported ABI、runtime/native import、compiler-private helper 三类 surface；`main` / `@Extern` / runtime 固定入口保持显式 external 例外，而 closure/effect/init helper 继续停留在 internal/private linkage 路径。

## P3：迁移 private LLVM naming source

### [DONE] P3-T01：用 `StableClosureKey` 替换 closure private naming，并清理旧 alias 兼容层

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
  - 改动范围：
    - `crates/scoopc/src/hir/lower/mod.rs`
    - `crates/scoopc/src/hir/lower/types.rs`
    - `crates/scoopc/src/llvm/emit.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
    - `crates/scoopc/src/llvm/codegen/ordinary_callee.rs`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/codegen/object_init.rs`
    - `crates/scoopc/src/stable_id.rs`
    - `crates/scoopc/src/llvm/tests.rs`
  - 核心决策：
    - 把 `LoweredHir.stable_cone_key` 贯穿到 LLVM codegen 输入，避免 backend 为 stable-id 继续从 path/span 临时猜 cone identity；closure stable owner key 现在直接基于当前 cone 的 authoritative semantic key 构造。
    - direct-HIR closure 路径新增 root/nested identity 跟踪：顶层函数、object init、top-level init 进入 codegen 时建立 `StableDefKey`，嵌套 closure 复用同一 owner key 加 `$lambdaN...` 词法路径；closure body / resume / env / env type-desc 统一改走 `PrivateSymbolMangler`。
    - 为 materialized MIR / effect-lowered closure 增加一条共享 lexical-path 恢复路径：从 owner HIR body 按词法顺序重建 closure 的 `$lambdaN.$lambdaM` 路径，再与 owner 的 `StableDefKey` / `StableInstanceKey` 组装成同一份 `StableClosureKey`。这样 materialized-MIR closure body/env/type-desc 与 effect-lowered closure env transport 也不再依赖 `ClosureId`、`scoop.mir.lambda_env$...` 或 `__scoop_type_desc_mir_closure_env__...`。
    - 删除 `direct_hir_closure_carrier_alias` / `is_direct_hir_closure_carrier_alias` 兼容层，callable carrier contract 不再把 direct HIR closure 重新映射回 `scoop.lambda$<n>` 旧族名。
    - 保持 ordinary callee / pass-MIR fallback 的行为语义不变：materialized-MIR closure body symbol 虽已切到 stable private name，但 fallback 获取函数指针时继续优先复用已声明 symbol，仅在确实缺符号时才触发 body 定义，避免把既有不支持的 pass-MIR body 形态错误地提前 materialize 成新回归。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialized_mir_closure_private_symbols_use_stable_hash_namespaces -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc closure_call_without_outward_effect_stays_on_direct_call_surface -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_closure_env_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 生产级 `crates/scoopc/src/llvm/codegen` 精确 grep：`scoop\.lambda\$[0-9]+`、`scoop\.lambda_resume\$[0-9]+`、`scoop\.lambda_env\$[0-9]+`、`scoop\.mir\.lambda_env\$`、`__scoop_type_desc_mir_closure_env__` 均为 0 命中。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P3 / `STABLE_ID.md` §6.5、§8.5：closure private body/resume/env/type-desc 现在统一以 `StableClosureKey` 为 authoritative identity，并由 `PrivateSymbolMangler` 生成 private LLVM naming。
    - 对应 `PLAN.md` P3 的 alias 清理目标与 `STABLE_ID.md` §3.4.3：旧 `scoop.lambda$<n>` / `scoop.lambda_resume$<n>` / `scoop.lambda_env$<n>` 以及 direct-HIR closure carrier alias 已退出 production LLVM codegen 路径；materialized-MIR / effect-lowered closure env 也不再残留 `scoop.mir.lambda_env$...` / `__scoop_type_desc_mir_closure_env__...` 旧族名。

### [DONE] P3-T02：用 stable schema key / canonical type hash 替换 effect helper 与 transport type 的 private naming

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
  - 改动范围：
    - `crates/scoopc/src/stable_id.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/mod.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/stable_naming.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/types.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `TODO.md`
  - 核心决策：
    - 新增 `effect_lowered::stable_naming` 作为本阶段 private effect naming 的集中入口：它复用共享 `stable_id` canonical encoder / `PrivateSymbolMangler`，构造 callable version key、`StableEffectSchemaKey`、`StableContinuationSchemaKey` 与 transport canonical type key，避免 `layout.rs` / `body.rs` / `value.rs` 各自再拼 ad hoc hash/字符串。
    - 为了让 `body.rs` / `value.rs` 在缺少完整 semantic 上下文时仍能用 authoritative identity 生成 helper 名字，给 `RefactorStepLayout`、`RefactorCallableLayout`、`RefactorPlainCallableLayout`、`RefactorContinuationSurfaceResumeLayout`、`RefactorContinuationSurfaceResumeOwnerTrampolineLayout` 增加 stable key text；后续同家族 helper 统一基于这些 key text + private role 继续 mangling。
    - 任务范围按“同根问题成组修复”扩展到 review 文本未逐项列出的 sibling case：除条目正文点名的 `resume` / `surface_resume` / `dynamic_invoke` / `direct_invoke` / `owner_dispatch` / `task_transport_resume` / effect transport box 外，也一并迁移了 `plain_adapter`、`closure_step_adapter`、`thread_resume_u64`、`thread_resume_transport`，确保 production LLVM effect helper naming 不再残留 `StepSchemaId` / `ContinuationSchemaId` / `TypeId` 控制路径。
    - 新 private role 继续保留语义家族前缀（如 `refactor_direct_invoke`、`refactor_surface_resume_owner__core`、`refactor_thread_resume_transport`），但唯一性主体只来自 stable schema key / canonical type text hash，不再依赖 `__schemaN` / `kN` / `t<TypeId>__pretty` 旧拼写。
    - 同步迁移 3 条 LLVM 测试定位逻辑：effect helper 现在按 private role、step type 与 helper family 定位，不再假设 effect helper symbol 仍包含 FQN 或 `ContinuationSchemaId` 文本。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc surface_resume -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composed_continuation_resume_publishes_internal_outcome_surface_and_owner_core -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 精确 grep（`crates/scoopc/src/llvm/codegen`）：`__schema[0-9]+` / `__k[0-9]+` / `t[0-9]+__` 均为 0 命中。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P3：closure 之外剩余的 effect helper / continuation helper / transport type private naming 已统一迁移到 stable schema key / canonical type hash，不再受 `StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`TypeId` 或 pretty type text 控制。
    - 对应 `STABLE_ID.md` §3.4.4 / §3.4.6 / §6.7 / §8.5：`resume`、`surface_resume`、`dynamic/direct invoke`、owner dispatch、task/thread transport thunk、plain/closure adapter、effect transport box/type 等 production effect lowering helper 已改走 `PrivateSymbolMangler` + stable key；旧 `__schemaN` / `kN` / `t<TypeId>__...` spelling 已退出 production LLVM naming 路径。

### [DONE] P3-T02R：Review private naming 迁移，确认 dense id 已退出 LLVM private symbol 控制路径

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
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_lowered/stable_naming.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoop/src/commands/build.rs`
    - `TODO.md`
  - 核心决策：
    - review 过程中先发现 `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs` 仍通过 `ProgramLayoutView::step_stem()` 把 `StepSchemaId` 退回到 `__schemaN` / `schemaN` 命名 stem，并进一步驱动 step/storage/case/vtable/frame/continuation 的 LLVM type/global 名字；这与 `P3-T02R` 完成条件直接冲突，因此先修复这一阻塞再继续 review。
    - 在 `effect_lowered::stable_naming` 新增复用 `PrivateSymbolMangler` hash 主体的 private type-name helper；step/storage/complete/case/vtable/frame/continuation 的 LLVM type 名和内部 anchor/global 名现在都基于 stable effect key、stable callable version key 或 per-case stable key 生成，不再依赖 `StepSchemaId`、`CaseTag` 或旧 readable stem 回退。
    - 新增 `stable_id_source_inventory_removes_legacy_effect_private_naming_fallbacks_from_codegen`，把 `step_stem` / `schemaN` / `caseN` / `{stem}` 旧模板拼接固化为防回流检查；同时把相关 LLVM / build 行为测试迁移到“hashed private family + IR 结构/调用关系”断言模型，避免继续锁死旧 private/type spelling。
    - 明确 review 后仍允许保留数字、但不属于 authoritative private naming 的场景：
      - LLVM basic block label（如 `mir.bb0`、`plain.bb0`）
      - SSA/局部临时名与显式 root slot 序号（如 `%tmp42`、`%explicit_root_frame_slot_15`）
      - 语义性数字常量与索引（如 runtime tag 值、`extractvalue`/GEP field index、layout offset/alignment/size 常量）
      - 这些数字不再决定 linker-visible/private symbol、private type name 或 compiler-private global name 的 authoritative identity。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_ -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc effect_contract_struct_types_are_registered_for_effect_codegen -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_no_outward_plain_abi_layout_has_no_step_shell -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop build_emit_llvm_dynamic_entry_publication_keeps_plain_carrier_targets_buildable -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialized_mir_closure_private_symbols_use_stable_hash_namespaces -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composed_continuation_resume_publishes_internal_outcome_surface_and_owner_core -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoop --all-targets -- -D warnings`
    - 生产级 `crates/scoopc/src/llvm/codegen` 精确 grep：`scoop\.lambda\$[0-9]+`、`scoop\.lambda_resume\$[0-9]+`、`scoop\.lambda_env\$[0-9]+`、`scoop\.mir\.lambda_env\$`、`__scoop_type_desc_mir_closure_env__`、`__schema[0-9]+`、`__k[0-9]+`、`t[0-9]+__` 均为 0 命中。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P3 的 review 目标：closure/effect/transport private naming 的剩余 dense-id 控制路径已复核并收口；private LLVM type/global/function 命名继续与 P2 的 internal/private linkage 保持一致。
    - 对应 `STABLE_ID.md` §5.1、§6.5-§6.8、§8.5：`ClosureId`、`StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`TypeId` 已退出 private LLVM symbol/type/global authoritative naming path；剩余数字仅保留在 IR-local 或语义性 ordinal/offset 场景，不再承担外部 identity 责任。

## P4：迁移 exported ABI naming

### [DONE] P4-T01：重写 overload / template / instance 的 exported identity 来源

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
  - 改动范围：
    - `crates/scoopc/src/mir/materialize.rs`
    - `crates/scoopc/src/effect_facts/schema.rs`
    - `crates/scoopc/src/effect_facts/builder.rs`
    - `crates/scoopc/src/effect_lowered/ir.rs`
    - `crates/scoopc/src/effect_lowered/builder.rs`
    - `crates/scoopc/src/effect_lowered/opt.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/stable_naming.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
  - 核心决策：
    - 保留 `TemplateKey { fqn, source_path, decl_span }` / `InstanceKey { template, type_args, eff_args }` 作为内部 materialization handle，但新增并持久化 authoritative stable identity side table：`MaterializedMir` 现在保存 `StableInstanceKey`、`StableTemplateKey` 与 non-generic callable signature fallback，供后续 exported naming / effect helper naming 直接消费，不再现场按 `template.fqn`、pretty text 或 path/span 重建。
    - `stable_template_symbol_suffix()` 的实际输入继续统一为 `StableTemplateKey`；同时把 overloaded generic / non-generic callable 的 exported identity 收口到 `StableDefKey + canonical signature key`，并让 `MaterializedMir::instance_exported_fun_symbol(...)` 显式走 `StableInstanceKey + AbiMangler`。
    - late-lowered callable 与 `ConcreteOpKey` 现在都显式携带 authoritative `StableInstanceKey`；LLVM materialized-closure stable key 与 effect-lowered stable naming 改为消费这些 authoritative key，避免同名 overload 的同型实例在 downstream private/exported naming 中坍缩到同一个 key。
    - `stable_instance_fqn` / `monomorph_instance_fqn` 仍保留为 display-only 路径；grep 审计中的剩余命中仅对应 dump/debug 或 pre-specialize 文本，不再控制 exported identity。
    - `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct` 按新语义更新为断言 receiver overload target 维持 distinct overload-aware symbol，而不是把两个合法 overload 误压成单目标假设。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialized_overloaded_generic_instances_publish_distinct_path_stable_exported_symbols -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc callable_version_key_text_distinguishes_overloaded_instances_with_same_type_args -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_continuation_layout_uses_codegen_owned_fields -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_backend_gate_smoke_lowers_effectful_handle_body_without_legacy -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - grep 审计摘要：
      - `stable_template_symbol_suffix` 只剩 shared helper、stable call site 与测试审计命中；无 path/span 版旧实现回流。
      - `source_path.*decl_span` 剩余命中仅在 `TemplateKey` 说明注释、stable-id grep 审计测试，以及 `hir/lower/types.rs` 的内部 local symbol intern；不再进入 exported naming 路径。
      - `instance_fqn\(` 剩余命中只对应 display-only `stable_instance_fqn` / `monomorph_instance_fqn` helper，与 exported symbol 生成解耦。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P4 的第一阶段目标：overload suffix、template identity、instance exported symbol 来源现在都已收口到 authoritative stable key / canonical signature / `AbiMangler` 预备路径，后续 P4-T02 只需把 declaration path 全量接入统一 mangler。
    - 对应 `STABLE_ID.md` §3.4.2、§6.2-§6.4、§8.3：overload / template / instance identity 已脱离 `source_path + decl_span`、pretty type text 与 `TypeId`；同名 overload 的同型实例在 materialize、late-lowering 与 LLVM naming 下都保持 distinct 且 path-stable。

### [DONE] P4-T02：把 `AbiMangler` 接入 exported declaration path，并验证跨路径稳定性

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
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
    - `crates/scoop/src/commands/build.rs`
    - `crates/scoop/src/fixtures/mod.rs`
    - `crates/scoop/tests/p7_default_pipeline.rs`
    - `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
    - `tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`
    - `tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
    - `tests/fixtures/build/effect_refactor_step_enum_no_outward.scoop`
    - `tests/fixtures/build/member_call_devirt_final_receiver_direct_call.scoop`
  - 核心决策：
    - 在 `MainCodegen` 中集中新增 authoritative exported declaration path：source-level HIR callable 与 materialized/plain callable 统一通过 `exported_abi_symbol_for_hir_fun(...)` / `exported_abi_symbol_for_materialized_fun(...)` 生成 user ABI symbol；优先消费 `authoritative StableInstanceKey + AbiMangler`，否则退回 `StableDefKey { StableConeKey, canonical callable signature key } + AbiMangler`。`main`、`@Extern` 指定 symbol 与 runtime/native fixed entry 继续保留显式例外。
    - `declare_top_level_fun*`、`declare_materialized_top_level_fun_with_symbol(...)`、`declare_materialized_mir_plain_fun_with_symbol(...)` 与 effect-lowered plain callable layout 现在都把 `ExportedAbi` surface 接到同一套 authoritative symbol 生成路径上，不再把 raw callable `fqn` 直接作为 linker-visible exported symbol。
    - 新增 exported ABI symbol reservation registry：若 HIR/MIR/effect-lowered 多条 declaration path 试图把不同 canonical key 注册到同一个 exported symbol，会在 codegen 期显式报 collision，而不是静默复用或继续生成冲突 object symbol。
    - 新增 object external symbol audit helper 与路径稳定性/virtual-cone collision smoke，验证同源程序跨 checkout 根路径 external symbol 集一致，不同 virtual cone 的 user ABI symbol 保持分离；同时把 `scoop` 侧 build/pipeline 回归与 fixture 断言迁移到 `__scoop_abi0_fun__*` / `__scoop_priv0__*` namespace 语义，不再锁死旧 raw callable/private spelling。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_overloaded_source_level_callables_publish_distinct_abi_symbols -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_vtable_targets_use_abi_mangler_namespace -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_exported_object_symbols_are_path_stable_across_checkout_roots -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_user_abi_symbols_stay_disjoint_for_distinct_virtual_cones -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc via_mir_direct_interface_default_call_is_not_reinterpreted_as_itable_dispatch -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_top_level_and_materialized_generic_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop commands::build:: -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop --test p7_default_pipeline -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P4 的第二阶段目标：source-level top-level function、materialized plain callable 与 effect-lowered plain callable 的 exported declaration path 现已统一走 `AbiMangler`；同一输入跨 checkout 根路径 external symbol 集一致，overload / multi-cone collision smoke 也证明 exported namespace 已进入 cone-aware、path-stable 语义。
    - 对应 `STABLE_ID.md` §5.2、§7.3、§7.4、§8.5：user ABI exported symbol 现统一落在 `__scoop_abi0_fun__<escaped-fqn>__h<hash128>` namespace，compiler-private refactor/closure/layout helper 继续留在 `__scoop_priv0__...`；`main`、`@Extern` 与 runtime/native fixed entry 仍是显式例外；不同 canonical key 复用同一 exported symbol 时会显式失败，不再静默碰撞。

### [DONE] P4-T02R：Review exported ABI naming，确认 exported 与 private namespace 已完全分家

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
  - 改动范围：
    - `TODO.md`
    - 复核并验证（无代码改动）：
      - `crates/scoopc/src/stable_id.rs`
      - `crates/scoopc/src/llvm/codegen/mod.rs`
      - `crates/scoopc/src/llvm/codegen/mir_body.rs`
      - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
      - `crates/scoopc/src/llvm/tests.rs`
      - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - 核心决策：
    - 本轮按 review 任务要求只做定向复核，不新增实现性代码。复核结论是：authoritative exported declaration path 已闭合到 `AbiMangler`，且未发现需要插入到 `P5` 之前的新 blocker，因此直接闭合 `P4-T02R`。
    - exported 与 private namespace 的最终分类规则明确如下：
      - user ABI exported symbol：统一由 `AbiMangler` 生成，命名空间为 `__scoop_abi0_{fun|global|type}__<readable>__h<hash128>`，并通过 `declare_exported_abi_function(...)` 保持 `External` linkage。
      - fixed external exception / runtime-native import：`main` 保留宿主固定入口；`@Extern` 指定 symbol、runtime/native 固定入口与 `malloc/free/exit` 继续通过 `declare_runtime_or_native_import_function(...)` 保持 external surface，不接入 private namespace。
      - compiler-private helper：closure/object-init/effect/refactor helper 继续通过 `PrivateSymbolMangler` 与 compiler-private declaration API 发布，命名空间为 `__scoop_priv0__<role>__h<hash128>`，linkage 显式为 `Internal`/`Private`（以及少量设计上要求显式 external 的 helper），不与 exported ABI namespace 混用。
    - `fqn` 仍保留源级语义：`exported_abi_symbol_for_hir_fun(...)` / `exported_abi_symbol_for_materialized_fun(...)` 会把 raw `fqn` 映射到 stable exported symbol，但 `mir_fun.fqn`、`callable.root_fqn()`、`StableDefKey.readable_path()` 仍只承担源级身份、查询、display/审计角色，没有被 whole-sale 改写成 mangled symbol。
    - object audit、IR 行为测试与 source inventory 共同证明 exported/private 已分家：external symbol 集里只保留 user ABI 与显式例外；closure/effect/object-init 等 helper 继续留在 internal/private surface；旧 raw callable/private spelling 也有常驻 inventory 防止回流。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_top_level_and_materialized_generic_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_extern_global -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc function_declaration_helpers_emit_explicit_linkage -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_overloaded_source_level_callables_publish_distinct_abi_symbols -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_vtable_targets_use_abi_mangler_namespace -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_exported_object_symbols_are_path_stable_across_checkout_roots -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_user_abi_symbols_stay_disjoint_for_distinct_virtual_cones -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P4 的 review 验收项：可明确写出 exported ABI naming 已与 compiler-private helper naming 完全分家，且跨 checkout 路径稳定性、multi-cone collision 与全量 `scoopc` 测试均已通过。
    - 对应 `STABLE_ID.md` §5.2 / §7.3 / §10：exported ABI 统一使用 `AbiMangler` namespace；`main`、`@Extern`、runtime/native fixed entry 仍是显式例外；closure/effect/object-init/private helper 继续停留在 private namespace 与 internal/private linkage，未回流到 external symbol 集。

## P5：重写 dump / fixture renderer

### [DONE] P5-T01：重写 HIR / MIR / materialized IR dump renderer，并刷新相关 fixture

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
  - 改动范围：
    - `crates/scoopc/src/dump_support.rs`
    - `crates/scoopc/src/hir/dump.rs`
    - `crates/scoopc/src/mir/dump.rs`
    - `crates/scoopc/src/hir/mod.rs`
    - `crates/scoopc/src/mir/mod.rs`
    - `crates/scoopc/src/mir/materialize.rs`
    - `crates/scoopc/src/pipeline/hir_stage.rs`
    - `crates/scoopc/src/pipeline/mir_stage.rs`
    - `crates/scoop/src/commands/dump_hir.rs`
    - `crates/scoop/src/commands/dump_mir.rs`
    - `crates/scoop/src/commands/dump_ir.rs`
    - `crates/scoop/src/fixtures/mod.rs`
    - `crates/scoopc/src/hir/lower/mod.rs`
    - `crates/scoopc/src/hir/lower/placeholder_inventory.rs`
    - `tests/fixtures/hir/**`
    - `tests/fixtures/mir/**`
    - `tests/fixtures/mir_refactor/**`
  - 核心决策：
    - 新增共享 `dump_support` 与专用 HIR/MIR renderer；`dump-hir`、`dump-mir`、`dump-ir` 都改为显式稳定 renderer，不再依赖 `format!("{:#?}")` 或字符串后处理 canonicalize。
    - HIR renderer 现在用 source-anchor 派生的稳定 label 替代 `SymbolId` / `ClosureId`，并为 `ValDecl` 补了符号声明 span 收集器，保证声明/引用 label 对齐；typed HIR side-table dump 中的 assign-place contract 也移除了 `id: S*` 泄漏。
    - MIR renderer 现在直接输出语义类型文本与稳定 `local` / `bb` / `site` label；workspace-relative path 规范化在 renderer 内完成，`dump-mir` 不再走 `TypeId(...)` canonicalize 或 path replace 补丁链。
    - `dump-ir` 增加 `MaterializedMir::stable_dump()`，把 materialized instance surface 改为稳定 `instance#h...` label、稳定 instance display 与 exported symbol，不再暴露 `InstanceKey` 的 `tN` 视图。
    - fixture runner、HIR golden 单测与 dump 命令测试全部切到 stage-owned `stable_dump()` surface，避免 CLI、fixture、单测各自维护不同文本协议。
  - 验证结果：
    - `cargo test -p scoopc`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor`
    - `cargo test -p scoop`
    - `cargo clippy -p scoopc --all-targets -- -D warnings`
    - `cargo clippy -p scoop --all-targets -- -D warnings`
    - 文本审计：刷新后的 `tests/fixtures/hir/**`、`tests/fixtures/mir/**`、`tests/fixtures/mir_refactor/**` 中，`TypeId(`、`S0`、`C0`、`bb0`、`site0` 均已无命中。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P5 / `STABLE_ID.md` §3.2.1-§3.2.3、§8.2：HIR / MIR / materialized MIR 的对外文本协议已从 raw `Debug`、allocator-derived label 与 `TypeId`/`SymbolId`/`ClosureId` 表面完全迁出。
    - 对应 `PLAN.md` P5 的 fixture 验收：`tests/fixtures/hir/**`、`tests/fixtures/mir/**`、`tests/fixtures/mir_refactor/**` 已一次性刷新到新的稳定协议，后续 P5-T02 可继续处理 effect facts / effect lowered textual surface。

### [DONE] P5-T02：重写 effect facts / effect lowered dump renderer，并刷新相关 snapshot

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
  - 改动范围：
    - `crates/scoopc/src/effect_facts/dump.rs`
    - `crates/scoopc/src/effect_lowered/builder.rs`
    - `crates/scoopc/src/effect_lowered/dump.rs`
    - `crates/scoopc/src/effect_lowered/ir.rs`
    - `crates/scoopc/src/effect_lowered/materialize.rs`
    - `crates/scoopc/src/effect_lowered/opt.rs`
    - `crates/scoopc/src/mir/dump.rs`
    - `crates/scoopc/src/mir/mod.rs`
    - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
    - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
    - `tests/fixtures/effect_facts/*.effectfacts`
    - `tests/fixtures/effect_lowered/*.effectlowered`
  - 核心决策：
    - 复用 MIR dump 现有的 `local` / `bb` / `site` 稳定标签算法，对外暴露 `BodyLabels` / `build_body_labels_for_dump(...)`，避免 effect dumps 再各自维护一套 source-anchor 派生逻辑。
    - `effect_facts` dump 新增上下文型 renderer：`step_schema#N` / `continuation_schema#N` / `case#N` 改成基于 schema 内容、owner root、concrete op 的稳定 `step#h...` / `cont#h...` / `case#h...`，body 内 `bbN` / `siteN` 改成与 MIR 一致的稳定 local labels。
    - `LateLoweredProgram` 增加 dump 元数据承载（类型文本缓存 + body label inventory），由 builder 构建、optimizer 保留；`effect_lowered` dump 因此可以把 `t/s/k/c/ri/ko/st/bd/fs/local/bb/site` 全部迁到语义类型文本与稳定 `*#h...` 标签，而不回退到 dense id。
    - wrapper projection / handle binder / resume boundary / runtime-error boundary 等路径都显式改成用 authoritative schema / publication contract 推导 textual identity，不保留“冲突时退回旧 dense id”的旁路。
    - 对应 textual 变化类别主要包括：
      - `TypeId(tN)` / `tN` -> 语义类型文本
      - `step_schema#N` / `sN` -> `step#h...`
      - `continuation_schema#N` / `kN` -> `cont#h...`
      - `case#N` / `cN` -> `case#h...=<concrete op>`
      - `riN` / `koN` -> `packing#h...` / `cont_obj#h...`
      - `stN` / `bdN` / `fsN` -> `state#h...` / `boundary#h...` / `slot#h...`
      - `localN` / `bbN` / `siteN` -> 稳定 `local#h...` / `bb#h...` / `site#h...`
  - 验证结果：
    - `cargo fmt`
    - `cargo test -p scoopc`
    - `cargo test -p scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
    - `cargo clippy -p scoopc --all-targets -- -D warnings`
    - `cargo clippy -p scoop --all-targets -- -D warnings`
    - 精确文本审计：
      - `tests/fixtures/effect_facts/*.effectfacts` 中 `step_schema#0`、`continuation_schema#0`、`case#0`、`k0`、`ri0`、`ko0`、`st0`、`bd0`、`fs0`、`local0`、`bb0`、`site0` 均无命中。
      - `tests/fixtures/effect_lowered/*.effectlowered` 中同样无上述旧协议命中。
      - `crates/scoopc/src/effect_facts/dump.rs` / `crates/scoopc/src/effect_lowered/dump.rs` 中也已无对应旧 textual protocol 直出。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P5 / `STABLE_ID.md` §3.2.4、§3.2.5、§8.2：effect facts 与 late-lowered active dump/fixture surface 已从 allocator 顺序、raw `Debug` id、`TypeId`/schema dense id 表面迁出。
    - 现在 HIR / MIR / IR / effect facts / effect lowered 五类 active textual surface 都有独立稳定 renderer，P5-T02R 可直接做全面 review，而不是再修 renderer 基础设施。

### [DONE] P5-T02R：Review dump / fixture 迁移，确认所有 textual surface 已与 raw `Debug` 脱钩

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
  - 改动范围：
    - `TODO.md`
  - 核心决策：
    - 本次 review 未引入新的 renderer 改造；P5-T01 / P5-T02 已经把 HIR / MIR / materialized IR / effect facts / effect lowered 五类 active textual surface 全部收口到 stage-owned `stable_dump()` / dedicated renderer。`crates/scoop/src/commands/dump_{hir,mir,ir,effect_facts,effect_lowered}.rs` 与 `crates/scoop/src/fixtures/mod.rs` 当前 active 路径均直接复用这些稳定 surface，不再走 `format!("{:#?}")` 或“先 `Debug` 再字符串补丁”的链路。
    - fixture 侧的 textual churn 继续限定在 identity surface：P5-T01 / P5-T02 两次提交只刷新了 `.hir` / `.mir` / `.effectfacts` / `.effectlowered` golden 与对应 renderer / dump 入口，没有引入额外语义测试协议分叉；本轮复跑 `cargo test -p scoopc`、`cargo test -p scoop` 与五个 fixture phase 后也未发现行为漂移。
    - 允许继续保留 `Debug` 的边界明确为内部用途，不属于 active dump / fixture 协议：
      - 单元测试 / panic / 诊断信息里的调试输出；
      - `crates/scoop/src/fixtures/mod.rs` 中受 `SCOOP_FIXTURE_REPRO_DIR` 控制的 opt-in raw MIR repro 文件；
      - `crates/scoop/src/commands/dump_ast.rs` 这类不在本轮 stable-id textual surface 范围内的命令。
      - 上述边界之外，CLI dump、fixture golden、stage `stable_dump()` 与相关 snapshot surface 均不得再直接依赖 raw `Debug` 协议。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/hir`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/mir`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoop --all-targets -- -D warnings`
    - 精确文本审计结果：
      - `tests/fixtures/hir/*.hir`、`tests/fixtures/mir/*.mir`、`tests/fixtures/mir_refactor/*.mir` 中 `TypeId(`、`S0`、`C0`、`bb0`、`site0` 均无命中。
      - `tests/fixtures/effect_facts/*.effectfacts`、`tests/fixtures/effect_lowered/*.effectlowered` 中 `step_schema#0`、`continuation_schema#0`、`case#0`、`k0`、`ri0`、`ko0`、`st0`、`bd0`、`fs0`、`local0`、`bb0`、`site0` 均无命中。
      - `crates/scoop/src` 中仅剩两处 `format!("{:#?}")`：`dump_ast` 命令与 `SCOOP_FIXTURE_REPRO_DIR` 控制的 raw MIR repro；不属于 active stable dump / fixture surface。
      - `crates/scoopc/src` 中未发现 active dump/fixture 路径残留旧 textual protocol；仅有内部测试 / 诊断 / `hir_preflight` 调试用途的 `format!("{:#?}")`，以及 `effect_lowered/materialize.rs` 中局部变量名 `k0` 这类实现细节命中，不构成对外 textual surface 泄漏。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P5 review 目标：已复核 HIR / MIR / IR / effect facts / effect lowered 五类 active textual surface 都经由独立稳定 renderer 输出，fixture runner 与 CLI 也都复用同一 authoritative dump 入口。
    - 对应 `STABLE_ID.md` §5.1 / §5.2 / §10：active textual surface 已满足“显式选择 identity 来源，而非先 `Debug` 再补丁”的强制规则；保留的 `Debug` 使用已收缩到内部调试边界，不再充当对外协议。

## P6：收尾 RTTI / JSON / shared hash helper

### [DONE] P6-T01：统一 RTTI / interface hash helper，并修复 closure env identity 来源

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
  - 改动范围：
    - `crates/scoopc/src/stable_id.rs`
    - `crates/scoopc/src/hir/mod.rs`
    - `crates/scoopc/src/hir/stable_closure.rs`
    - `crates/scoopc/src/rtti/mod.rs`
    - `crates/scoopc/src/rtti/type_desc.rs`
    - `crates/scoopc/src/itable.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
    - `TODO.md`
  - 核心决策：
    - 新增共享 HIR closure lexical-path helper（`crates/scoopc/src/hir/stable_closure.rs`），把 materialized MIR closure 与 RTTI closure-env 都收口到同一份 `$lambdaN(. $lambdaM)*` authoritative 规则，不再让 RTTI 侧单独发明一套 closure identity 恢复逻辑。
    - `dump-rtti` closure env 现在按 root owner 的 `StableDefKey` + lexical path 构造 `StableClosureKey`，并使用 `StableClosureKey::env_canonical_name()` 作为 canonical name；`type_id` 同步改走 shared `stable_rtti_type_id(...)`，不再输出/哈希 `scoop.lambda_env$<ClosureId>`。
    - 在 `stable_id.rs` 增加 shared RTTI helper：`stable_rtti_type_id(...)` 与 `stable_rtti_interface_id(...)`。`rtti/mod.rs`、`rtti/type_desc.rs`、`itable.rs`、以及 LLVM sibling case（`llvm/codegen/mod.rs`、`mir_body.rs`、`gc.rs`）中的 RTTI/interface/runtime-match id 生成点全部改为复用这组 helper，避免继续散落 `stable_hash64(StableHashScope::RttiV0, ...)` 分叉调用。
    - 对 interface/runtime-match 输入规范的最终决定是：descriptor `type_id`、`interface_type_id`、`runtime_match_type_ids` 统一消费 canonical RTTI type name，经 `stable_rtti_type_id(...)` 生成；`interface_id` 统一消费 canonical interface FQN，经 `stable_rtti_interface_id(...)` 生成。保持 shared `rtti0:` scope，不额外引入与现有 runtime contract 不一致的 ad hoc 子前缀。
    - `.cone` / JSON 健康 schema 仅做防回归审计，继续复用既有 `path_free` 基线测试，不重写 `api.scoopir` / `PRE_SPECIALIZE.json` / `SYMBOL_VISIBILITY.json` / `ANNOTATION_CLASSES.json` 结构。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc dump_rtti -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 精确搜索摘要：
      - `fn stable_hash64`：仅剩 `crates/scoopc/src/stable_id.rs`
      - `ClosureId|scoop\.lambda_env\$`：`crates/scoopc/src/rtti`、`crates/scoopc/src/itable.rs`、`crates/scoopc/src/llvm/codegen/gc.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs` 中均为 0 命中
      - `crates/scoopc/src/llvm/codegen/mod.rs` 仍保留 `ClosureId` 内部命中，但仅用于 codegen-time lexical-path cache，不属于 RTTI identity path
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P6：RTTI / interface id / type id 已统一经 shared helper 生成；closure env 的 canonical name / `type_id` 已从 `ClosureId` 迁出；健康 `.cone` / JSON surface 继续停留在“防回归而非重写”的边界内。
    - 对应 `STABLE_ID.md` §3.3 / §8.4：`dump-rtti` closure env 现在显式走 `StableClosureKey -> canonical name -> shared RTTI hash helper`，不再把 dense id 当作 RTTI hash 输入；interface / runtime-match 同类 surface 也已统一到 shared helper，而不是各模块自带 `stable_hash64` 调用。

### [DONE] P6-T01R：Review RTTI 与 JSON 收口，确认剩余外部 surface 已只需最终验收

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
  - 改动范围：
    - `TODO.md`
  - 核心决策：
    - 本次 review 未发现需要回插到 `P6-T01R` 之前的新前置任务；最近一次提交 `[P6-T01] Unify RTTI helpers and closure env identity` 的正文也未声明仍待处理的直接后续项，因此 `P6-T01R` 可以按既定范围闭合。
    - closure env 的 RTTI identity 路径已完成从 `ClosureId` 脱钩：`crates/scoopc/src/rtti/type_desc.rs` 现通过 `collect_stable_closure_envs(...)` 恢复 `StableClosureKey`，并以 `StableClosureKey::env_canonical_name()` + `stable_rtti_type_id(...)` 生成 `dump-rtti` 的 `name/type_id`；相关精确搜索未再发现 `rtti` / `itable` / `llvm/codegen/gc.rs` / `llvm/codegen/mir_body.rs` 中残留旧 `ClosureId` 或 `scoop.lambda_env$` 生产路径。
    - RTTI / interface / runtime-match id 已统一收口到 shared helper：`rtti/mod.rs`、`rtti/type_desc.rs`、`itable.rs`、`llvm/codegen/gc.rs`、`llvm/codegen/mir_body.rs` 与 `llvm/codegen/mod.rs` 中的相关入口均复用 `stable_rtti_type_id(...)` / `stable_rtti_interface_id(...)`；`StableHashScope::RttiV0` 的直接使用已收缩到 `stable_id.rs` 内部 helper 封装与其单测。
    - `.cone` / JSON 健康基线继续停留在“防回归而非重写”的边界内；本轮明确证明当前无需再改的 JSON / cache surface 为：`api.scoopir`、`PRE_SPECIALIZE.json`、`SYMBOL_VISIBILITY.json`、`ANNOTATION_CLASSES.json`。除最终 P7 全量审计外，这些 surface 不再需要单独的 stable-id 结构改写任务。
  - 验证结果：
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc dump_rtti -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - 精确搜索摘要：
      - `ClosureId`：`crates/scoopc/src/rtti`、`crates/scoopc/src/itable.rs`、`crates/scoopc/src/llvm/codegen/gc.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs` 中均为 0 命中；`crates/scoopc/src/llvm/codegen/mod.rs` 仍保留 2 处 `ClosureId` 作为 codegen-time lexical-path cache key，不属于 RTTI / JSON 外部 surface。
      - `scoop\.lambda_env\$`：生产代码中无命中；仅 `crates/scoopc/src/rtti/type_desc.rs` 保留 1 处负向测试断言，用于防止旧 closure env spelling 回流。
      - `stable_hash64\(`：RTTI 路径上不再有模块自带分叉调用；仓库内 `StableHashScope::RttiV0` 只剩 `crates/scoopc/src/stable_id.rs` 中的 shared helper 与单测命中。
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P6 review 目标：已复核 closure env RTTI identity、interface/runtime-match id shared helper、以及 `.cone` / JSON 健康基线三项要求，确认除 P7 最终全量审计外，不再存在新的技术迁移面。
    - 对应 `STABLE_ID.md` §3.1 / §3.3 / §10：closure env `name/type_id` 已与 `ClosureId` 脱钩；RTTI / interface / runtime-match id 统一由 shared helper 产生；四个健康 JSON / cache surface 继续保持语义键与 path-free 基线，无需额外 schema churn。

## P7：全量审计、fixture refresh 与无功能漂移验收

### [DONE] P7-T01：运行最终审计矩阵，刷新快照，并验证路径稳定性 / 多 cone / 无功能漂移

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
  - 改动范围：
    - `crates/scoop/Cargo.toml`
    - `Cargo.lock`
    - `crates/scoop/src/fixtures/expectations.rs`
    - `crates/scoop/src/fixtures/mod.rs`
    - `crates/scoopc/src/dump_support.rs`
    - `crates/scoopc/src/rtti/type_desc.rs`
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
    - `crates/scoopc/src/llvm/codegen/call/lowering.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`
    - `tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
    - `tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`
    - `tests/fixtures/build/effect_refactor_step_enum_canonical_full_O0.scoop`
    - `tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
    - `tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - 核心决策：
    - 为满足 `P7` 的“路径稳定性”验收，补了两类常驻回归：`dump_support.rs` 现在显式验证不同 checkout 根路径下的 dump path / dump label 归一化结果一致；`rtti/type_desc.rs` 现在显式比较不同 checkout 根路径下的 RTTI dump identity 一致。现有 `pipeline::llvm_codegen_stage` object-level path-stable / multi-cone tests 继续作为 external symbol 与 collision 基线。
    - build fixture 断言模型扩展为支持 `BUILD-LLVM-REGEX`，并把仍锁定旧 private helper spelling 的 stable-id 敏感 build fixtures 统一迁到 hashed family regex。这样保留了对 helper family / case-tag / vtable shape 的验证力度，同时不再把旧 textual spelling 当作正确性标准。
    - 全量矩阵暴露并修复了两个真实 P7 blocker，而不是用 snapshot/fixture 规避：
      - 纯 direct-HIR closure 在 `top_level_val_init` / 同类 compiler-private init helper 中构造 callable object 时，没有注册 plain callable-carrier fallback，导致 callable carrier contract 报缺少 published target entry；现已在 `closure/mod.rs` 为纯 direct-HIR closure 注册 fallback，并补了对应 LLVM 回归。
      - class init / ctor 路径上的 concrete generic direct call 在缺少 authoritative instance key 或 published signature 时，会错误退回 generic HIR callable 计算 exported symbol；现已同时修正 HIR direct-call 的 concrete arg-type 回填、materialized callable 的 concrete-MIR signature fallback，以及 HIR top-level call binding 的 call-span 使用，恢复 init/helper 路径对 concrete generic callable 的 stable ABI 解析。
    - 最终 grep 审计分类结论：
      - `__schema[0-9]+`、`__k[0-9]+`、`t[0-9]+__` 均为 0 命中，说明旧 effect private symbol 路径已经退出 active tree。
      - `scoop.lambda$[0-9]+` / `scoop.lambda_resume$[0-9]+` / `scoop.lambda_env$[0-9]+` 仅剩 `crates/scoopc/src/llvm/tests.rs` 中的 classifier / negative inventory 文本，属于测试数据，不是生产路径。
      - `module.add_function(..., None)` 命中仅剩 `crates/scoopc/src/llvm/tests.rs` 审计常量与失败信息文本，生产 LLVM declaration path 已无 raw callsite。
      - `stable_template_symbol_suffix` 命中只剩 `stable_id.rs` authoritative helper、本体接入点（`hir/lower/util.rs`、`mir/materialize.rs`）与审计测试；它是当前 stable-id 正式 API，不是 legacy fallback。
      - `source_path.*decl_span` 命中仅剩内部 binder/symbol key（`hir/mod.rs`、`hir/lower/types.rs`）与 dump wiring，本轮未再发现它们进入 active dump / JSON / RTTI / object external surface。
      - 其余 `TypeId(` / `SymbolId(` / `ClosureId(` / `SourceId(` / `ConeId(` / `BasicBlockId(` / `LocalId(` / `SiteId(` / `StepSchemaId(` / `ContinuationSchemaId(` / `CaseTag(` / `ResumeInterfaceId(` / `ContinuationObjectId(` / `StateId(` / `BoundaryId(` / `FrameSlotId(` 残余命中，均落在三类位置：内部 handle/type 定义、healthy schema 的负向 path-free 断言、以及 stable-id 审计测试常量；未发现新的 active external leakage，因此无需插回新的前置任务。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc checkout_root -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc distinct_virtual_cones -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit_grep_inventory_scans_repo_roots -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop parse_build_llvm_contains_directives -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop build_fixtures_propagate_single_pipeline_session_options_to_build_command -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/build`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc callable_value_pattern_binder_receiver_named_args_fixture_codegen_succeeds -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc class_init_order_fixture_collects_class_init_println_call_bindings -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop_runtime`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop_tools -- spec-fixtures check`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test --all`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P7 与 `STABLE_ID.md` §10 的验收项：external symbol 集、RTTI identity、dump label 的路径稳定性已有常驻验证；多-cone virtual-source user ABI symbol 继续保持无碰撞；dump/fixture/build surface 现已刷新到 stable family / stable label / hashed namespace 基线。
    - 对应 `STABLE_ID.md` §11：完整 grep 清单已重跑并分类，旧 effect helper 名字模式已清零，残余命中均已证明属于内部 handle、健康基线负向断言或审计测试文本，而不是 active external leakage。
    - 对应 `PLAN.md` §6 / `STABLE_ID.md` §10-§12：在当次全量 `scoopc` / runtime / fixture / spec-fixture / workspace / clippy 矩阵通过后进入最终 review；随后 `P7-T01R` 预检查发现 object/top-level init 仍残留 legacy private naming，因此需先完成补遗任务 `P7-T01A`，再做最终签收。

### [DONE] P7-T01A：收口剩余 object/top-level init compiler-private function/global 命名到 `PrivateSymbolMangler`

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / P2、§6
  - [`STABLE_ID.md`](./STABLE_ID.md) §5.1、§7.3、§7.4、§8.5、§10
- 背景：
  - `P7-T01R` 预检查发现，production code 中 object/top-level init 相关 compiler-private function/global 仍有 active legacy spelling，尚未统一走 `PrivateSymbolMangler`。
  - 当前已确认入口包括：
    - `crates/scoopc/src/llvm/codegen/object_init.rs`
      - `__scoop_object_init__{object_fqn}`
      - `__scoop_refactor_hidden_object_init_bridge__{object_fqn}`
      - `__scoop_object_guard__{object_fqn}`
      - `__scoop_object_instance__{object_fqn}`
      - `__scoop_object_prop__{prop_fqn}`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
      - `__scoop_top_level_val_init__{value_fqn}`
      - `__scoop_refactor_hidden_top_level_init_bridge__{value_fqn}`
      - `__scoop_top_level_val_guard__{value_fqn}`
      - `__scoop_top_level_val__{value_fqn}`
      - `__scoop_top_level_var__{var_fqn}`
  - 该缺口直接违反 `STABLE_ID.md` §5.1 第 5/7 条与 §7.3/§8.5 对 compiler-private helper/global 命名的强制要求，因此 `P7-T01R` 不能在此之前签收。
- 目标：
  - 把 remaining object/top-level init compiler-private function/global 命名统一迁移到 `PrivateSymbolMangler`，同时保持现有 internal/private linkage 与初始化/GC 语义不变。
- 必须实现的内容：
  1. 在 `crates/scoopc/src/llvm/codegen/object_init.rs` 中，把 object init bridge/init function/guard/instance/property global 的 active production 命名收口到 `PrivateSymbolMangler`，必要时仅保留 FQN 作为 readable prefix。
  2. 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中，把 top-level immutable init bridge/init function/guard/value global/top-level var global 的 active production 命名收口到 `PrivateSymbolMangler`，并同步更新相关 root-callable identity、explicit-root descriptor 与调用路径。
  3. 更新 `crates/scoopc/src/llvm/tests.rs` 与相关 source inventory / object audit，确保：
     - 不再把 `__scoop_object_init__...` / `__scoop_top_level_val__...` 一族视为 production-active private name；
     - 仍继续验证 helper family、调用关系、descriptor/global 角色与 linkage 语义。
  4. 对 class/object/top-level init 相关 fixture 与定向 LLVM 测试补齐回归验证，避免只修一个 helper 而留下 sibling case。
- 必须遵从的约束：
  - 不得改变 class/object/top-level init 的 once 语义、求值顺序、GC rooting 或 runtime ABI。
  - 不得把 `main`、runtime/native import、`@Extern` symbol 误并入 private naming 清理范围。
  - 必须按同根问题成组处理 object/top-level init private functions 与 globals，不能只改其中一两个名字。
- 验证：
  1. `cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
  2. `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  3. `cargo test -p scoopc top_level_immutable_init_emits_explicit_root_frame_descriptor -- --nocapture`
  4. `cargo test -p scoopc direct_hir_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref -- --nocapture`
  5. `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  6. `cargo test -p scoopc class_init_order_fixture_collects_class_init_println_call_bindings -- --nocapture`
  7. `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  8. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop`
  9. `cargo test -p scoopc`
  10. `cargo clippy -p scoopc --all-targets -- -D warnings`
- 完成条件：
  - object/top-level init 相关 compiler-private function/global 的 active production 命名已统一走 `PrivateSymbolMangler`，不再残留上述 legacy family；随后才能继续执行 `P7-T01R`。
- 依赖：P7-T01
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/object_init.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`
    - `crates/scoop_runtime/tests/once_guard_cross_dylib.rs`
  - 核心决策：
    - object init bridge/init function/guard/instance/property global 与 top-level immutable init bridge/init function/guard/value global、top-level var global，现统一通过 `PrivateSymbolMangler` 生成 `__scoop_priv0__<role>__h<hash128>` 名字；保留可读 role，但唯一性完全转交 stable hash。
    - stable key 仍显式保留 `namespace` / `declaration_kind`，不是因为 `package.name` 与 `package.objectId.name` 这类源码 FQN 会冲突，而是因为同一 owner FQN 会派生多个 compiler-private 实体（init helper、bridge、guard、instance slot、backing global 等），不能只靠 FQN 文本区分。
    - 本轮收口的 role family 固定为：`object_init`、`hidden_object_init_bridge`、`object_guard`、`object_instance`、`object_prop`、`top_level_val_init`、`hidden_top_level_init_bridge`、`top_level_val_guard`、`top_level_val`、`top_level_var`；从而在不再依赖旧 FQN 拼接的前提下，保留 helper/global 家族可读性与现有测试语义。
    - 测试面新增 `stable_id_source_inventory_removes_legacy_init_private_naming_from_codegen`，并把相关 LLVM/build/runtime 回归改到新 private role family：
      - LLVM 定向测试现在显式验证 `object_instance` / `object_init` / `top_level_val_init` role。
      - `unsafe_atomic_int_top_level_storage_llvm.scoop` 从旧固定名字迁到 hashed regex，继续验证 top-level atomic storage 直接命中静态槽。
      - `once_guard_cross_dylib.rs` 改用 representative 的新 `object_guard` private family，避免 runtime 回归继续示范旧 spelling。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc top_level_immutable_init_emits_explicit_root_frame_descriptor -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc direct_hir_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc class_init_order_fixture_collects_class_init_println_call_bindings -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/build`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
    - `cargo test -p scoop_runtime once_guard_is_canonical_across_dylibs -- --nocapture`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` P2 / P7 与 `STABLE_ID.md` §5.1、§7.3、§7.4、§8.5：remaining object/top-level init compiler-private function/global 已全部收口到统一 private mangler，且继续保持 internal/private linkage，不再依赖 legacy FQN 拼接进入全局命名空间。
    - 对应 `STABLE_ID.md` §10：object/top-level init 相关 LLVM/build/runtime 回归现在锁定的是 private role family、调用关系、descriptor/global 角色与 linkage 语义，而不是 `__scoop_object_init__...` / `__scoop_top_level_var__...` 旧 textual spelling；`P7-T01R` 现在可以在此基线上继续做最终签收。

### [DONE] P7-T01B：收口剩余 sanitize/type-display/TypeId 驱动的 private LLVM type/global 命名

- 参考：
  - [`PLAN.md`](./PLAN.md) §6
  - [`STABLE_ID.md`](./STABLE_ID.md) §5.1、§7.3、§8.5、§10
- 背景：
  - `P7-T01R` review 复核发现，production code 中仍有一批 active compiler-private LLVM type/global 名称直接由 `sanitize_llvm_ident(...)`、`TypeStore::display()` 或 raw `TypeId` 数字驱动，尚未统一进入 stable private naming。
  - 当前已确认入口至少包括：
    - `crates/scoopc/src/llvm/codegen/mod.rs`
      - `llvm_boxed_enum_type`
      - `get_or_create_boxed_enum_type_desc_global`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
      - `get_or_create_class_type_desc_global`
      - `llvm_object_singleton_type`
      - `get_or_create_object_singleton_type_desc_global`
      - `get_or_create_itable_global_from_entries`
      - `get_or_create_class_vtable_global`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
      - `mir_capture_box_object_type`
      - `mir_value_box_object_type`
      - `get_or_create_mir_capture_box_type_desc_global`
      - `get_or_create_mir_value_box_type_desc_global`
    - `crates/scoopc/src/llvm/codegen/ty.rs`
      - `llvm_class_payload_type`
      - `llvm_class_object_type`
      - `llvm_enum_boxed_payload_struct_type`
      - `llvm_enum_boxed_payload_object_type`
      - `get_or_create_enum_boxed_payload_type_desc_global`
    - `crates/scoopc/src/llvm/codegen/composite_transport.rs`
      - `composite_transport_descriptor_global_name`
  - 这些路径与 `STABLE_ID.md` §5.1 第 5/7 条及 `PLAN.md` §6 第 1/7 条直接冲突：`sanitize_llvm_ident()` 仍在承担唯一性，且部分命名仍混入 `TypeStore::display()` / `descriptor.source_ty.as_u32()`。
- 目标：
  - 把 remaining runtime metadata / type-desc / transport private LLVM type/global 命名统一迁到 stable semantic key + `PrivateSymbolMangler`（或等价 authoritative stable helper），同时保持 internal/private linkage 与 RTTI/GC/layout 语义不变。
- 必须实现的内容：
  1. 收口上述 runtime boxed enum、class/object type desc、itable/vtable、MIR capture-box/value-box、composite transport descriptor 的 active production naming。
  2. 新命名不得再由 `sanitize_llvm_ident()`、`TypeStore::display()` 或 raw `TypeId` / `source_ty.as_u32()` 承担唯一性主体；若保留可读文本，只能作为 prefix。
  3. 为这些 private type/global family 增加或扩充 stable-id source inventory / IR 回归，防止 sanitize/type-display/TypeId 命名回流。
  4. 按同根问题成组处理 sibling case，不能只修某一种 type-desc 或 transport 名称。
- 必须遵从的约束：
  - 不得改变 RTTI `type_id` / `interface_id`、GC bitmap、layout、itable/vtable slot、transport contract 或运行时 ABI 语义。
  - 不得把 exported ABI symbol、`main`、runtime/native import 误并入 private naming 清理范围。
  - 对 capture/value box 与 composite transport，不得把旧 `TypeId` / pretty text 只“换个壳”继续当唯一性主体；必须改到 authoritative stable key。
- 验证：
  1. `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  2. `cargo test -p scoopc runtime_type_primitives -- --nocapture`
  3. `cargo test -p scoopc composite_transport -- --nocapture`
  4. `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  5. `cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
  6. `cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
  7. `cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
  8. `cargo test -p scoopc`
  9. `cargo clippy -p scoopc --all-targets -- -D warnings`
- 完成条件：
  - remaining runtime metadata / type-desc / transport private LLVM type/global naming 不再由 sanitize/type-display/raw `TypeId` 控制；之后 `P7-T01R` 才能对 `PLAN.md` §6 第 1/7 条做最终签收。
- 依赖：P7-T01A
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/stable_id.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/ty.rs`
    - `crates/scoopc/src/llvm/codegen/composite_transport.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
    - `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
  - 核心决策：
    - 在 `stable_id.rs` 新增 `CanonicalTextKey`，并为 `PrivateSymbolMangler` 增加 `hash_suffix` / `type_name`，让 production code 可以直接从 authoritative canonical key 生成 hashed private LLVM type/global 名称，而不再依赖 `sanitize_llvm_ident()` 或 pretty text 拼接唯一性。
    - 把 boxed-enum runtime metadata、class/object type desc、itable/vtable、MIR capture-box/value-box、enum boxed payload、composite transport descriptor 这批 sibling private family 成组迁到 stable semantic key + `PrivateSymbolMangler`：
      - nominal/runtime metadata 路径统一走 `StableDefKey`；
      - type-driven metadata 路径统一走 canonical type text；
      - composite transport descriptor 改为用 transport contract 语义字段构造 canonical record，移除 `descriptor.source_ty.as_u32()` 与 display/sanitize 对唯一性的控制。
    - 扩充 LLVM/pipeline/source inventory 与 fixture 断言，改为锁定 `__scoop_priv0__*` private namespace、hashed type family、descriptor/global 角色与结构语义，防止 legacy sanitize/type-display/TypeId 命名回流到行为测试。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc runtime_type_primitives -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composite_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` §6 第 1/7 条：remaining runtime metadata、transport descriptor 与 private LLVM type/global naming 已统一走 authoritative stable private naming，`sanitize_llvm_ident()` / `TypeStore::display()` / raw `TypeId` 不再承担 private identity 主体。
    - 对应 `STABLE_ID.md` §5.1、§7.3、§8.5、§10：private LLVM metadata/type family 现已按 semantic key + hashed private namespace 发布，同时继续保持原有 RTTI/GC/layout/itable/vtable/transport ABI 语义不变，并有常驻 source inventory / IR 回归防止旧 naming source 回流。

### [DONE] P7-T01C：收口剩余 RTTI / runtime-match type_id 对 pretty text / sanitize 的依赖

- 参考：
  - [`PLAN.md`](./PLAN.md) §6
  - [`STABLE_ID.md`](./STABLE_ID.md) §3.3、§3.4.6、§3.4.7、§5.1、§7.1、§8.4、§10
- 背景：
  - `P7-T01R` 复核发现，active production code 中仍有一类 external identity 直接或间接由 `TypeStore::display()` / `sanitize_llvm_ident()` 驱动，尚未统一走 `stable_id` 的 canonical type encoder。
  - 当前已确认入口至少包括：
    - `crates/scoopc/src/rtti/mod.rs`
      - `type_rtti()` 先用 `self.types.display(ty)` 生成 `name`，再直接 `stable_rtti_type_id(&name)`。
    - `crates/scoopc/src/llvm/codegen/mod.rs`
      - `codegen_ref_is_instance_of_nonnull()` 在 interface runtime-match 路径中直接 `stable_rtti_type_id(&self.types.display(target_ty).to_string())`。
      - `get_or_create_boxed_enum_type_desc_global()` 把 `format!("scoop.runtime.BoxedEnum<{}>", type_store.display(enum_ty))` 作为 type descriptor `canonical_name`。
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
      - `get_or_create_mir_capture_box_type_desc_global()` 仍以 `sanitize_llvm_ident(&types.display(value_ty).to_string())` 构造 descriptor `canonical_name`。
      - `get_or_create_mir_value_box_type_desc_global()` 仍以 `format!("scoop.runtime.ValueBox<{}>", types.display(source_ty))` 构造 descriptor `canonical_name`。
    - `crates/scoopc/src/llvm/codegen/gc.rs`
      - `get_or_create_type_descriptor_global()` 当前仍把 `canonical_name` 直接喂给 `stable_rtti_type_id(...)`，导致上述 display/sanitize 文本继续承担 RTTI `type_id` 身份输入。
  - 这直接违反了 `STABLE_ID.md` 对“`TypeStore::display()` / `sanitize_llvm_ident()` 只能作为可读文本，不能承担 identity 责任”的要求，因此 `P7-T01R` 不能在该缺口未收口前签收。
- 目标：
  - 把 remaining RTTI / runtime-match / derived type-desc `type_id` 输入统一迁到 authoritative semantic canonical key，同时把“可读 display name”和“真正参与 hash 的 canonical key”彻底分开。
- 必须实现的内容：
  1. 为 RTTI `type_id` / runtime-match type-id 补齐统一的 semantic canonical key helper，直接复用 `stable_id` 的 canonical type encoder，而不是 `TypeStore::display()` / `sanitize_llvm_ident()`。
  2. 调整 `rtti/mod.rs`、`llvm/codegen/mod.rs`、`llvm/codegen/mir_body.rs`、`llvm/codegen/gc.rs` 中相关 call site；必要时为 `TypeDescriptorSpec` / `get_or_create_type_descriptor_global()` 显式拆分：
     - 供外部显示的 readable name
     - 供 `stable_rtti_type_id(...)` 使用的 canonical identity key
  3. 按同根问题成组处理 sibling case，至少覆盖：generic/parameterized RTTI query、interface runtime-match type-id、boxed enum / MIR capture-box / MIR value-box descriptor 路径；不得只修单一 descriptor 名称。
  4. 扩充 `stable_id_source_inventory`、RTTI dump 测试与相关 LLVM/runtime metadata 回归，防止 `stable_rtti_type_id(types.display(...))`、`sanitize_llvm_ident(display)` 再次回流为 external identity 输入。
- 必须遵从的约束：
  - 不得改变 RTTI dump 的人类可读展示目标，除非变化仅限 identity 字段从 pretty text 输入改到 semantic canonical key。
  - 不得改变 GC bitmap、layout、itable/vtable slot、runtime-match 语义或运行时 ABI。
  - 不得把 readable label 和 identity key 再次混用；若某处仍需要保留可读名，必须与 hash 输入显式分离。
- 验证：
  1. `cargo test -p scoopc dump_rtti -- --nocapture`
  2. `cargo test -p scoopc runtime_type_primitives -- --nocapture`
  3. `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  4. `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  5. `cargo test -p scoopc`
  6. `cargo clippy -p scoopc --all-targets -- -D warnings`
- 完成条件：
  - RTTI `type_id`、interface runtime-match type-id 与 derived runtime type-desc identity 已不再由 `TypeStore::display()` / `sanitize_llvm_ident()` 直接控制；随后 `P7-T01R` 才能继续做最终签收。
- 依赖：P7-T01B
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/stable_id.rs`
    - `crates/scoopc/src/typecheck/lower.rs`
    - `crates/scoopc/src/itable.rs`
    - `crates/scoopc/src/rtti/mod.rs`
    - `crates/scoopc/src/rtti/type_desc.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/ty.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `TODO.md`
  - 核心决策：
    - 在 `stable_id.rs` 新增 RTTI 专用 canonical helper：`stable_rtti_type_key_for_type`、`stable_rtti_type_id_for_type`、`stable_rtti_derived_type_key`、`canonical_nominal_type_key`。RTTI query / runtime-match / derived type-desc 现在统一复用 canonical type encoder，而不是再把 `TypeStore::display()`、`sanitize_llvm_ident()` 或裸 nominal 文本直接当作 hash 输入。
    - `rtti/mod.rs` 现在显式把 `TypeStore::display()` 只保留为 `TypeRtti.name` 的可读输出；`type_id` 改由 semantic canonical type key 计算。`itable.rs` 的 precise class itable metadata 与 `mir_body.rs` 的 value-box interface metadata 也同步切到 canonical type id，确保 parameterized interface runtime-match 与 RTTI query 走同一 identity 规则。
    - `TypeDescriptorSpec` 明确改名为 `type_id_key`，避免再把“可读 descriptor 名字”和“真正参与 `stable_rtti_type_id(...)` 的 identity key”混用。boxed enum / MIR capture-box / MIR value-box descriptor 现在都以“wrapper role + canonical payload/source type key”生成派生 type-id key，既脱离 display/sanitize，又不会与被包装源类型本身发生 identity 坍缩。
    - 扩充了 `stable_id_source_inventory` 与 RTTI dump 回归：source inventory 现在会阻止 `stable_rtti_type_id(self.types.display(...))`、`stable_rtti_type_id(interface_type_name)`、`stable_rtti_type_id(runtime_match_type_names)` 以及 `BoxedEnum<pretty>` / `ValueBox<pretty>` / capture-box sanitize 文本重新回流；RTTI dump 测试则显式验证 parameterized query / runtime-match metadata 的 `type_id` 已与 display name 分离，但 query 与 type-desc metadata 仍保持一致。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc dump_rtti -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc runtime_type_primitives -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` §6 与 `STABLE_ID.md` §3.3 / §3.4.6 / §3.4.7 / §7.1 / §8.4：remaining RTTI query、interface runtime-match type-id、boxed enum / MIR capture-box / MIR value-box derived type-desc identity 已全部切到 canonical semantic key，不再由 pretty text / sanitize 直接控制。
    - 对应 `STABLE_ID.md` §5.1 / §10：可读 RTTI/type-desc 名称与真正 hash 输入现在已显式分离，且有常驻 source inventory / RTTI dump / LLVM metadata 回归防止旧输入源回流；`P7-T01R` 可以继续做最终全量签收。

### [DONE] P7-T01D：为 LLVM type-driven stable-id helper 接入 authoritative type-param resolver

- 参考：
  - [`PLAN.md`](./PLAN.md) §5.4、§6
  - [`STABLE_ID.md`](./STABLE_ID.md) §5.1、§7.1、§8.4、§8.5、§10
- 背景：
  - 在执行 `P7-T01R` 的最终签收矩阵时，`cargo run -p scoop -- test` 失败，阻塞用例为 `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`。
  - 直接复现 `cargo run -p scoop -- run tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop` 的报错为：
    - `LLVM codegen 前端准备失败：MIR value box LLVM type 无法构造 stable canonical type key: missing stable type parameter key for 'B'`
  - 初步定位显示，active production code 中 `crates/scoopc/src/llvm/codegen/mod.rs` 的 `canonical_type_key_text_for_codegen` / `stable_rtti_type_id_for_codegen` 仍固定使用 `NoTypeParamResolver`；而 `crates/scoopc/src/llvm/codegen/mir_body.rs` 的 MIR value-box/type-desc/type-driven private naming 在某些泛型 cleanup / unwind / boxing 路径中已经会遇到仍含 type param 的 `TypeId`。这直接违反了 `stable_id` canonical encoder 对 type-param key 的要求，也阻断了最终全量验收。
- 目标：
  - 让 LLVM codegen 中所有会为“仍含 type param 的语义类型”生成 stable canonical key / RTTI type-id / private type-global 名称的 active production 路径，都改为使用 authoritative type-param resolver，而不是 `NoTypeParamResolver`。
  - 修复该类路径后，恢复 `class_init_raise_cleanup_init_block_gc_basic.scoop` 与同根 generic cleanup/boxing 路径的正常编译运行。
- 必须实现的内容：
  1. 在 LLVM codegen 层建立可复用的 authoritative stable type-param resolver 接入点，来源必须是当前 callable / owner / instance 的真实语义键，而不是 pretty text、raw type param 名或任何 path/span 文本。
  2. 把 `canonical_type_key_text_for_codegen`、`stable_rtti_type_id_for_codegen` 以及其 active production 调用点接到该 resolver；至少覆盖：
     - `crates/scoopc/src/llvm/codegen/mir_body.rs`
       - `mir_value_box_object_type`
       - `get_or_create_mir_value_box_type_desc_global`
       - 同根的 MIR capture-box / value-box / itable owner type-driven naming 路径
     - 任何仍会对 generic-bearing `TypeId` 调用上述 helper 的 RTTI / private metadata / transport sibling case
  3. 为本次 blocker 补齐回归测试，至少覆盖：
     - `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`
     - 一个直接锁定 generic-bearing MIR value-box 或同根 type-driven naming 的 LLVM/codegen 定向测试
  4. 按同根问题成组处理 sibling case，不能只让单个 fixture 通过，而继续让 generic capture-box / RTTI / transport private naming 在别的 cleanup/boxing 路径上保留同类缺口。
- 必须遵从的约束：
  - 不得回退到 `TypeStore::display()`、`sanitize_llvm_ident()`、raw type param 名字、path/span 或 dense id 作为 canonical 输入或 hash 主体。
  - 不得只在该 fixture 上加特判；必须修复 generic-bearing type-driven stable-id helper 的整类问题。
  - 不得改变 value-box / capture-box / RTTI / transport / cleanup unwind 的 GC、layout、ABI 与运行语义。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`
  2. `cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
  3. `cargo test -p scoopc runtime_type_primitives -- --nocapture`
  4. `cargo test -p scoopc`
  5. `cargo run -p scoop -- test`
  6. `cargo clippy -p scoopc --all-targets -- -D warnings`
- 完成条件：
  - LLVM codegen 的 generic-bearing type-driven stable-id helper 已能稳定拿到 authoritative type-param key，不再因为 `missing stable type parameter key` 阻断 active production 编译路径；随后 `P7-T01R` 才能继续最终签收。
- 依赖：P7-T01C
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/hir/lower/mod.rs`
    - `crates/scoopc/src/hir/lower/types.rs`
    - `crates/scoopc/src/hir/lower/util.rs`
    - `crates/scoopc/src/llvm/emit.rs`
    - `crates/scoopc/src/llvm/codegen/mod.rs`
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/stable_naming.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `TODO.md`
  - 核心决策：
    - 在 HIR lowering 侧新增 `stable_type_param_keys` 索引，把声明级 `TypeParamType` / effect-row placeholder 绑定到 authoritative `StableTypeParamKey(owner_def_key#index)`，并随 `LoweredHir` 一起传入 LLVM codegen；这样 codegen 不再需要按 pretty text、raw param 名或 path/span 临时猜测 owner。
    - `CompilationUnitCodegenCx` 现在把这份索引作为共享 `StableTypeParamResolver` 暴露给 production 路径；`canonical_type_key_text_for_codegen`、`stable_rtti_type_id_for_codegen`、MIR value-box interface RTTI、non-generic callable signature key、以及 effect-lowered 的 task transport / effect transport box / step schema / continuation schema stable naming 全部切到同一 authoritative resolver。
    - effect-lowered stable naming 不再把 generic-bearing type/effect canonicalization 固定绑在 `NoTypeParamResolver`；同根的 MIR value-box、effect transport box、resume task transport、continuation/step private naming sibling case 现在会与主 helper 共用同一份 owner/index 规则，而不是只修单个 fixture。
    - 新增 `generic_class_init_raise_cleanup_uses_stable_type_driven_box_naming`，直接锁定“generic class init cleanup + Raise unwind”下的 type-driven private box naming，防止 `missing stable type parameter key` 在 MIR value-box / effect transport family 回流。
  - 验证结果：
    - `cargo fmt`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc generic_class_init_raise_cleanup_uses_stable_type_driven_box_naming -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc runtime_type_primitives -- --nocapture`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo run -p scoop -- test`
    - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 与 `PLAN.md` / `STABLE_ID.md` 对应闭合：
    - 对应 `PLAN.md` §5.4 / §6：LLVM codegen 的 generic-bearing type-driven stable-id helper 已统一接入 authoritative type-param resolver，`class_init_raise_cleanup_init_block_gc_basic.scoop` 不再阻断最终签收矩阵，`P7-T01R` 可继续执行最终审计。
    - 对应 `STABLE_ID.md` §5.1 / §7.1 / §8.4 / §8.5 / §10：MIR value-box、RTTI type-id、effect-lowered transport/private naming 等 active production surface 现在都从声明级 owner/index key 取得 canonical type-param identity，不再回退到 `NoTypeParamResolver`、pretty text、raw param 名或局部 workaround。

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
- 依赖：P7-T01、P7-T01A、P7-T01B、P7-T01C、P7-T01D
- 完成记录：
  - 待填。
