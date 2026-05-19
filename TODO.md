# TODO（Sysroot Reshape R2）

> 生成时间：2026-05-19
> 设计基线：[`SYSROOT_RESHAPE_R2.md`](./SYSROOT_RESHAPE_R2.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 当前状态：`P1-T02` 已完成；下一任务为 `P1-T03`。
> 执行原则：严格按 P0 -> P10 顺序推进；同一阶段内可按任务依赖拆 PR，但每个任务完成后必须保持仓库无 failing fixture，并回写完成记录。

## 全局约束

- [`SYSROOT_RESHAPE_R2.md`](./SYSROOT_RESHAPE_R2.md) 是本轮唯一设计基线；[`PLAN.md`](./PLAN.md) 是本轮唯一执行计划基线。若实现中需要改变 cone kind、sysroot layout、native boundary、runtime core 边界或 object/top-level 初始化语义，必须先更新设计与计划。
- 本轮先退场现有 `.cone` archive active path，回到 source-only cone 模型。不保留当前 `.cone` 格式兼容性，不做 binary distribution 新设计。
- Cone kind 只包含 `bin`、`lib`、`syslib`。`syslib` 只允许位于 `sysroot/lib/<cone.fqn>/` 下。
- `SourceOrigin::Sysroot` 不再直接授予语义特权；`@Intrinsic`、`@file:AllowIntrinsic`、sealed marker 等特权必须来自所属 cone 的 trusted `syslib` 身份。
- `lib` / `syslib` 不要求 entry point；只有 consumer `bin` cone 要求 entry point。
- Auto dependency 和 prelude package 是两个独立列表。自动依赖只决定 cone 是否加载/编译/链接；prelude 只决定未显式 import 时的短名可见性。
- Cone-local native sources 必须参与完整编译和链接；不能继续只编译当前 executable cone 的 C/C++ 源。
- `runtime/c/` 中实际属于某个 sysroot cone 的 C 代码必须迁移到对应 cone 的 `native/` 目录，并通过该 cone 的 `native-build` metadata 编译链接。
- Runtime core 只能保留 GC、allocation、root/native transition、object/type descriptor、global root、GC thread lifecycle、fatal trap substrate 和稳定公开 runtime header 所需声明。
- Object 初始化使用 codegen LLVM atomics + compiler-generated TLS object-init frame stack；不依赖 `Mutex`、`CondVar`、`scoop.sync.Once` 或 `scoop_once_begin/end`。
- Top-level `val` / annotated top-level `var` 必须覆盖所有 linked source cones，并在进入用户 `main` body 前 eager 初始化；不得 lazy first-access。
- Top-level `var` 必须显式标注 `@Global` 或 `@ThreadLocal`。
- 仓库内不得留下 failing fixture。过时 archive fixture 要删除或改写为 source-only cone fixture；删除必须说明被测对象已消失或被哪些新 fixture 覆盖。
- 每个任务完成后必须回写：改动范围、核心决策、验证结果、与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 的闭合项、如有 fixture 删除/改写则说明理由。

## 固定定位清单

### 设计与计划

- `SYSROOT_RESHAPE_R2.md`：R2 设计基线。
- `PLAN.md`：R2 阶段计划。
- `TODO.md`：本任务清单。
- `SCOOP_FULL_SPEC.md`：最终需要同步 cone/sysroot/native boundary 语义。

### Cone / manifest / package

- `crates/scoopc/src/cone/manifest.rs`：`Cone.toml` 解析，当前已有 `[native-build]`。
- `crates/scoopc/src/cone/package.rs`：source package 加载，当前要求 `src/main.scoop`。
- `crates/scoopc/src/cone/archive.rs` / `consume.rs` / `scoopir/*`：`.cone` archive 写入/读取/API 注入路径。
- `crates/scoop/src/commands/package.rs`：`scoop package` archive CLI。
- `crates/scoop/src/commands/build.rs`：normal build context；P1-T02 后不再加载 `.cone` dependency graph。

### Sysroot / frontend

- `crates/scoopc/src/sysroot/mod.rs`：当前递归扫描 sysroot `.scoop` 文件。
- `crates/scoopc/src/frontend.rs`：support sources、`ProjectInput` / input-only `ProjectContext`、entry selection。
- `crates/scoopc/src/resolve/imports.rs`：自动 prelude star imports。
- `crates/scoopc/src/source.rs`：`SourceOrigin::Sysroot` / `SourceFile::is_sysroot()`。

### Native boundary / ABI

- `crates/scoopc/src/typecheck/annotations.rs`：`@Extern`、`@CallingConvention`、native/scoop ABI gates。
- `crates/scoopc/src/hir/mod.rs`：`ExternAbi`、`CallableAbiIdentity`、`ExternFun.calling_convention`。
- `crates/scoopc/src/llvm/codegen/call/abi.rs`：native callable ABI classification 和 `enter_native/leave_native` 插入。
- `crates/scoopc/src/llvm/codegen/main/declare.rs`：top-level function declaration、callconv、exported/object symbols。

### Runtime / native build

- `crates/scoop/src/commands/build.rs`：当前只编译 current explicit cone native sources。
- `crates/scoop/src/toolchain.rs`：C/C++ 编译与 runtime C 编译/链接。
- `runtime/c/scoop_runtime_api.h`：runtime core export allowlist。
- `runtime/c/scoop_thread.c` / `scoop_sync.c` / `scoop_test.c` / `scoop_once.c`：迁移或退场重点。
- `crates/scoop_runtime/src/abi_exports_allowlist.rs`：runtime export allowlist 测试。

### Fixture 入口

- `tests/fixtures/run_pass_cone/c_sources_extern_call_basic/`：现有 current-cone C source FFI 编译/链接基线。
- `tests/fixtures/typecheck_cone_archive/**`：当前 archive API 注入 fixtures，需删除或改写。
- `tests/testdata/cone/selectors_basic/`：source package selector 测试数据。

## 执行顺序总览

```text
P0 baseline freeze
  -> P1 archive 退场
    -> P2 native boundary annotations
      -> P3 cone kind
        -> P4 sysroot/lib layout
          -> P5 source cone graph
            -> P6 auto dependency + prelude
              -> P7 native build graph
                -> P8 runtime FFI migration
                  -> P9 object once + eager top-level init
                    -> P10 final verification
```

## 任务索引

| ID | 状态 | 阶段 | 标题 |
| --- | --- | --- | --- |
| `P0-T01` | [DONE] | P0 | 冻结 R2 baseline 与迁移清单 |
| `P1-T01` | [DONE] | P1 | 禁用/删除 `scoop package` archive CLI |
| `P1-T02` | [DONE] | P1 | 移除 normal build 的 `.cone` dependency flow |
| `P1-T03` | [TODO] | P1 | 删除或改写 archive fixtures 与 archive-only tests |
| `P2-T01` | [TODO] | P2 | `@Extern` 支持 `callingConvention` property |
| `P2-T02` | [TODO] | P2 | 有 body 的 `@CallingConvention` 生成 object-level native callable symbol |
| `P2-T03` | [TODO] | P2 | 支持 `Any as?` closed Pure function runtime cast |
| `P3-T01` | [TODO] | P3 | `Cone.toml` 解析 `kind = bin/lib/syslib` |
| `P3-T02` | [TODO] | P3 | `lib/syslib` 无 entry point 加载规则 |
| `P3-T03` | [TODO] | P3 | `syslib` path trust gate 与 intrinsic privilege gate |
| `P4-T01` | [TODO] | P4 | 重排 sysroot 到 `sysroot/lib/<cone>/src` |
| `P4-T02` | [TODO] | P4 | sysroot loader 改为加载 `sysroot/lib/*/Cone.toml` |
| `P4-T03` | [TODO] | P4 | sysroot overlay 迁移到 `overlay/lib/<cone>/...` |
| `P5-T01` | [TODO] | P5 | 引入 source cone graph 数据结构 |
| `P5-T02` | [TODO] | P5 | 支持本地 source path dependency fixtures |
| `P5-T03` | [TODO] | P5 | 保留 cone identity/kind 到 resolver/typecheck/codegen |
| `P5-T04` | [TODO] | P5 | 生成 per-cone init routine 与 final system entry 调用骨架 |
| `P6-T01` | [TODO] | P6 | 实现 auto dependency cone 列表 |
| `P6-T02` | [TODO] | P6 | 实现 prelude package 列表并与 auto dependency 解耦 |
| `P7-T01` | [TODO] | P7 | 将 native-build 扩展到所有 loaded source cones |
| `P7-T02` | [TODO] | P7 | dependency cone C++/link-flags/linker driver 覆盖 |
| `P8-T01` | [TODO] | P8 | 建立公开 `scoop_runtime.h` runtime core header |
| `P8-T02` | [TODO] | P8 | 迁移 `scoop.runtime.test` native helpers |
| `P8-T03` | [TODO] | P8 | 迁移 `scoop.sync` native implementation |
| `P8-T04` | [TODO] | P8 | 迁移 `scoop.thread` native implementation 与 thread entry trampoline |
| `P8-T05` | [TODO] | P8 | 迁移 string/native helper 边界并收窄 runtime allowlist |
| `P9-T01` | [TODO] | P9 | Object once 改为 LLVM atomics + TLS init-frame stack |
| `P9-T02` | [TODO] | P9 | Top-level `val` / annotated `var` 改为 per-cone eager init |
| `P9-T03` | [TODO] | P9 | top-level `var` annotation gate 与 storage 语义 |
| `P10-T01` | [TODO] | P10 | 全仓旧模型残留审计与 spec 文档同步 |
| `P10-T02` | [TODO] | P10 | 全量验证与最终完成记录 |

---

## [DONE] P0-T01：冻结 R2 baseline 与迁移清单

- 参考：`PLAN.md` §3、`SYSROOT_RESHAPE_R2.md` §0-§2。
- 目标：在删除旧 archive 和重排 sysroot 前，固定现状与迁移范围。
- 当前实现入口：`crates/scoopc/src/cone/**`、`crates/scoopc/src/sysroot/mod.rs`、`crates/scoop/src/commands/build*.rs`、`runtime/c/**`。
- 必须实现：
  1. 列出现有 archive 入口：`scoop package`、`ConeArchiveApi`、`load_cone_archive_api`、`inject_cone_dependency_public_api`、`typecheck_cone_archive` fixtures、`api.scoopir` export/consume。
  2. 列出现有 sysroot 特权入口：`SourceOrigin::Sysroot`、`SourceFile::is_sysroot()`、`@Intrinsic` gate、sealed marker sysroot-only gate。
  3. 列出现有 runtime-core 外溢 symbols：`scoop_thread_*` user API、`scoop_sync_*`、`scoop_test_*`、`scoop_once_*`。
  4. 记录 current-cone native-build 能力与 fixture：`tests/fixtures/run_pass_cone/c_sources_extern_call_basic/`。
- 验证：`cargo build`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`。
- 完成条件：本任务末尾有完成记录，后续 P1-P10 触碰文件簇清晰。

### 完成记录（2026-05-20）

- 改动范围：仅冻结和记录 R2 基线；未改编译器/runtime 行为，未删除或改写 fixture；`PLAN.md` 和 `SYSROOT_RESHAPE_R2.md` 的阶段级计划未变化。
- Archive active path 基线：`crates/scoop/src/cli.rs` 仍暴露 `scoop package`；`crates/scoop/src/commands/mod.rs` 仍 dispatch 到 `crates/scoop/src/commands/package.rs::run`；`crates/scoopc/src/cone/archive.rs` 写出 `Cone.toml`、`api.scoopir`、`SOURCES_SHA256` 等 archive entries；`crates/scoop/src/commands/build/deps.rs` 仍通过 `SCOOP_CONE_PATH`、`cone/`、`deps/` 和 consumer root 搜索 `.cone`；`crates/scoopc/src/frontend.rs::ProjectContext` / `run_frontend` 仍携带并注入 `Vec<ConeArchiveApi>`；`crates/scoopc/src/cone/consume.rs` 仍实现 `ConeArchiveApi`、`load_cone_archive_api`、`inject_cone_dependency_public_api` 与 `api.scoopir` consume；`crates/scoop/src/fixtures/mod.rs` 仍把 `tests/fixtures/typecheck_cone_archive/**` 作为真实 `.cone` 注入 suite 运行。
- Sysroot privilege 基线：`crates/scoopc/src/source.rs` 仍以 `SourceOrigin::Sysroot` / `SourceFile::is_sysroot()` 表示 sysroot 来源；`crates/scoopc/src/sysroot/mod.rs` 仍递归收集 sysroot/overlay 下 `.scoop` 并用 `SourceFile::load_sysroot` 加载；`crates/scoopc/src/frontend.rs::load_default_support_sources` 仍把 sysroot files 作为 support sources 加入输入；`crates/scoopc/src/typecheck/builtin_annotations.rs` 仍识别 `@file:AllowIntrinsic`；`crates/scoopc/src/typecheck/annotations.rs::check_intrinsic_builtin_annotation_gate` 仍允许 `file_allows_intrinsic || source_is_sysroot(source)`；`crates/scoopc/src/typecheck/type_env.rs::check_sealed_interface_decl_shape` 仍用 `source.is_sysroot()` 限制 sealed marker 定义。
- Runtime-core 外溢基线：`runtime/c/scoop_runtime_api.h` allowlist 仍登记 `scoop_once_begin/end/guard_canonicalize`、`scoop_sync_mutex_*`、`scoop_sync_condvar_*`、`scoop_sync_once_*`、大量 `scoop_test_*`、以及 user-level `scoop_thread_spawn/join/yield/sleep_millis/current_id` 等符号；实现集中在 `runtime/c/scoop_once.c`、`runtime/c/scoop_sync.c`、`runtime/c/scoop_test.c`、`runtime/c/scoop_thread.c`，并由 `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` / `runtime_abi.rs` 等 active codegen 路径引用。
- Current-cone native-build 基线：`crates/scoopc/src/cone/manifest.rs` 已解析 `[native-build]` 的 `c-sources`、`c-flags`、`cxx-sources`、`cxx-flags`、`linker`、`link-flags`；`crates/scoop/src/commands/build.rs` 仅当 `front.input().is_explicit_cone()` 时编译当前 consumer cone 的 C/C++ sources 并透传 link flags；`tests/fixtures/run_pass_cone/c_sources_extern_call_basic/` 以 `native/add.c` + `@Extern(name = "cone_add_int")` 覆盖当前显式 cone 的 C source 编译/链接/run 能力。
- 后续触碰文件簇：P1 聚焦 `scoop` CLI/package/build deps、`scoopc::frontend`、`scoopc::cone::{archive,consume,scoopir}` 和 `typecheck_cone_archive` fixtures；P2 聚焦 `typecheck/annotations.rs`、`hir` ABI fields、LLVM call/declare ABI、parser/typecheck/build fixtures；P3 聚焦 `cone/manifest.rs`、`cone/package.rs`、frontend entry selection、syslib trust/intrinsic gates；P4 聚焦 `sysroot/` layout、`sysroot/mod.rs`、overlay fixtures；P5 聚焦 source cone graph、`ProjectInput/ProjectContext`、resolver/typecheck/codegen cone identity；P6 聚焦 auto dependency/prelude lists 和 `resolve/imports.rs`；P7 聚焦 `commands/build.rs`、`toolchain.rs` 和 all-loaded-cone native build fixtures；P8 聚焦 `runtime/c/` headers/sources、sysroot cone `native/` ownership 和 runtime allowlist tests；P9 聚焦 LLVM object/top-level init codegen、entry emission 和 once runtime 退场；P10 聚焦全仓旧模型 grep、`SCOOP_FULL_SPEC.md` 与最终验证。
- 核心决策：`P0-T01` 是基线冻结任务，不拆分，不引入 workaround，不修改阶段级计划；archive fixture 删除/改写留给 P1，sysroot/source cone/trust 语义迁移留给 P3-P6，native/runtime 迁移留给 P7-P9。
- 与计划/设计闭合：已覆盖 `PLAN.md` §3 和 `SYSROOT_RESHAPE_R2.md` §0-§2 所要求的旧 archive、sysroot privilege、runtime-core 外溢和 current-cone native-build 基线；未发现需要更新 `PLAN.md` 的阶段级依赖变化。
- 验证结果：`cargo build` 通过；`cargo test --all --all-targets` 通过；`cargo run -p scoop -- test` 通过（fixtures: ok，1558 checks）；额外运行 `cargo clippy --all-targets -- -D warnings` 通过。

## [DONE] P1-T01：禁用/删除 `scoop package` archive CLI

- 参考：`PLAN.md` §4。
- 目标：让当前 `.cone` archive 不再被呈现为可用发布能力。
- 当前实现入口：`crates/scoop/src/commands/package.rs`、CLI command dispatch、`crates/scoopc/src/cone/archive.rs`。
- 必须实现：
  1. `scoop package` 删除或改为稳定 diagnostic，说明 `.cone` archive 暂未支持、等待重设计。
  2. 如果保留 archive 写入代码作为 future isolated helper，不得由 normal CLI 暴露为可用功能。
  3. 更新相关 CLI tests / help 文案。
- 验证：`cargo test -p scoop package -- --nocapture` 或对应 CLI tests；`cargo build`。
- 完成条件：用户不能通过 normal CLI 生成看似可用的 `.cone` 依赖包。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoop/src/commands/package.rs`、`crates/scoop/src/cli.rs`；未修改 archive writer/reader future helper 代码，未改变 normal build dependency flow，未更新 `PLAN.md`。
- 核心决策：保留 `scoop package` 解析入口以提供稳定 diagnostic，但命令实现不再 canonicalize 输入、不创建输出目录、不调用 `write_cone_archive_v0`，因此 normal CLI 不能写出 `.cone` archive。
- Help / tests：`package` help 文案改为说明 `.cone` archive packaging 在 source-only cone redesign 期间暂不支持；原写 archive 的 package 单元测试改为断言稳定 unsupported diagnostic 和不写出 `.cone` 文件。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P1 中“`scoop package` 要么删除子命令，要么改为稳定 diagnostic”的要求；P1 后续 `.cone` build consume flow 与 archive fixtures 退场仍由 `P1-T02` / `P1-T03` 处理。
- 验证结果：`cargo fmt` 通过；`cargo build` 通过；`cargo test -p scoop package -- --nocapture` 首次与并行 build 争用后超时，重跑通过（6 passed, 0 failed）；`cargo clippy --all-targets -- -D warnings` 通过。

## [DONE] P1-T02：移除 normal build 的 `.cone` dependency flow

- 参考：`PLAN.md` §4。
- 目标：normal `scoop build/run` 不再从 `.cone` archive 读取 dependency API。
- 当前实现入口：`crates/scoop/src/commands/build/deps.rs`、`crates/scoopc/src/frontend.rs::ProjectContext`、`crates/scoopc/src/cone/consume.rs`。
- 必须实现：
  1. 移除 `SCOOP_CONE_PATH`、`cone/`、`deps/` 下 `.cone` 搜索在 normal build path 的使用。
  2. `ProjectContext` active path 不再携带 `Vec<ConeArchiveApi>`。
  3. `run_frontend` 不再调用 `inject_cone_dependency_public_api` 作为 normal dependency 机制。
  4. 保留 future/debug 代码时必须隔离，不能参与 build/run。
- 验证：grep active build path 中 `ConeArchiveApi`、`load_cone_archive_api`、`SCOOP_CONE_PATH`；`cargo build`。
- 完成条件：normal build/frontend 不依赖 archive API 注入。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoop/src/commands/build.rs`、删除 `crates/scoop/src/commands/build/deps.rs`、`crates/scoopc/src/frontend.rs`；未修改 `PLAN.md`，未删除 archive writer/reader future helper，未处理 archive fixture suite（留给 `P1-T03`）。
- 核心决策：normal `scoop build/run` 不再解析 manifest `[dependencies]` 为 `.cone` archive graph，不再搜索 `SCOOP_CONE_PATH` / `cone/` / `deps/`，`ProjectContext` 改为 input-only，`run_frontend` 不再接收 `Vec<ConeArchiveApi>` 或调用 `inject_cone_dependency_public_api`。
- 测试更新：删除原本证明 normal build 可消费 `.cone` 的 build 单元测试；新增 `build_cone_package_ignores_archive_dependencies_in_normal_path`，用 manifest dependency + 无效 `.cone` 文件证明 normal build 不读取 archive。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P1 中“移除 normal build `.cone` dependency flow”的要求；source-only dependency graph、source path dependency、archive fixture 删除/改写仍按后续 `P1-T03` / `P5-*` 任务推进。
- 验证结果：`cargo fmt` 通过；grep 确认 Rust 源码中无 `SCOOP_CONE_PATH` / `load_dependency_graph`，`crates/scoop/src/commands` 中无 `ConeArchiveApi` / `load_cone_archive_api` / `inject_cone_dependency_public_api`，`crates/scoopc/src/frontend.rs` 中无 archive API 符号；`cargo test -p scoop --bin scoop build_cone_package_ignores_archive_dependencies_in_normal_path -- --nocapture` 通过；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test -p scoop --bin scoop` 通过（121 passed）。
- 备注：一次未限定 `--bin scoop` 的 filtered test 在目标单元测试已通过后继续启动无关 integration test binary 并超过 120s；随后用限定命令重跑目标测试通过。

## P1-T03：删除或改写 archive fixtures 与 archive-only tests

- 参考：`PLAN.md` §4。
- 目标：移除 `.cone` archive fixture 对 active test suite 的影响。
- 当前实现入口：`tests/fixtures/typecheck_cone_archive/**`、`crates/scoop/src/fixtures/mod.rs` 中 archive fixture runner。
- 必须实现：
  1. 对每个 `typecheck_cone_archive` fixture 决定删除或改写为 source-only dependency fixture。
  2. 被测对象是 archive API 注入本身的 fixture 删除；被测对象是 cone boundary/type visibility 的 fixture 改写。
  3. 删除 archive fixture runner active path 或改为 future-disabled。
- 验证：`cargo run -p scoop -- test tests/fixtures/typecheck_cone_archive/` 不再作为 active archive suite；`cargo run -p scoop -- test`。
- 完成条件：仓库无 active fixture 依赖 `.cone` archive。

## P2-T01：`@Extern` 支持 `callingConvention` property

- 参考：`PLAN.md` §5、`SYSROOT_RESHAPE_R2.md` §6。
- 目标：外部函数导入的 machine calling convention 由 `@Extern(..., callingConvention = "...")` 表达。
- 当前实现入口：`crates/scoopc/src/typecheck/annotations.rs`、`hir::ExternFun.calling_convention`、parser annotation args。
- 必须实现：
  1. `@Extern` annotation parser 接受 `callingConvention` 字符串 property。
  2. `@Extern(abi = "c", callingConvention = "C"/"cdecl")` 保存到 `ExternFun.calling_convention`。
  3. `@Extern(abi = "scoop", callingConvention = ...)` 稳定拒绝。
  4. 外部函数上单独叠加 `@CallingConvention` 的旧用法改为拒绝或迁移到新语义，避免两处声明冲突。
- 验证：新增 parser/typecheck fixtures；`cargo test -p scoopc typecheck::annotations -- --nocapture`。
- 完成条件：外部 C symbol 的 symbol name、boundary ABI、machine calling convention 都由 `@Extern` 单点表达。

## P2-T02：有 body 的 `@CallingConvention` 生成 object-level native callable symbol

- 参考：`PLAN.md` §5、`SYSROOT_RESHAPE_R2.md` §6。
- 目标：Scoop 函数可生成 object-level native callable symbol，供 cone-local C 调用，但不表示 dylib/package export。
- 当前实现入口：`typecheck/annotations.rs`、`llvm/codegen/main/declare.rs`、ABI mangler/stable symbol 逻辑。
- 必须实现：
  1. `@CallingConvention(name = "...", convention = "C")` 可用于有 body 的普通 Scoop 函数。
  2. 该函数签名必须满足 C ABI / GC-free surface。
  3. codegen 生成指定 object symbol 和 LLVM callconv。
  4. 函数体仍按 managed Scoop codegen；native caller 必须已 GC attach，不自动 enter/leave native。
  5. `@Extern` 与 `@CallingConvention` 同时出现稳定拒绝。
- 验证：新增 build/IR fixture 检查 symbol/callconv；新增 negative fixtures 覆盖互斥和 GC ref 参数拒绝。
- 完成条件：cone-local C 可以链接调用 compiler 生成的 C-callable Scoop body function。

## P2-T03：支持 `Any as?` closed Pure function runtime cast

- 参考：`PLAN.md` §5、`SYSROOT_RESHAPE_R2.md` §6。
- 目标：补齐 `Any -> () -> Unit / Pure!` 方向的 runtime cast，支撑 thread entry Scoop shim。
- 当前实现入口：`typecheck/expr/infer.rs::check_function_type_cast_boundary`、`llvm/codegen/main/expr_op.rs::codegen_ref_is_instance_of_nonnull`、closure type descriptor 生成。
- 必须实现：
  1. 允许 `Any as? closed Pure function type`，至少 `Any as? () -> Unit / Pure!`。
  2. 继续拒绝任何 non-Pure/effectful function target。
  3. closure/function value runtime descriptor 必须区分函数签名，不能只用统一 `ScoopClosure` descriptor。
  4. `codegen_ref_is_instance_of_nonnull` 支持纯函数目标的 descriptor 检查。
  5. 成功 cast 后按普通 function value 调用路径调用。
- 验证：新增 run-pass/typecheck fixture：Pure closure -> Any -> `as? () -> Unit / Pure!` -> call；新增 effectful negative fixture。
- 完成条件：thread entry 可在 Scoop 代码中从 `GcHandle` 恢复 closure 并安全调用。

## P3-T01：`Cone.toml` 解析 `kind = bin/lib/syslib`

- 参考：`PLAN.md` §6。
- 目标：manifest 层正式表达 cone kind。
- 当前实现入口：`crates/scoopc/src/cone/manifest.rs`。
- 必须实现：
  1. 新增 `ConeKind` enum。
  2. 解析 `[cone].kind`，仅允许 `bin`、`lib`、`syslib`。
  3. 明确缺失 kind 策略。推荐主线要求显式 kind；如临时默认 `bin`，必须在完成记录中登记后续退场。
- 验证：manifest parser unit tests 覆盖三种 kind、非法 kind、缺失 kind。
- 完成条件：后续 loader/build 不再用目录形态猜测 cone kind。

## P3-T02：`lib/syslib` 无 entry point 加载规则

- 参考：`PLAN.md` §6。
- 目标：`lib` 和 `syslib` source package 不再要求 `src/main.scoop`。
- 当前实现入口：`crates/scoopc/src/cone/package.rs`、`frontend.rs` entry selection。
- 必须实现：
  1. `load_cone_source_package` 对 `lib/syslib` 只要求 `src/**/*.scoop` 非空。
  2. `bin` 仍要求 entry anchor，并最终由 entry package/main 选择入口。
  3. 依赖 `lib` 中存在 `main` 时不能被 consumer `bin` 误选。
- 验证：loader tests；source dependency negative fixture。
- 完成条件：source library cone 能作为 dependency 存在而不带 entry point。

## P3-T03：`syslib` path trust gate 与 intrinsic privilege gate

- 参考：`PLAN.md` §6、`SYSROOT_RESHAPE_R2.md` §8。
- 目标：把 sysroot-origin 特权迁移到 trusted `syslib` cone。
- 当前实现入口：`source.rs`、`typecheck/annotations.rs`、sealed marker typecheck gate、sysroot loader。
- 必须实现：
  1. `kind = "syslib"` 仅在 `sysroot/lib/<cone.fqn>/Cone.toml` 有效。
  2. 用户/外部 source dependency 声明 `syslib` 稳定拒绝。
  3. `@Intrinsic` / `@file:AllowIntrinsic` 权限改查所属 cone 是否 trusted syslib。
  4. sealed marker 等 sysroot-only gate 改为 syslib-only gate。
- 验证：用户 cone 自声明 syslib negative fixture；普通 lib 声明 intrinsic negative fixture；syslib 声明 intrinsic positive fixture。
- 完成条件：`SourceOrigin::Sysroot` 不再直接授予语言特权。

## P4-T01：重排 sysroot 到 `sysroot/lib/<cone>/src`

- 参考：`PLAN.md` §7。
- 目标：把现有内置 sysroot 文件迁移到真实 cone source layout。
- 当前实现入口：`sysroot/scoop.*/*.scoop`。
- 必须实现：
  1. 创建 `sysroot/lib/`。
  2. 迁移现有 `sysroot/scoop.core/*.scoop` 等到 `sysroot/lib/scoop.core/src/*.scoop`。
  3. 为每个 sysroot cone 添加 `Cone.toml`。
  4. 创建保留目录 `sysroot/bin/`、`sysroot/docs/`，但不参与加载。
- 验证：文件布局检查；`cargo build`。
- 完成条件：所有内置标准库源码位于 `sysroot/lib/<cone>/src`。

## P4-T02：sysroot loader 改为加载 `sysroot/lib/*/Cone.toml`

- 参考：`PLAN.md` §7。
- 目标：sysroot loader 不再递归扫描整个 `sysroot/**/*.scoop`。
- 当前实现入口：`crates/scoopc/src/sysroot/mod.rs`。
- 必须实现：
  1. loader 只发现 `sysroot/lib/*/Cone.toml`。
  2. 每个 sysroot cone 用 source cone package rules 加载。
  3. `sysroot/docs/*.scoop` 不会进入 compilation unit。
  4. 更新 sysroot unit tests。
- 验证：新增 `sysroot/docs/foo.scoop` 不加载测试；`cargo test -p scoopc sysroot -- --nocapture`。
- 完成条件：sysroot loading 基于 cone manifest，不基于盲递归 `.scoop` 搜索。

## P4-T03：sysroot overlay 迁移到 `overlay/lib/<cone>/...`

- 参考：`PLAN.md` §7。
- 目标：overlay 规则镜像新 sysroot layout。
- 当前实现入口：`sysroot/mod.rs` overlay merge、overlay fixtures。
- 必须实现：
  1. `SCOOP_SYSROOT_OVERLAY` 使用 `lib/<cone>/...` 结构。
  2. overlay 替换按 cone/file-relative path，而非全目录盲扫。
  3. 迁移旧 `.sysroot/` overlay fixtures。
- 验证：overlay tests；相关 fixture suite。
- 完成条件：旧 overlay 结构不再是 active path。

## P5-T01：引入 source cone graph 数据结构

- 参考：`PLAN.md` §8。
- 目标：用 source cone graph 替代 support sources + current project 的扁平输入。
- 当前实现入口：`frontend.rs::ProjectInput/ProjectContext`、`cone/package.rs`。
- 必须实现：
  1. 定义 graph node：root、manifest、kind、sources、native-build、trust status、dependency edges。
  2. graph 支持 sysroot auto cones、consumer cone、local source dependency cones。
  3. 保留 deterministic DAG order。
- 验证：unit tests 构造 graph 并检查 order/kind/source set。
- 完成条件：frontend/build 能接收 source cone graph 作为 authoritative input。

## P5-T02：支持本地 source path dependency fixtures

- 参考：`PLAN.md` §8。
- 目标：为 source-only dependency 提供最小可测试路径。
- 当前实现入口：manifest dependencies 解析、build context loader、fixture runner。
- 必须实现：
  1. 定义本轮最小 path dependency 语法或 fixture-only 约定。
  2. `bin` cone 可依赖本地 `lib` cone。
  3. dependency `lib` public API 可被 consumer 解析/typecheck/codegen。
  4. dependency internal/private 不可见。
- 验证：source dependency positive fixture、internal visibility negative fixture。
- 完成条件：不依赖 `.cone` 也能覆盖跨 cone source dependency。

## P5-T03：保留 cone identity/kind 到 resolver/typecheck/codegen

- 参考：`PLAN.md` §8。
- 目标：flatten 成 compilation unit 后仍不丢 cone 边界。
- 当前实现入口：`resolve::ConeId`、`IndexedFile`、`TypeEnv`、HIR lowering setup、LLVM codegen inputs。
- 必须实现：
  1. 每个 source/indexed file 带所属 cone id/kind。
  2. resolver/typecheck 可查询 symbol owner cone。
  3. codegen 可访问当前 callable/source 所属 cone，用于 stable identity、native ownership、init routine。
- 验证：unit tests 和 source dependency fixtures。
- 完成条件：visibility、syslib privilege、native ownership 不依赖文件路径猜测。

## P5-T04：生成 per-cone init routine 与 final system entry 调用骨架

- 参考：`PLAN.md` §8、§12。
- 目标：先建立按 cone DAG 调 init routine 的结构，为 P9 eager init 落地做准备。
- 当前实现入口：LLVM entry emission、top-level init metadata、object/top-level value codegen。
- 必须实现：
  1. 收集每个 linked cone 的 top-level initializer roots。
  2. 为每个 linked cone 生成独立 init routine stub。
  3. final system entry 按 source cone DAG 调用 init routines 后进入用户 `main`。
  4. 初期 routine 可为空或只承接现有逻辑，但结构必须稳定。
- 验证：LLVM IR fixture 检查 init routine 和 call order。
- 完成条件：P9 可在该骨架上替换 top-level lazy init。

## P6-T01：实现 auto dependency cone 列表

- 参考：`PLAN.md` §9。
- 目标：自动加载基础标准 cones，但不自动 import 它们的短名。
- 当前实现入口：source cone graph builder、sysroot loader。
- 必须实现：
  1. 初始 auto dependencies：`scoop.core`、`scoop.lang.string`、`scoop.collections`、`scoop.delegates`。
  2. `scoop.thread`、`scoop.sync`、`scoop.runtime.test` 不进入默认 auto dependency。
  3. 显式 dependency 与 auto dependency 去重。
  4. `scoop.unsafe` 优先作为 `scoop.core` manifest dependency；如临时 auto-load，记录退场。
- 验证：未显式依赖 thread/sync 时不可解析其短名且不链接其 native objects。
- 完成条件：auto dependency 只负责加载/编译/链接，不影响短名 import。

## P6-T02：实现 prelude package 列表并与 auto dependency 解耦

- 参考：`PLAN.md` §9。
- 目标：自动 `import scoop.core.*` 与 `import scoop.lang.string.*`，并证明非 prelude auto dependency 不自动短名可见。
- 当前实现入口：`resolve/imports.rs`。
- 必须实现：
  1. Prelude packages 初始为 `scoop.core`、`scoop.lang.string`。
  2. Prelude package 所属 cone 未加载时报 compiler configuration error。
  3. auto dependency 中非 prelude package 的短名不可见，显式 import 后可见。
- 验证：prelude positive fixture；non-prelude auto dependency short-name negative fixture。
- 完成条件：auto dependency 与 prelude package 语义完全分离。

## P7-T01：将 native-build 扩展到所有 loaded source cones

- 参考：`PLAN.md` §10。
- 目标：所有 loaded source cones 的 C/C++ sources 都会编译和链接。
- 当前实现入口：`commands/build.rs` native-build block、`toolchain.rs`。
- 必须实现：
  1. 遍历 source cone graph nodes，收集 native-build sources。
  2. C/C++ source path 按 owning cone root 解析。
  3. object 输出名带 cone identity，避免冲突。
  4. flags 只作用于 owning cone。
  5. 复用 current-cone native-build 能力，并推广到 all loaded cones。
- 验证：`bin` 依赖 `lib`，`lib/native/add.c` 被编译链接并运行。
- 完成条件：cone-local FFI 不再局限 consumer executable cone。

## P7-T02：dependency cone C++/link-flags/linker driver 覆盖

- 参考：`PLAN.md` §10。
- 目标：dependency cone 的 C++ 和 link flags 参与最终链接决策。
- 当前实现入口：`commands/build.rs` linker selection、`toolchain.rs`。
- 必须实现：
  1. 任一 loaded cone 有 `cxx-sources` 时默认 C++ linker driver 选择规则生效。
  2. 各 cone `link-flags` 按 dependency-topological order 稳定追加。
  3. duplicate symbol 等 linker 错误不被隐藏。
- 验证：dependency C++ fixture、dependency link-flags fixture。
- 完成条件：native build graph 从编译到最终链接完整覆盖 loaded cones。

## P8-T01：建立公开 `scoop_runtime.h` runtime core header

- 参考：`SYSROOT_RESHAPE_R2.md` §7、`PLAN.md` §11。
- 目标：给 cone-local C FFI 提供稳定 runtime core 入口，避免每个 FFI hack private runtime layout。
- 当前实现入口：`runtime/c/` headers、`toolchain.rs` include path、runtime allowlist。
- 必须实现：
  1. 新增公开 header 路径，例如 `runtime/c/include/scoop_runtime.h`。
  2. 暴露 GC thread attach/detach substrate。
  3. 暴露 GC handle/pin APIs、必要 alloc/type descriptor/string/array helper 的最小稳定 surface。
  4. 不暴露 Immix/heap/thread-list/platform/private root internals。
  5. Cone native build 自动加入公开 header include path。
- 验证：cone-local C fixture include `scoop_runtime.h` 编译通过。
- 完成条件：迁移后的 cone native C 不再 include runtime private headers，除明确 syslib/core substrate 例外。

## P8-T02：迁移 `scoop.runtime.test` native helpers

- 参考：`PLAN.md` §11。
- 目标：test-only helper 不再随普通 runtime core 链接。
- 当前实现入口：`runtime/c/scoop_test.c`、`sysroot/scoop.runtime.test`、runtime allowlist、runtime_gc fixtures。
- 必须实现：
  1. 将 `scoop_test_*` native code 迁到 `sysroot/lib/scoop.runtime.test/native/`。
  2. `scoop.runtime.test` 不进入默认 auto dependency。
  3. 测试 fixture 显式依赖或由 test harness 注入该 cone。
  4. runtime allowlist 删除迁出 symbols。
- 验证：runtime/test helper fixtures 通过；普通 hello-world 链接不含 `scoop_test_*`。
- 完成条件：test-only ABI 不再污染 normal runtime core。

## P8-T03：迁移 `scoop.sync` native implementation

- 参考：`PLAN.md` §11。
- 目标：Mutex/CondVar/user-visible Once native implementation 归 `scoop.sync` cone。
- 当前实现入口：`runtime/c/scoop_sync.c`、`sysroot/scoop.sync/sync.scoop`、`llvm/codegen/intrinsics/sync.rs`。
- 必须实现：
  1. 将 sync native C 迁到 `scoop.sync/native/`。
  2. 能用 C ABI + GC-free handle 表达的底层 primitive 优先转 C ABI。
  3. 必须 managed ABI 的 helper 说明原因，并使用 `scoop_runtime.h`。
  4. public Scoop API 尽量用 Scoop wrapper 表达，不把整个 API 都写成 FFI。
  5. runtime allowlist 删除 `scoop_sync_*` runtime-core exports。
- 验证：sync fixtures 通过；普通不依赖 sync 程序不链接 sync native objects。
- 完成条件：`scoop.sync` C 实现由 cone native-build 提供。

## P8-T04：迁移 `scoop.thread` native implementation 与 thread entry trampoline

- 参考：`PLAN.md` §11、`SYSROOT_RESHAPE_R2.md` §6-§7。
- 目标：user-level thread API 归 `scoop.thread` cone，runtime core 只保留 GC thread lifecycle substrate。
- 当前实现入口：`runtime/c/scoop_thread.c`、`sysroot/scoop.thread/thread.scoop`、`llvm/codegen/intrinsics/thread.rs`。
- 必须实现：
  1. 将 thread native C 迁到 `scoop.thread/native/`。
  2. C entry trampoline：attach current OS thread -> call `@CallingConvention` Scoop thread entry object symbol -> normal detach。
  3. Fatal trap 直接 abort，不承诺 detach/recovery。
  4. Closure/userdata 通过 GC handle raw token 传递，不裸传 GC ref。
  5. runtime core 保留并公开 GC thread attach/detach。
  6. runtime allowlist 删除 user-level `scoop_thread_spawn/join/sleep/currentId/yield` core exports。
- 验证：thread fixtures 通过；普通不依赖 thread 程序不链接 thread native objects。
- 完成条件：runtime core 不再实现 user-level thread API。

## P8-T05：迁移 string/native helper 边界并收窄 runtime allowlist

- 参考：`PLAN.md` §11。
- 目标：按 ownership 迁移 string-from-array 等 helper，并完成 runtime core allowlist 收口。
- 当前实现入口：`runtime/c/scoop_runtime.c` string helpers、`sysroot/scoop.lang.string`、runtime allowlist。
- 必须实现：
  1. 对 string helpers 做 ownership 分类：core substrate vs `scoop.lang.string` helper。
  2. 迁移属于 `scoop.lang.string` 的 native code 到该 cone。
  3. 保留 runtime core 中确属 canonical String object/type descriptor substrate 的部分。
  4. 更新 `scoop_runtime_api.h` allowlist 和 tests。
- 验证：string/lang fixtures、runtime export allowlist test、普通 link smoke。
- 完成条件：runtime core 只保留必要 string substrate，不承载高层 string cone FFI。

## P9-T01：Object once 改为 LLVM atomics + TLS init-frame stack

- 参考：`PLAN.md` §12。
- 目标：object lazy initialization 不再调用 runtime once helper。
- 当前实现入口：`llvm/codegen/object_init.rs`、`runtime/c/scoop_once.c`。
- 必须实现：
  1. object guard global zero-init，`0/1/2` 状态。
  2. codegen atomic load/cmpxchg/release-store helper。
  3. compiler-generated TLS object-init frame stack。
  4. `initializing` 时扫描 TLS stack，命中 same guard fatal trap，否则 busy wait。
  5. 检测 `A -> B -> A` 同线程间接环。
  6. `object_init.rs` 不再声明/调用 `scoop_once_begin/end`。
- 验证：IR 不含 `@scoop_once_begin/end`；self recursion、A/B cycle、cross-thread wait fixtures。
- 完成条件：object initialization 仅依赖 codegen atomics/TLS stack，不依赖 runtime once/sync/thread。

## P9-T02：Top-level `val` / annotated `var` 改为 per-cone eager init

- 参考：`PLAN.md` §12。
- 目标：top-level values 不再 lazy first-access，而在 final system entry 中按 cone DAG eager 初始化。
- 当前实现入口：`llvm/codegen/main/immut_value.rs`、entry emission、P5-T04 init routine 骨架。
- 必须实现：
  1. top-level immutable/mutable init path 不再调用 once helper。
  2. 每个 linked source cone 的 init routine 初始化本 cone roots。
  3. final system entry 按 source cone DAG 调用 init routines。
  4. 覆盖 consumer bin、explicit deps、auto deps、transitive deps。
  5. GC-containing global storage 在 initializer 可能触发 GC 前注册 root。
  6. GC-free top-level storage 可使用 `.bss` / `.data` / constant fold 等实现细节，但语义仍是 main 前完成。
- 验证：consumer/dependency/auto dependency top-level init order fixtures。
- 完成条件：top-level value access 不再触发 lazy init。

## P9-T03：top-level `var` annotation gate 与 storage 语义

- 参考：`PLAN.md` §12。
- 目标：明确 top-level mutable storage 必须显式选择 `@Global` 或 `@ThreadLocal`。
- 当前实现入口：typecheck annotations/properties、top-level var codegen、TLS/global storage codegen。
- 必须实现：
  1. 未标注 `@Global` / `@ThreadLocal` 的 top-level `var` 稳定拒绝。
  2. `@Global var` 使用 global storage 并在 cone init routine 初始化。
  3. entry thread 的 `@ThreadLocal var` 在 cone init routine 初始化 TLS storage。
  4. 明确非 entry thread 的 TLS 初始化策略；若本轮只覆盖 entry thread，文档和 diagnostic 必须写清。
- 验证：`@Global var`、`@ThreadLocal var` run-pass；unannotated var negative fixture。
- 完成条件：top-level var 初始化语义与 storage policy 明确。

## P10-T01：全仓旧模型残留审计与 spec 文档同步

- 参考：`PLAN.md` §13。
- 目标：删除或明确隔离旧模型残留，并更新语言/spec 文档。
- 当前实现入口：全仓搜索、`SCOOP_FULL_SPEC.md`、`docs/spec/**`。
- 必须实现：
  1. 搜索确认 normal build/frontend/codegen 不再使用 archive dependency injection。
  2. 搜索确认 `SourceOrigin::Sysroot` 不再直接授予 intrinsic/syslib 特权。
  3. 搜索确认 object init 不再调用 `scoop_once_begin/end`。
  4. 搜索确认 top-level value init 走 per-cone init routine + final system entry DAG。
  5. 搜索确认 runtime core 不再导出已迁移 feature-specific symbols。
  6. 更新 `SCOOP_FULL_SPEC.md` cone/sysroot/native boundary 章节。
- 验证：对应 grep 清单、`cargo run -p scoop_tools -- spec-fixtures check`。
- 完成条件：文档与实现一致，旧模型无 active path。

## P10-T02：全量验证与最终完成记录

- 参考：`PLAN.md` §13-§14。
- 目标：完成 R2 收尾，确认所有 fixture 与测试通过。
- 必须实现：
  1. 运行全量验证命令。
  2. 补齐所有任务完成记录。
  3. 如新增/修改 spec fixture code block，运行 sync/check。
  4. 更新 `PLAN.md` / `TODO.md` 当前状态，必要时按惯例归档。
- 验证：
  - `cargo fmt`
  - `cargo build`
  - `cargo test --all --all-targets`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo run -p scoop -- test`
  - `cargo clippy --all-targets -- -D warnings`
- 完成条件：R2 总完成判据全部满足，仓库无 failing fixture。
