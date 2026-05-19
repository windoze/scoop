# Sysroot Reshape R2 落地计划

> 生成时间：2026-05-19
> 设计基线：[`SYSROOT_RESHAPE_R2.md`](./SYSROOT_RESHAPE_R2.md)
> 当前状态：待开始
> 行号说明：下文以当前文件路径和符号名为准；后续若行号漂移，优先按文件路径、符号名、阶段编号和 fixture 名定位。

## 0. 范围与硬约束

- 本计划是 `SYSROOT_RESHAPE_R2.md` 的实现计划，允许修改 compiler/frontend/typecheck/HIR/MIR/codegen、driver/build、runtime C、sysroot layout、fixtures 和相关文档。
- 本轮目标是把 sysroot 从“递归扫描的一组特殊 `.scoop` 文件”改成“`sysroot/lib/*` 下的真实 source-only cones”。
- 本轮先删除或禁用现有 `.cone` archive 路径；不设计新的 binary distribution，不保留当前 `.cone` 格式兼容性。
- Cone kind 只包含 `bin`、`lib`、`syslib`。`clib`、`dylib`、plugin、registry、binary package 都不在本轮范围内。
- `syslib` 只允许位于 `sysroot/lib/<cone.fqn>/` 下；普通用户或外部 source dependency 自声明 `kind = "syslib"` 必须被拒绝。
- `SourceOrigin::Sysroot` 不再直接授予语言特权；`@Intrinsic`、`@file:AllowIntrinsic`、sealed marker 等特权必须来自所属 cone 的 trusted `syslib` 身份。
- `lib` 和 `syslib` cone 不要求 entry point；只有 `bin` cone 要求 entry point。
- auto dependency 和 prelude package 是两个独立机制，不能混用。
- Cone-local native sources 必须参与完整编译和链接；不能继续只编译当前 executable cone 的 C/C++ 源。
- 迁移 sysroot FFI 前必须先澄清 native boundary 注解：`abi` 表示边界 trampoline 形状，calling convention 表示机器层参数/返回布局。
- Runtime core 只能保留 GC、allocation、root/native transition、object/type descriptor、global root、必要语言 substrate 和 fatal trap substrate；thread/sync/test helper 等特定功能 FFI 要迁出 runtime core。
- 全局 object 初始化必须改为 codegen 生成 LLVM atomics；不依赖 `Mutex`、`CondVar`、`scoop.sync.Once` 或 `scoop_once_begin/end` runtime helper。
- Top-level `val` / annotated top-level `var` 初始化必须覆盖所有链接进最终程序的 source cones，并在进入用户 `main` body 前统一完成，不允许 lazy initialization 行为。
- 仓库内不得留下 failing fixture。过时 archive fixture 要删除或改写为 source-only cone fixture，并在对应阶段说明理由。

## 1. 当前基线

### 1.1 Sysroot 与 cone

- 当前 sysroot 目录为 `sysroot/scoop.core/`、`sysroot/scoop.thread/`、`sysroot/scoop.sync/` 等直接子目录。
- 这些目录没有 `Cone.toml`、`src/`、cone-local metadata，也没有 cone-local native source ownership。
- `crates/scoopc/src/sysroot/mod.rs` 当前递归收集 `sysroot/**/*.scoop`，并以 `SourceOrigin::Sysroot` 加载。
- `crates/scoopc/src/frontend.rs::load_default_support_sources` 当前把所有 sysroot `.scoop` 文件当 support sources 加入当前 compilation unit。
- `crates/scoopc/src/cone/package.rs` 当前要求 source cone 必须有 `src/main.scoop`，因此不能表达无 entry point 的 `lib` cone。

### 1.2 `.cone` archive

