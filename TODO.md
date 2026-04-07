# TODO（Scoop：近期任务清单）

> 生成时间：2026-04-07  
> 说明：本文件是新的短版 TODO，只记录“接下来要做的新任务”。历史任务与已完成事项请看 `TODO-1.md` / `PLAN-1.md`。

## 约定

- 状态：
  - `[TODO]`：可立即实现与验收
  - `[BLOCKED]`：依赖未满足（例如缺文件/缺前置能力）
  - `[DONE]`：已完成（短版 TODO 一般不搬运历史 DONE）
- 每个任务包含：**描述 / 目标 / 验收 / 依赖**。

常用验收命令：

```bash
cargo test --all
cargo run -p scoop_tools -- spec-fixtures check
cargo run -p scoop -- test
```

LLVM 端到端（本机需 `clang` + `llvm-config`）：

```bash
cargo run -p scoop --features llvm -- test
```

---

## T01：LLVM 工具链对齐（LLVM 21 / Rust stable）

### T0101 [DONE] LLVM 21：将后端基线从 LLVM 18 迁移到 LLVM 21（对齐 Rust stable）
- 描述：将 LLVM 后端开发/测试基线升级到 LLVM 21，避免当前“系统 LLVM / Homebrew LLVM 18 / 机器差异”导致的行为漂移，并为后续优化 pipeline 与 GC/statepoint 相关 pass 的稳定性提供统一前提。
- 目标：
  - 依赖升级：更新 `inkwell`/`llvm-sys`（如有）到支持 LLVM 21 的组合，并固定选择策略（prefer Rust stable 对齐）。
  - 构建入口一致：`cargo build/test` 与 `cargo run -p scoop --features llvm -- test` 在开发机上只依赖 LLVM 21（包括 `llvm-config`）。
  - 文档与诊断：明确“需要的 LLVM 版本/安装方式/常见错误”，并让错误提示能指出版本不匹配。
  - pass 稳定性复核：在 LLVM 21 下复核 `rewrite-statepoints-for-gc` 管线；`place-safepoints` 继续默认关闭，除非在 LLVM 21 下单独验证稳定可用。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`（使用 LLVM 21）
- 依赖：无

---

## T11：Cone（改进项吸收）

### T1119 [DONE] Cone：产出工程化改进设计（目录结构 / build 产物 / profile / 增量路线）
- 描述：为 CONE 项目工程化体验补齐“目标状态设计”，覆盖目录结构、build 产物布局、profile 行为与增量构建路线。
- 目标：把“接下来要实现什么”写清楚，并为未来 cross compile 预留目录结构（但不要求立即实现 cross compile）。
- 验收：
  - 仓库根目录新增 `CONE-IMPROVEMENTS.md`，并覆盖本次需求列出的全部要点。
- 依赖：无

### T1120 [DONE] `scoop new`：生成 `.gitignore` + `main.scoop` 默认 `println`
- 描述：更新 `scoop new <project-name>` 生成的 CONE 项目结构，使其符合 `CONE-IMPROVEMENTS.md`：
  - 生成 `.gitignore`（至少忽略 `/build/` 等）
  - 自动生成的 `src/main.scoop` 包含 `println("Hello, Scoop!")`（或等价可观察输出）
- 目标：只改 project scaffold；不引入新的语言/stdlib 依赖。
- 验收：
  - `cargo test -p scoop`：新增/更新单测覆盖 `.gitignore` 与 `println` 模板内容。
  - （可选）新建项目后 `scoop run` 能输出固定字符串（与 T1123 一起验收）。
- 依赖：无（已存在 `scoop new`；但后续端到端 run 验收建议在 LLVM 21 基线上做）

### T1121 [DONE] build 输出目录：统一落到项目内 `build/<profile>/…`（并预留 `build/<target>/<profile>`）
- 描述：让 CONE 项目的 build 产物不再散落到 `/tmp` 或 workspace 其它目录，统一输出到项目内 `build/`：
  - 默认/host：`build/<profile>/…`
  - 预留 cross compile：`build/<target>/<profile>/…`（暂不要求实现 cross compile，只要求不要把路径写死导致未来迁移困难）
- 目标：
  - 最终可执行文件固定落点：`build/<profile>/bin/<project-name>`（Windows 可为 `.exe`）
  - build 过程产生的中间产物（.o 等）也应进入 `build/<profile>/obj/`（若实现成本过高，至少保证最终可执行与关键中间产物不再进 `/tmp`）
- 验收：
  - 新增/更新 `run_pass_cone` fixture：断言 `scoop build` 后对应路径存在且可运行（stdout 可断言）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0101（统一 LLVM 21 基线，避免 build/run 行为在不同 LLVM 版本下漂移）

### T1122 [DONE] build profile：`scoop build --debug/--release` 与默认策略落地
- 描述：补齐 build profile 的对外接口与行为：默认 debug，`--release` 选择 release，`--debug` 显式选择 debug（便于脚本化）。
- 目标：
  - CLI 行为要和 `scoop run` 保持一致（共用 profile 解析/默认值）。
  - 先只支持 `debug/release` 两个 profile；更复杂的 profile 名称后置。
- 验收：
  - `crates/scoop` 单测：CLI 参数解析覆盖 `--debug/--release` 冲突处理与默认值。
  - 端到端：同一 cone fixture 在 debug/release 两种 profile 下都可 build（路径不同）并可运行。
- 依赖：T0101、T1121

### T1123 [DONE] `scoop run`：在 CONE 项目目录下自动 build 并运行（支持 `--debug/--release`）
- 描述：当在 CONE 项目目录（存在 `Cone.toml`）下执行 `scoop run` 时：
  - 若目标 profile 的可执行文件不存在：先 build，再运行
  - 若已存在：v0 允许仍然 always rebuild，但至少要支持“未构建则构建”与 profile 选择
- 目标：`run` 复用 `build` 的参数解析与输出目录规则，避免两套路径不一致。
- 验收：
  - 新增 `run_pass_cone` fixture：在空 build 目录下直接 `scoop run`，应先构建并输出 stdout。
  - 另增 fixture：`scoop run --release` 运行 release 产物（与 debug 输出目录不同）。
- 依赖：T0101、T1121、T1122

### T1124 [DONE] 增量构建 v1（粗粒度）：输入 fingerprint 未变则跳过 build（优化项）
- 描述：在 v0（always rebuild 但输出稳定）之后，引入最小增量优化：记录输入 fingerprint，未变化则跳过 build。
- 目标：
  - 在 `build/<profile>/build.json` 写入 fingerprint（至少包含：`Cone.toml` + `src/**/*.scoop` + 关键 build flags + 工具链版本）。
  - `scoop run`/`scoop build` 在 fingerprint 未变且可执行存在时直接复用产物。
  - 只做粗粒度缓存：不做依赖图，不做“只重建受影响文件”。
- 验收：
  - 新增集成测试或 fixture：连续两次 `scoop run`，第二次应打印“skipping build / cache hit”（或等价可断言行为）并直接运行。
  - 行为必须可禁用（例如 `--no-incremental` 或 `SCOOP_INCREMENTAL=0`），避免排查问题困难。
- 依赖：T0101、T1121、T1123

---

## T16：Scoop 编译器（语义完善 + 优化等级/去虚化/HIR-MIR）

### T1601 [DONE] 对外接口：新增并统一优化等级（CLI + Cone.toml + 默认策略）
- 描述：为 `scoop build/run/test` 增加明确的优化等级选项，并与 `Cone.toml[native-build]` 配置对齐，形成可预测的默认策略（debug/release）。
- 目标：
  - CLI：支持 `-O/--opt-level <0|1|2|3|s|z>`（或等价 API），并允许覆盖 `Cone.toml` 默认值。
  - manifest：在 `Cone.toml[native-build]` 增加 `opt-level`（或等价字段），并定义与 profile（debug/release）的映射规则。
  - LLVM 后端：把 `TargetMachine` 的 `OptimizationLevel` 与 opt-level/profile 对齐（当前实现仍是 `OptimizationLevel::None`，仅跑少量 IR passes）。
  - 不在本任务引入 LTO/PGO 等更高阶优化；先把“等级语义”固定下来。
- 验收：
  - `crates/scoop` 单测覆盖：CLI 参数解析与优先级（CLI 覆盖 toml）。
  - 端到端：新增 `tests/fixtures/run_pass_cone/**` 用例分别在 `-O0` 与 `-O2` 下可构建并运行（语义一致）。
- 依赖：T0101；（历史）LLVM build/run 链路已可用；细节见 `TODO-1.md` 的相关任务

### T1602 [DONE] LLVM 优化流水线：按 opt-level 启用常见 passes（DCE/inlining/unroll 等）
- 描述：基于 LLVM PassBuilder（`Module::run_passes`）按优化等级启用/禁用常见优化 passes，优先引入“低复杂度但高收益”的 IR 清理与 DCE/CSE/DSE，并在 release 下逐步接入更重的全局优化。
- 目标：
  - `-O0`：尽量保持 IR 可读与可调试（最小化优化）。
  - `-O1/-O2/-O3`：优先采用 LLVM 默认优化 pipeline（`default<O2>` 等），必要时再做少量补丁式增强。
  - `-Os/-Oz`：针对 size 的 pipeline（若暂不支持，必须给出稳定错误码与文档说明，而不是静默忽略）。
  - 建议的“低复杂度高收益”清单（优先接入）：
    - 必备清理：`instcombine`、`simplifycfg`
    - 早期冗余/内存优化：`early-cse`、`dse`、`dce`（必要时 `adce`）、`sccp`
    - release 再考虑：`gvn/newgvn`、`jump-threading`/`correlated-propagation`、`memcpyopt`
  - GC/statepoint 约束：
    - 大多数优化应放在 `rewrite-statepoints-for-gc` **之前**；
    - rewrite 之后仅做轻量清理（例如 `function(instcombine,simplifycfg)`），避免在 `gc.statepoint/gc.relocate` 之后跑大量 pass 增加风险；
    - `place-safepoints` 暂不纳入默认管线（旧 LLVM 18.1.8 曾观察到 SIGSEGV；在 LLVM 21 上需单独验证稳定性后再决定是否接入）。
- 验收：
  - 新增 build fixtures：同一输入在 `-O0` 与 `-O2` 下 `--emit-llvm` 产物可用 `BUILD-LLVM-(NOT-)CONTAINS` 断言观察到至少 1 个典型优化（例如死代码被移除或内联发生）。
  - 新增 build fixture（或复用现有单测）：断言 `rewrite-statepoints-for-gc` 仍然产出 `gc.statepoint`（避免优化管线破坏 GC rewrite 的前置条件）。
  - `cargo test --all` + `cargo run -p scoop -- test` 通过。
- 依赖：T0101、T1601；（历史）`--emit-llvm` 与 build fixtures 子串断言能力已存在

### T1603 [TODO] 去虚化：receiver 类型已知时确保能生成直调用（final/sealed/value）
- 描述：当 method call 的 receiver 类型在编译期已知时，尽量走直调用路径，确保 LLVM 去虚化能生效（尤其是 receiver 为 final/sealed class 或 value type 时）。
- 目标：
  - value type：默认应为静态分派（direct call），不得引入不必要的 vtable/间接调用。
  - final/sealed class：在可证明单一目标时生成直调用（或提供足够信息让 LLVM 去虚化）。
  - 不在本任务实现全局的 class hierarchy analysis；先从“显然可证明”的 case 落地。
- 验收：
  - 新增 build fixture：`--emit-llvm` 断言在目标 case 下不出现 function pointer 间接调用，而是直接 `call @Type_method`（按实际 codegen 命名规则断言关键子串即可）。
  - 新增 run-pass fixture：验证语义正确（stdout）。
- 依赖：（历史）对象语义与方法调用链路已存在

### T1604 [TODO] HIR/MIR 级优化 v0：无 `perform` 时不生成 `handle` 结构/帧
- 描述：在 lowering/codegen 前进行一次“cheap”静态分析：若某作用域（函数/块）内不存在 `perform`，则不应生成 `handle` 相关的结构体、栈帧或 TLS handler 链接，减少运行时开销。
- 目标：
  - 只做“没有 perform → 不生成 handle”的消除；不做复杂的跨函数分析或效果推断优化。
  - 保证与当前 effect 语义一致：一旦存在 `perform`，仍按既有机制生成并正确工作。
- 验收：
  - 新增 build fixture：对不含 `perform` 的程序，`--emit-llvm` 中不出现 handler/handle 相关符号（用 `BUILD-LLVM-NOT-CONTAINS` 断言关键子串）。
  - 新增 run-pass fixture：在默认模式与 `--gc-stress` 下行为一致。
- 依赖：（历史）effect lowering/codegen 基础链路已存在

### T1605 [TODO] 高级优化候选清单：建立并持续维护（不阻塞主线）
- 描述：建立并维护一份 Scoop 编译器的高级优化候选清单，用于后续分阶段立项（避免“想到哪做哪”）。
- 目标：清单必须标注：
  - 适用层级（HIR/MIR/LLVM）
  - 预期收益（性能/体积/GC 压力/线程扩展）
  - 风险与前置依赖（例如需要更强的类型/效果信息或 runtime 支持）
- 验收：把清单维护在 `PLAN.md` 的“编译器优化”部分，并为每个候选项保留可拆分的任务入口（后续逐步补齐）。
- 依赖：无

### T1606 [TODO] LLVM：escape continuation `handle` 完整语义（0..N perform 点）
- 描述：当前 LLVM 后端对 escape continuation（`, k ->`）的 `handle` 仍是“最小可回归链路”：只支持单个 perform 点，且要求为 block 的第一个语句。该限制导致 stdlib/fixtures 只能用“嵌套 handle / 二段 handle”绕开，无法表达真实 async/await 的直觉写法（同一 handle body 内多次 await）。
- 目标：
  - 语义完整性：支持 `handle { ... } with { Effect.op(...), k -> ... }` 的 body 含 **0..N** 个 perform 点：
    - 0 个 perform：handle 直接执行到结束并返回 body 的值（不依赖 arm）。
    - N≥1：每次 perform 触发一次 suspension，并生成新的 continuation；后续可多次 suspension/resume（每个 continuation one-shot，但同一“计算”可经历多个 suspension 点）。
  - 结构完整性：不再要求 perform 必须是 block 第一个语句；允许在 perform 前后有普通语句（含 val/assign/expr）。
  - 动态上下文正确性（Appendix A / spec §5.5）：
    - continuation resume 时恢复其捕获的 handler stack；
    - handler arm body 期间应避免自捕获（arm 内再次 perform 同一 op 应命中外层 handler，而不是自身）。
  - GC 正确性：heap state machine 的状态对象必须是 GC-managed，且其内部引用字段可被准确扫描/更新（moving GC 下不可漏扫）。
- 验收：
  - 新增 run-pass fixtures：单个 `handle` body 内连续 2~3 次 `await/yield`（不使用嵌套 handle workaround），stdout 顺序可观测且在 `--gc-stress` 下稳定。
  - 复跑既有 fixtures：`cargo run -p scoop -- test` 与（可选）`cargo run -p scoop --features llvm -- test` 通过。
- 依赖：（历史）T0617/T0914/T0915/T0916（escape continuation + handler stack 基础链路）；（新增）T1706/T1707（回归用例）

### T1607 [TODO] Continuation resume payload：从 `u64` 扩展为可表达任意 `T`
- 描述：当前 `k.resume(value)` 的 LLVM lowering 只支持把 `value` 编码为 `u64` word，且明确禁止 GC 指针（Ref/String）与复合值。这与 spec 对 `Continuation<T>` 的泛型语义不一致，也限制了 async/await/generator 在真实场景中的可用性。
- 目标：
  - 设计并落地一个“可携带任意 `T`”的 resume payload ABI（至少覆盖：Unit/Bool/Int/String/Ref/tuple/struct/enum；允许 future 扩展）。
  - 与 GC 对齐：payload 若包含引用类型，必须可被 GC 扫描/更新（moving GC 下 resume 后仍正确）。
  - 维持 one-shot：重复 resume 必须表现为可捕获的 `Raise<RuntimeError.ContinuationAlreadyResumed>`（而非进程级 exit）。
- 验收：
  - 新增 run-pass fixtures：`Continuation<String>`、`Continuation<(Int, String)>`、`Continuation<MyStruct>` 等在 `--gc-stress` 下通过，并覆盖“resume 后继续分配触发多轮 GC”。
  - 为 ABI 关键点补 build fixtures（可选）：断言不出现“ptr<->int”非法编码路径。
- 依赖：T1606（多点 suspension 需要更通用 payload 才能覆盖真实案例）；（历史）T0630（payload ABI 统一化方向）

### T1608 [TODO] Effect op_tag：稳定分配与统一 dispatch（运行期 handler stack / perform slot）
- 描述：runtime handler stack 的“最近匹配”分发基于 `op_tag` 精确匹配；而当前 codegen 对自定义 effect 的 `op_tag` 仍大量写 0（除 `Raise` 以外）。这会在“嵌套 handler + re-perform + 多 effect 并存”时产生错误分发或无法诊断的语义漂移。
- 目标：
  - 为所有 compiler-known 的 effect op 分配稳定 `op_tag`（至少在单次编译产物内稳定；若要求跨版本稳定需额外讨论）。
  - 统一 perform slot 编码：`perform` 写入 `op_tag + payload`；`handle` 边界解码并分派到最近匹配 handler。
  - 让 `EscapeContinuation` 与 `ImmediateResume` / non-resuming handler 共享同一套 dispatch 规则（避免多套“特判语义”长期并存）。
- 验收：
  - 新增 run-pass fixture：三层嵌套 handler（至少两种不同 effect），arm 内 re-perform 验证“最近匹配 + active/inactive”规则成立。
  - 针对错误分发给出稳定诊断（至少包含 op_tag / effect 名称 / src line/col）。
- 依赖：（历史）T0913/T0916（runtime handler stack），T0617（escape continuation lowering）

### T1609 [TODO] `finally` + escaping continuation：unwind/cleanup 的组合语义
- 描述：当前 escaping continuation 的 `handle` 明确不支持 `finally`。要实现完整 effect 语义，必须定义并实现：当计算被 suspend / resume / abandon 时，`finally` 的执行时机与次数规则（并保证与 flag-based unwinding 一致）。
- 目标：
  - 定义并实现 `finally` 的语义：至少覆盖
    - 正常执行（无 perform）；
    - 多次 suspension/resume；
    - arm 内抛 `Raise` 或发生未处理 effect 时的传播路径。
  - 保证 `finally` 不会被重复执行或漏执行，并在嵌套 handle 下保持栈式顺序。
- 验收：
  - 新增 run-pass fixtures：在多次 `await` 的 handle 外层加 `finally`，用 stdout 断言 `finally` 的执行次数/顺序；并在 `--gc-stress` 下稳定。
  - 复跑 try/catch/finally 相关 fixtures，确保无回归。
- 依赖：T1606、T1608（需要统一 dispatch/unwind 语义作为基础）

---

## T17：验证套件（覆盖已实现语义：Continuation/GC/多线程）

### T1701 [TODO] Escaping continuation：构造复杂 fixtures（模拟 async executor/scheduler）
- 描述：创建一组更复杂的 fixtures，模拟 async executor/scheduler 的行为：continuation 逃逸到数据结构中、跨函数/跨作用域恢复、多个任务交错调度。
- 目标：
  - 覆盖：多层 handler 嵌套 + continuation 多次捕获/恢复 + 恢复顺序变化（队列/栈/优先级等）。
  - 先用单线程实现调度器模型；多线程扩展交给 T1705。
- 验收：
  - 新增 run-pass fixtures：至少 3 个用例（FIFO/LIFO/round-robin），stdout 稳定可断言。
  - 同时在 `--gc-stress` 下运行不崩溃且输出一致。
- 依赖：（历史）escaping continuation 已实现；fixtures runner 支持 run-pass

### T1702 [TODO] `Continuation<T>` 完整性：覆盖 `T` 的全类型空间与操作组合
- 描述：验证 `Continuation<T>` 的泛型完整性：`T` 可以是任意类型（struct/tuple/enum/ref/甚至 `Continuation` 本身），并且所有对 `Continuation<T>` 的操作都按预期工作。
- 目标：
  - 覆盖 value/ref 混合：`Continuation<(Int, String)>`、`Continuation<Option<MyRef>>`、`Continuation<MyStruct>` 等。
  - 覆盖自递归：`Continuation<Continuation<Int>>` 的捕获与恢复（避免布局/GC root 漏洞）。
  - 不追求“性能最优”；先确保语义与内存安全。
- 验收：
  - 新增 run-pass fixtures：至少覆盖上述 5 类 `T`（struct/tuple/enum/ref/Continuation），并在 `--gc-stress` 下通过。
  - 必要时补 build fixtures：对关键 IR 形态做 contains/not-contains 断言（避免隐藏的 pointer encoding/roots 漏扫风险）。
- 依赖：（历史）escaping continuation + GC 安全布局规则已存在

### T1703 [TODO] GC 正确性：跨函数、复杂 value/ref 混合环境的 fixtures
- 描述：创建 fixtures 验证 GC 在跨函数场景下的正确性：多函数/多类/tuple/struct/enum 互相引用，以及“值类型包含 ref 字段”的深层嵌套与数组容器。
- 目标：
  - 覆盖：数组里既有 ref 又有 value（value 内再含 ref）；跨函数返回/传参形成长期存活对象图。
  - 覆盖：对象之间循环引用、短命对象与长命对象交错分配。
- 验收：
  - 新增 run-pass fixtures：至少 5 个用例，每个都在 `--gc-stress` 下稳定通过。
  - 若存在 `--gc-verify`（或等价开关），应优先启用以把“silent corruption” 变成显式失败。
- 依赖：（历史）GC 多线程/stackmap 协议基础能力已存在

### T1704 [TODO] GC + escaping continuation：验证 continuation 逃逸时的 roots/scan 正确性
- 描述：把 T1701 的 escaping continuation 与 T1703 的复杂对象图结合，验证 continuation 逃逸时 GC roots 枚举与更新完全正确。
- 目标：
  - 覆盖：continuation 捕获的环境中包含复杂对象图（数组/struct/enum/ref 混合）。
  - 覆盖：恢复 continuation 后继续分配，触发多轮 GC。
- 验收：
  - 新增 run-pass fixtures：至少 2 个用例（一个强调“深层对象图”，一个强调“高频捕获/恢复 + GC 压力”），在 `--gc-stress` 下通过。
- 依赖：T1701、T1703

### T1705 [TODO] 多线程扩展：在多线程下验证 continuation 与 GC 的组合正确性
- 描述：把上述验证场景扩展到多线程：跨线程恢复 continuation、并发分配、并发触发 GC（或协作式 STW），确保线程注册、root 枚举与对象移动/更新正确。
- 目标：
  - 覆盖：多个线程各自维护任务队列、跨线程偷取 continuation 并恢复。
  - 覆盖：GC 与线程同步原语交互（避免死锁/漏扫/崩溃）。
- 验收：
  - 新增 run-pass fixtures：至少 2 个多线程用例（stdout 稳定），并在 `--gc-stress` 与默认模式均可通过。
  - 为避免 flakiness，必须固定调度策略（barrier/顺序号/确定性调度器）。
- 依赖：（历史）多线程 STW/线程注册/并发分配基础能力已存在

### T1706 [TODO] 多 perform 点（单个 handle）：async/await 真实写法回归
- 描述：新增一组 fixtures，专门覆盖“单个 escape-continuation `handle` body 内出现多个 perform 点”的真实写法（例如连续 `await` 两到三次），不允许用“嵌套 handle / 二段 handle”的 workaround。
- 目标：
  - 覆盖：两次以上 suspension/resume；resume 后继续执行并再次 suspension。
  - 覆盖：perform 前后都有普通语句与局部变量（确保 state machine 的 local lifting 正确）。
  - 覆盖：arm body 将 continuation 入队（模拟 executor），并按确定性顺序恢复。
- 验收：
  - 新增 run-pass fixtures（stdout golden）：至少 2 个（一个单线程调度，一个跨线程 resume）。
  - 所有用例在 `--gc-stress` 下稳定通过。
- 依赖：T1606（多 perform codegen）、（历史）T0915a/T0618（跨线程 resume 运行期原语）

### T1707 [TODO] 控制流 + 多次 suspension：if/when/循环边界的语义回归
- 描述：针对多 suspension 点下最容易出错的控制流形态新增 fixtures：`if/when` 分支在不同路径上 perform 次数不同、以及在循环体内 suspension（至少先覆盖“有限次迭代的 while/for 等价形态”）。
- 目标：
  - 覆盖：分支合流（phi）上的局部变量在 suspend/resume 后仍正确。
  - 覆盖：同一局部变量跨多个 suspension 点读写（包含 value/ref 混合）。
  - 覆盖：arm 内 re-perform 与外层 handler 的交互（active/inactive 规则）。
- 验收：
  - 新增 run-pass fixtures：至少 3 个用例（分支/合流、循环、re-perform）。
  - 可选 build fixtures：对关键 IR 形态做 contains 断言（例如 state machine 的 pc/dispatch 存在）。
- 依赖：T1606、T1608

---

## T18：标准库完整性（基于 `KOTLIN_RUNTIME_GAP_AUDIT.md` 的持续补齐）

### T1801 [TODO] 现状对照：把 `KOTLIN_RUNTIME_GAP_AUDIT.md` 转成“可执行的 std 完整性清单”
- 描述：基于 `KOTLIN_RUNTIME_GAP_AUDIT.md` 的能力矩阵，梳理当前 `sysroot/` + `stdlib/` 的实现覆盖度，并产出一份可执行的清单（DONE/TODO/Blockers）。
- 目标：
  - 以“能力项”为粒度，而不是以 API 名称为粒度（保持与审计文档一致）。
  - 每个能力项必须链接到：实现位置（sysroot/stdlib/runtime/c）+ 对应 fixtures（若已有）或计划新增的 fixtures（若缺失）。
- 验收：
  - 更新 `KOTLIN_RUNTIME_GAP_AUDIT.md` 的表格/结论（或新增 `STDLIB_COMPLETENESS.md` 并从审计文档链接过去），并给出下一步 TODO 入口（T1802）。
- 依赖：无

### T1802 [TODO] 拆分任务：按领域/优先级把缺口拆成可单独回归的小任务
- 描述：把 std 完整性缺口按领域拆分为可实现的任务组（collections/text/ranges/sequences/math/random/time/io 等），并明确每组是纯 Scoop、需要 runtime lib，还是必须走 intrinsic gate。
- 目标：
  - 默认不新增 intrinsic：任何 “needs_new_intrinsic” 结论必须回到 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md` 的 gate 流程。
  - 每个子任务必须附带 fixtures 计划（compile-fail / run-pass），优先使用 cone 多文件 fixtures 覆盖真实使用方式。
- 验收：
  - TODO 中为每个 P0/P1 能力项至少创建 1 个任务条目，并标注依赖与验收命令。
- 依赖：T1801

### T1803 [TODO] 回归基座：建立 stdlib 的 smoke + matrix fixtures
- 描述：为 stdlib 建立一组“冒烟测试”与“覆盖矩阵”fixtures，确保每次改动都能覆盖核心能力面，并能指出缺口。
- 目标：
  - smoke：少量但高价值的端到端示例（文本/集合/迭代/范围/基础 IO）。
  - matrix：按领域扫描 fixtures 覆盖度（可复用 `scoop_tools fixtures-matrix` 的机制）。
- 验收：
  - 新增 `tests/fixtures/run-pass/stdlib_smoke/**`（或等价目录）至少 3 个。
  - `cargo run -p scoop_tools -- fixtures-matrix check` 能报告 stdlib 领域覆盖度（缺口提示即可，是否 gating 后续再定）。
- 依赖：T1802

### T1804 [TODO] 优先补齐（P0/P1）：从审计表中挑选最能提升可用性的缺口先落地
- 描述：在 T1801/T1802 的基础上，挑选最有 fixture 价值、最能提升“写示例的体验”的能力项，优先补齐。
- 目标（建议起步）：
  - text：`substring/startsWith/split` 的最小可用版本（必要时引入 runtime lib，但不新增 intrinsic）。
  - formatting：`StringBuilder/joinToString`（优先纯 Scoop，性能后置）。
  - time/io：`now()/readLine()` 的最小平台实现（走 `runtime/c/platform`）。
- 验收：每个能力项新增至少 1 个 run-pass fixture，并在 `--gc-stress` 下通过。
- 依赖：T1802