- `scoop package` 当前写 tar 形式 `.cone` archive。
- `.cone` 内容包括 `Cone.toml`、`api.scoopir`、`ANNOTATION_CLASSES.json`、`SYMBOL_VISIBILITY.json`、`PRE_SPECIALIZE.json` 和 `SOURCES_SHA256`。
- `crates/scoop/src/commands/build/deps.rs` 当前通过 `.cone` 搜索路径加载依赖，并把 `ConeArchiveApi` 注入 resolver/typecheck。
- `tests/fixtures/typecheck_cone_archive/**` 覆盖的是 archive API 注入，不是 source-only cone DAG。

### 1.3 Native build 与 runtime C

- `Cone.toml` 已支持 `[native-build].c-sources/c-flags/cxx-sources/cxx-flags/link-flags`。
- `crates/scoop/src/commands/build.rs` 当前只编译当前显式 cone 的 native sources。
- `tests/fixtures/run_pass_cone/c_sources_extern_call_basic/` 已覆盖当前显式 cone 的 C source FFI 编译与链接能力。
- 显式 cone build 仍会把 `runtime/c/*.c` 全量编译到 executable；其中包括 `scoop_thread.c`、`scoop_sync.c`、`scoop_test.c`。
- `runtime/c/scoop_runtime_api.h` allowlist 仍登记 user-level thread/sync/test helper symbols。

### 1.4 Thread/sync、object once 与 top-level 初始化

- `scoop.thread` 和 `scoop.sync` sysroot surface 当前是 `@Intrinsic`，codegen 通过 FQN 特例 lower 到 `scoop_thread_*` 和 `scoop_sync_*` runtime symbols。
- `scoop.sync.Once` 当前在 `runtime/c/scoop_sync.c` 用 mutex + condvar 实现，是用户可见 sync primitive。
- object/top-level immutable 初始化当前由 codegen 调 `scoop_once_begin` / `scoop_once_end`，实现位于 `runtime/c/scoop_once.c`，虽然用 C atomics，但仍是 runtime ABI 并依赖 platform thread helper。
- R2 目标中，只有 `object` 需要 atomic once；top-level `val` / annotated `var` 改为覆盖所有 linked cones 的 main body 前 eager init。

## 2. 阶段总览

| 阶段 | 名称 | 目标 |
| --- | --- | --- |
| P0 | Baseline freeze | 锁定现状、补充审计测试、确认删除/迁移清单 |
| P1 | Archive 退场 | 删除或禁用现有 `.cone` archive build/consume 路径 |
| P2 | Native boundary annotations | 明确 `@Extern` / `@CallingConvention` 的 ABI 与 calling convention 分工 |
| P3 | Cone kind | manifest 支持 `bin/lib/syslib`，并落实 entry/syslib path gate |
| P4 | Sysroot layout | 迁移到 `sysroot/lib/<cone>/Cone.toml + src/`，loader 不再递归扫整个 sysroot |
| P5 | Source cone graph | frontend/build 改为加载 source cone DAG，保留 cone identity/kind |
| P6 | Auto dependency + prelude | 拆分自动依赖 cone 列表和自动 import package 列表 |
| P7 | Native build graph | 所有加载的 source cones 都能贡献 C/C++ objects 和 link flags |
| P8 | Runtime FFI migration | thread/sync/test helper 等迁到 owning cone，runtime core 收窄 |
| P9 | Object once + eager top-level init | object 初始化改为 codegen LLVM atomics；top-level values 改为 main 前 eager init |
| P10 | Final verification | 全量 fixture、archive 残留、runtime ABI 和文档收尾 |

阶段之间原则上按顺序推进；P8 内部的具体 FFI 迁移可按 cone 分 PR，但必须建立在 P7 的 native build graph 之后。

## 3. P0：Baseline freeze

目标：在删除旧 archive 和重排 sysroot 前，固定现状与验证入口，避免迁移中混淆“旧模型删除”和“新模型回归”。

要求：

- 记录当前根目录旧计划已归档，`PLAN.md` 与 `SYSROOT_RESHAPE_R2.md` 成为本轮主线。
- 列出现有 archive 入口：`scoop package`、`ConeArchiveApi`、`load_cone_archive_api`、`inject_cone_dependency_public_api`、`typecheck_cone_archive` fixtures、`api.scoopir` export/consume 路径。
- 列出现有 sysroot 特权入口：`SourceOrigin::Sysroot`、`SourceFile::is_sysroot()`、`@Intrinsic` / `@file:AllowIntrinsic` gate、sealed marker sysroot-only gate。
- 列出现有 runtime-core 外溢 symbols：`scoop_thread_*` user API、`scoop_sync_*`、`scoop_test_*`、`scoop_once_*`。
- 为后续 native FFI migration 添加导出符号审计基线，确保每次迁移都能说明 runtime allowlist 的减少。

验收：

- `cargo build` 通过。
- `cargo test --all --all-targets` 通过或记录与本计划无关的既有失败。
- `cargo run -p scoop -- test` 通过或记录与本计划无关的既有失败。
- P0 产出一份短完成记录，列出 P1-P10 需要触碰的文件簇。

## 4. P1：Archive 退场

目标：把当前未规划完善的 `.cone` archive 功能从 active build path 中移除，回到 source-only 依赖模型的前置状态。

要求：

- `scoop package` 要么删除子命令，要么改为稳定 diagnostic，说明 `.cone` archive 暂未支持。
- normal `scoop build/run` 不再从 `SCOOP_CONE_PATH` 或 `cone/`、`deps/` 搜索 `.cone`。
- `ProjectContext` / frontend active path 不再依赖 `ConeArchiveApi` 注入 public API。
- 删除或隔离 `api.scoopir` archive consume 路径；如果保留 ScoopIR export 作为调试/未来功能，不能参与 normal build。
- 删除或重写 `tests/fixtures/typecheck_cone_archive/**`，保留测试意图的必须迁到 source-only dependency fixtures。
- 清理文档中把当前 `.cone` 描述为可用 dependency 的说法，转为 future work。

验收：

- normal build path 中 grep `load_cone_archive_api`、`ConeArchiveApi`、`SCOOP_CONE_PATH` 不应命中 active build/frontend dependency flow。
- archive fixture 不再作为 active suite 依赖 `.cone`。
- `cargo test -p scoopc cone:: -- --nocapture` 中 archive-only tests 要么删除，要么明确标记为 future isolated tests。
- `cargo run -p scoop -- test` 通过。

## 5. P2：Native boundary annotations

目标：在迁移 sysroot FFI 前，修正并明确 `@Extern`、`@CallingConvention` 和未来 `@Export` 的语义边界。

设计规则：

- `abi` 是边界 trampoline 形状，决定 GC 状态、root 暴露、hidden args、effect permeability、managed/native 进入方式。
- calling convention 是机器层调用布局，决定参数/返回位置、栈清理、LLVM callconv、sret 等目标 ABI 细节。
- `@Extern` 用于导入外部符号，拥有 `name`、`abi` 和可选 `callingConvention`。
- `@CallingConvention` 用于有 body 的 Scoop 函数，生成 object-level native callable symbol；它拥有 `convention` 和可选 object symbol `name`，但不拥有 boundary `abi`。
- `@Extern` 和 `@CallingConvention` 互斥。
- `@CallingConvention` 不表示未来 dylib/so/package export；未来 `@Export` 可独立拥有 `name`、`abi`、`callingConvention`。
- 当前 `@CallingConvention` body 函数签名必须满足 C ABI / GC-free surface；函数体仍是 managed Scoop code，native caller 必须已满足 runtime precondition，例如当前线程已 GC attach。

要求：

- `@Extern` annotation parser 增加 `callingConvention` property，替代外部函数上单独叠加 `@CallingConvention` 的需求。
- 对 `@Extern(abi = "scoop", callingConvention = ...)` 给出稳定拒绝。
- 对同一函数同时写 `@Extern` 和 `@CallingConvention` 给出稳定拒绝。
- 对有 body 的 `@CallingConvention` 函数执行 C ABI / GC-free signature gate。
- Codegen 为有 body 的 `@CallingConvention` 函数生成 object-level native callable symbol，不改变 Scoop visibility/import，不表示产物级 export。
- 明确当前 `@CallingConvention` entry 假设 caller 已 GC attach；不自动 enter/leave native。
- 补齐 `Any as? closed Pure function type`，至少支持 `Any as? () -> Unit / Pure!`，并继续拒绝任何 effectful function target。
- 为 function/closure runtime cast 增加 signature-specific runtime descriptor，不能只用统一 `ScoopClosure` descriptor。

验收：

- 新增 parser/typecheck fixture：`@Extern(..., callingConvention = "C")` 可解析并用于 C ABI import。
- 新增 negative fixture：`@Extern` 与 `@CallingConvention` 同时出现被拒。
- 新增 negative fixture：`@CallingConvention` 函数使用 GC ref 参数被 C ABI gate 拒绝。
- 新增 build/IR fixture：有 body 的 `@CallingConvention(name = ..., convention = "C")` 函数生成预期 object symbol 和 callconv。
- 新增 run-pass/typecheck fixture：closed `Pure!` closure 擦除到 `Any` 后可 `as? () -> Unit / Pure!` 恢复并调用。
- 新增 negative fixture：`Any as? () -> Unit / Raise<...>` 或其它 effectful function target 被拒。

## 6. P3：Cone kind

目标：让 manifest 和 source package loader 表达 `bin/lib/syslib`，并把 entry point 与 trust 规则移出隐式路径假设。

要求：

- `ConeManifest` 解析 `[cone].kind`，允许值仅为 `bin`、`lib`、`syslib`。
- 新建或复用 enum 表达 cone kind，避免后续用字符串散落判断。
- 对缺失 `kind` 的策略要明确。推荐 R2 主线要求显式 `kind`；如为兼容现有 fixture 临时默认 `bin`，必须有后续清理任务。
- `load_cone_source_package` 允许 `lib/syslib` 无 `src/main.scoop`。
- `bin` 仍要求可解析 entry point；entry source 可继续默认 `src/main.scoop`，但最终入口由 package main / entry-package 规则确定。
- `syslib` path gate：只有 `sysroot/lib/<cone.fqn>/Cone.toml` 下可生效；其它位置声明 `syslib` 报稳定 diagnostic。
- 普通 `lib` 下声明 `@Intrinsic` 必须报错；`syslib` 下允许。

验收：

- 新增 manifest parser tests 覆盖三种 kind、非法 kind、缺失 kind 策略。
- 新增 loader tests 覆盖 `lib` 无 main 成功、`bin` 无 main 失败。
- 新增 diagnostic fixture 覆盖用户 cone 自声明 `syslib` 被拒。
- 新增 typecheck/resolve fixture 覆盖普通 `lib` 不能声明 intrinsic。

## 7. P4：Sysroot layout

目标：把内置 cones 移到 `sysroot/lib/`，每个内置 cone 拥有真实 `Cone.toml` 和 `src/` 目录。

目标结构：

```text
sysroot/
├── lib/
│   ├── scoop.core/
│   │   ├── Cone.toml
│   │   └── src/*.scoop
│   ├── scoop.lang.string/
│   │   ├── Cone.toml
│   │   └── src/*.scoop
│   └── ...
├── bin/
└── docs/
```

要求：

- 迁移现有 `sysroot/scoop.*/*.scoop` 到 `sysroot/lib/scoop.*/src/`。
- 为每个 sysroot cone 添加 `Cone.toml`，kind 初始按设计保守分类。
- `Sysroot::default_path()` 仍指向 `sysroot/` umbrella，但 loader 只扫描 `sysroot/lib/*/Cone.toml`。
- `sysroot/bin`、`sysroot/docs` 可创建保留目录，但不参与 source loading。
- `SCOOP_SYSROOT_OVERLAY` 改为镜像 `lib/<cone>/...` 结构；旧 overlay fixture 要迁移。
- 不再递归扫描任意 `sysroot/**/*.scoop`。

验收：

- `Sysroot::load_from(Sysroot::default_path())` 通过并加载预期 cones。
- 新增测试证明 `sysroot/docs/foo.scoop` 不会被加载。
- 新增 overlay 测试证明 `overlay/lib/scoop.core/src/core.scoop` 可替换 base file。
- 所有现有 sysroot/import/typecheck/codegen fixture 通过或在同 PR 完成路径更新。

## 8. P5：Source cone graph

目标：frontend/build 从“support sources + 当前 project sources”演进为“source cone graph”，但仍保持单 compilation unit codegen 能力。

要求：

- 引入 source cone graph 数据结构，节点包含 cone root、manifest、kind、source set、native build config、trust status。
- 每个 `SourceFile` 或 indexed file 能追踪所属 cone id 和 cone kind。
- Resolver/typecheck/codegen 能区分 sysroot auto cones、explicit dependencies、consumer cone。
- `ProjectContext` 输入从单个 `ProjectInput + deps(Vec<ConeArchiveApi>)` 改为 source cone graph。
- 依赖图初期至少支持 sysroot auto dependencies 和本地 path dependencies，用于 fixture 和开发。
- Cone visibility、public/internal/private 的现有逻辑不能因为 graph flatten 丢失边界。
- Entry point 只能从 consumer `bin` cone 选择；依赖 `lib/syslib` 中的 `main` 不能被误选。
- 收集每个 linked source cone 的 top-level initializer roots。
- 为每个 linked source cone 生成独立 cone init routine；routine 内只初始化本 cone 的 top-level `val`、`@Global var` 和 entry-thread `@ThreadLocal var`。
- 最终系统 entry point 按 source cone DAG 顺序逐个调用 cone init routine，全部完成后再进入用户 `main` body。

验收：

- 新增 source dependency fixture：`bin` 依赖本地 `lib` 并调用 public 函数。
- 新增 negative fixture：`bin` 不能把依赖 `lib` 的 `main` 当 entry point。
- 新增 internal visibility fixture：依赖 cone 的 internal symbol 不可见。
- `cargo run -p scoop -- test tests/fixtures/run_pass_cone/` 通过或已迁移到新 source graph fixture layout。

## 9. P6：Auto dependency 与 prelude packages

目标：明确“自动加载的 cone”和“自动 import 的 package”是两个独立列表。

初始 auto dependency 推荐：

- `scoop.core`
- `scoop.lang.string`
- `scoop.collections`
- `scoop.delegates`

初始 prelude packages：

- `scoop.core`
- `scoop.lang.string`

要求：

- auto dependency 列表让对应 cone 参与 resolver/typecheck/codegen/native build/link。
- prelude package 列表只影响 import table 的默认 star imports。
- prelude package 所属 cone 未加载时，报 compiler configuration error，不能静默退化。
- 显式 dependencies 与 auto dependencies 按 cone identity 去重。
- `scoop.thread`、`scoop.sync`、`scoop.runtime.test` 不进入默认 auto dependency。
- `scoop.unsafe` 优先作为 `scoop.core` manifest dependency；如短期实现困难，可以作为非 prelude auto dependency 过渡，并在 TODO 中标记退场。

验收：

- 无显式 import 时可直接使用 `scoop.core` 和 `scoop.lang.string` prelude names。
- auto dependency 中非 prelude package 的短名不可见，显式 import 后可见。
- 未显式依赖 `scoop.thread` / `scoop.sync` 的程序不能解析其短名。
- `scoop.thread` / `scoop.sync` 不因 auto dependency 进入普通程序链接输入。

## 10. P7：Native build graph

目标：所有加载的 source cones 都能贡献 native objects 和 link flags，实现 cone 专属 FFI 的完整编译到链接链路。

要求：

- Native build 输入从当前 executable cone 扩展为 source cone graph 全部节点。
- C/C++ 源路径按 owning cone root 解析。
- C/C++ object 输出命名带 cone identity，避免不同 cone 源文件 stem 冲突。
- C/C++ flags 只作用于 owning cone 声明的对应 sources。
- Link flags 按 dependency-topological order 稳定追加。
- C++ source 存在时最终 linker driver 选择规则要覆盖 dependency cone，不只覆盖 consumer cone。
- Linker duplicate symbol 等错误直接暴露，不做静默重命名。
- Runtime C object 编译应与 cone native objects 分开，便于后续 runtime core 收窄。
- 记录并复用现有 current-cone native-build 能力；本阶段重点是把该能力推广到所有 loaded source cones。

验收：

- 新增 source dependency run fixture：`bin` 依赖 `lib`，`lib` 声明 `native/add.c`，`bin` 调 `@Extern` 成功链接运行。
- 新增 fixture 覆盖 dependency cone 的 `cxx-sources` 会选择 C++ linker driver。
- 新增 fixture 覆盖 dependency cone 的 `link-flags` 被传递。
- `cargo run -p scoop -- test tests/fixtures/run_pass_cone/` 中现有 native-build fixtures 通过或完成迁移。

## 11. P8：Runtime FFI migration

目标：把特定功能 FFI 从 runtime core 移到 owning cone，runtime core allowlist 收窄。

推荐子阶段：

| 子阶段 | 迁移对象 | 目标 |
| --- | --- | --- |
| P7-A | `scoop.runtime.test` | test-only helper 不再进入普通程序 runtime |
| P7-B | `scoop.sync` | `Mutex`、`CondVar`、user-visible `Once` 迁到 cone native source |
| P7-C | `scoop.thread` | user-level thread API 迁到 cone native source |
| P7-D | string helper 边界 | string-from-array 等归属 `scoop.lang.string` 或 core substrate |
| P7-E | runtime ABI audit | `scoop_runtime_api.h` 删除已迁出的 symbols |

要求：

- 每个迁移对象都先转换 sysroot surface：能用 `@Extern(abi = "scoop")` 表达的不要保留 `@Intrinsic`。
- 只有 compiler 确实生成实质代码的 callsite 保留 `@Intrinsic`，并且只能位于 `syslib` cone。
- `scoop.sync.Once` 作为用户 API 的实现可暂时继续使用 mutex/condvar，但不得参与 core object initialization。
- `scoop.thread.threadSpawn` 如果仍需要 closure ABI adapter，可暂时保留 syslib intrinsic；其 native implementation 仍应归 `scoop.thread`。
- `scoop.runtime.test` 不进入普通 auto dependency；测试 fixture 必须显式依赖或通过 test harness 注入。
- Runtime allowlist 每迁出一组 symbols 就同步更新并跑 allowlist 测试。
- runtime core 保留 GC thread lifecycle substrate，例如 `scoop_gc_thread_attach_current()` / `scoop_gc_thread_detach_current()`，供 OS-created thread entry trampoline 使用。
- thread entry trampoline 只负责 attach -> 调用 `@CallingConvention` Scoop thread entry object symbol -> normal return detach；fatal trap 直接 abort 进程，不承诺 detach 或 recovery。
- 当前 `runtime/c/` 中实际属于各 sysroot cone 的 C 代码必须在本阶段迁移到对应 cone 的 `native/` 目录，并通过该 cone 的 `native-build` metadata 编译链接。
- 不允许把 `scoop_thread.c`、`scoop_sync.c`、`scoop_test.c` 这类 feature-specific C 实现长期留在 runtime core，仅因为它们历史上位于 `runtime/c/`。

验收：

- 普通 hello-world 程序最终链接输入不包含 `scoop_sync_*`、user-level `scoop_thread_*`、`scoop_test_*` migrated symbols。
- 显式依赖 `scoop.sync` 的程序仍能运行 mutex/condvar/once fixtures。
- 显式依赖 `scoop.thread` 的程序仍能运行 thread fixtures。
- runtime export allowlist test 通过，且迁出 symbols 不再作为 runtime core export。

## 12. P9：Codegen object once + eager top-level init

目标：删除 core object init 对 `scoop_once_begin/end` runtime ABI 的依赖，改为 codegen 内联 LLVM atomic once state machine；同时把 top-level `val` / annotated top-level `var` 从 lazy init 改为 `main` body 前 eager init。

Object 初始化语义：

- Guard state：`0 = uninitialized`、`1 = initializing`、`2 = initialized`。
- 一个线程通过 atomic cmpxchg 从 `0` 切到 `1` 并执行 init。
- 初始化线程先写 object/global storage，再 release-store `2`。
- 其他线程看到 `1` 时 busy loop acquire-load guard，直到看到 `2`。
- 其他线程不能读取半初始化 storage。
- 同线程递归访问同一正在初始化的 object 是 fatal trap，不是 user-visible effect。
- 同线程递归检测通过 compiler-generated TLS object-init frame stack 完成，不通过 OS thread id 或 runtime helper。
- object init winner 线程把当前 object guard 链入 TLS init-frame stack，initializer body 执行期间保持 frame 存在，object publish 后再恢复旧 TLS top。
- 访问到 `initializing` 时扫描 TLS init-frame stack；命中同一 guard 表示同线程递归或间接环，fatal trap；未命中则 busy wait。
- Top-level `val` 不使用 once/lazy 初始化协议。
- Top-level `val` initializer 必须在进入用户 `main` body 前运行完成。
- Top-level `var` 行为与 top-level `val` 一致，但必须显式标注 `@Global` 或 `@ThreadLocal`。
- `@Global var` 在 global storage 中初始化；entry thread 的 `@ThreadLocal var` 在 TLS 中初始化，且发生在用户 `main` body 前。
- 未标注 `@Global` / `@ThreadLocal` 的 top-level `var` 必须由 frontend/typecheck 稳定拒绝。
- Object 初始化中可以读取同 object 已完成初始化的前序 property，不能读取后序 property，不能通过普通 object access 访问自身 singleton。

要求：

- 在 LLVM codegen 中实现 object once guard atomic load/cmpxchg/store helper。
- 在 LLVM codegen 中实现 compiler-generated TLS object-init frame stack helper，用于检测同线程递归和 `A -> B -> A` 间接环。
- `object_init.rs` 不再声明/调用 `declare_runtime_once_begin/end`。
- top-level immutable/mutable init path 不再调用 once helper；改由 cone init routine 统一调用。
- 每个 linked source cone 生成一个独立 init routine；该 routine 不递归初始化依赖 cone。
- final system entry point 必须覆盖 consumer `bin` cone、显式依赖 cone、auto dependency cones 和所有传递 source cones；不能只初始化当前 bin cone。
- final system entry point 的 cone init 调用顺序先按 source cone DAG dependency topology，再由各 cone init routine 内部按 deterministic source/item order 初始化本 cone roots。
- 初始化期 same-object earlier property access 走 direct initialized storage path，不触发 ordinary object access guard。
- 后序 property/self object access 至少要有静态 reject；若暂时无法静态覆盖全部，必须有 fatal trap，不能返回零值。
- 删除或退场 `runtime/c/scoop_once.c` 中 object init 专用 runtime ABI；如 `scoop_once_guard_canonicalize` 仍有动态链接需求，单独记录 future work，不能阻塞 core once helper 退场。
- `scoop.sync.Once` 保持独立，不参与本阶段语义。

验收：

- LLVM IR/unit test 中 object/top-level init 不再包含 `@scoop_once_begin` / `@scoop_once_end` 引用。
- 新增 run-pass fixture：top-level `val` initializer 在 `main` body 第一条用户语句前已经执行。
- 新增 run-pass fixture：dependency `lib` cone 的 top-level `val` initializer 也在 consumer `main` body 前执行。
- 新增 run-pass fixture：auto dependency/sysroot cone 的必要 top-level initializer 会进入 pre-main init phase。
- 新增 run-pass fixture：`@Global var` initializer 在 `main` body 前执行，后续读写使用 global storage。
- 新增 run-pass fixture：entry thread 的 `@ThreadLocal var` initializer 在 `main` body 前执行，后续读写使用 TLS storage。
- 新增 negative fixture：未标注 `@Global` / `@ThreadLocal` 的 top-level `var` 被拒绝。
- 新增 run-pass fixture：同 object 中 `val b = a + 1` 可运行。
- 新增 negative/fatal fixture：同 object 中读取后序 property 被拒或稳定失败。
- 新增 negative/fatal fixture：object initializer 通过 ordinary `O` / `O.a` 自递归访问被拒或稳定失败。
- 新增 negative/fatal fixture：两个 object 在同一线程初始化中形成 `A -> B -> A` 间接环时稳定失败，不 deadlock。
- 新增 cross-thread fixture：其他线程访问 initializing object 会等待 initialized 后读取结果，不读半初始化值。

## 13. P10：Final verification and cleanup

目标：确保 R2 完成后没有旧模型残留，且从 source cone 编译到 native link 的主线完整。

要求：

- 根目录 `PLAN.md` 和 `SYSROOT_RESHAPE_R2.md` 与实现状态一致；完成后按惯例更新/归档下一轮文档。
- `SCOOP_FULL_SPEC.md` 的 cone/sysroot 章节更新为 `sysroot/lib`、source-only dependency、`bin/lib/syslib`、auto dependency/prelude 的新语义。
- 删除或明确隔离 `.cone` archive 文档、fixtures、CLI、tests 的所有 active path。
- 搜索确认 normal build/frontend/codegen 不再使用 archive dependency injection。
- 搜索确认 `SourceOrigin::Sysroot` 不再直接授予 intrinsic/syslib 特权。
- 搜索确认 object init 不再调用 `scoop_once_begin/end`。
- 搜索确认 top-level value init 不再走 lazy first-access path，而是在 per-cone init routine + final system entry DAG call sequence 中执行。
- 搜索确认 runtime core 不再导出已迁移的 feature-specific symbols。

全量验收：

- `cargo fmt` 通过。
- `cargo build` 通过。
- `cargo test --all --all-targets` 通过。
- `cargo run -p scoop_tools -- spec-fixtures check` 通过。
- `cargo run -p scoop -- test` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。

## 13. 总完成判据

R2 完成时必须同时满足：

- `sysroot/lib/*/Cone.toml` 是标准库加载入口；compiler 不再递归扫描整个 `sysroot/**/*.scoop`。
- 所有内置 library surfaces 都属于真实 source cones。
- `bin/lib/syslib` kind 生效，`lib/syslib` 无 entry point 要求，`syslib` path trust gate 生效。
- 普通用户 cone 无法获得 `syslib` 特权。
- Auto dependency 与 prelude package 分离，并有 fixture 覆盖二者差异。
- `.cone` archive 不参与 active dependency/build path。
- Source cone dependency 可从解析、typecheck、codegen 到 native link/run 完整工作。
- Cone-local native C/C++ sources 对所有 loaded cones 生效。
- Runtime core 不再承载 thread/sync/test helper 等可迁出的 feature-specific FFI。
- Object initialization 由 codegen atomics 实现，不依赖 sync/thread/runtime once helper。
- Top-level `val` / annotated top-level `var` 在 `main` body 前 eager 初始化，覆盖所有 linked source cones，不依赖 lazy first-access once helper。
- 全量测试和 fixture suite 通过。

## 14. 后续非本轮事项

- 重新设计 `.cone` binary archive、precompiled objects、ScoopIR distribution 和 package registry。
- 设计 `clib`、`dylib`、plugin、hosted SDK 等 cone kind。
- 配置化 auto dependency 和 prelude package 列表。
- 为 cone-local native sources 设计稳定公开 runtime C header 契约。
- 进一步把 `scoop.thread.threadSpawn` / `scoop.sync.Once.run` 从 compiler intrinsic 降级为普通 FFI 或更通用 callable ABI。
