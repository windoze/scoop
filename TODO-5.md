# TODO（core / stdlib reshape）：P9 + P10 + P11 + P12 + P13：删 stdlib + 清 thread/atomic + 测试 helper + core 去 sysroot 化 + 文档收尾

> 计划基线：[`PLAN.md`](./PLAN.md)
> 任务索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。
> 全局约束：见 [`TODO.md`](./TODO.md) `## 全局约束` 一节。
## P9：删除 `stdlib/`

### [DONE] P9-T01：把 desugar 依赖从 stdlib 迁入 core

- 参考：
  - [`PLAN.md`](./PLAN.md) §6.5 / §9 / P9
  - `stdlib/prelude.scoop`（line 79-138：`Int.rangeTo` / `Int.downTo` / `Int.until` / `IntProgression.forEach` + `__scoop_range_default_step` helper）
  - `sysroot/core.scoop` 中现 `IntProgression` struct 声明（line 462）
- 目标：
  - 把 `for (x in 0..n)` / range desugar 必需的 helper 从 `stdlib/prelude.scoop` 迁入 sysroot core。
  - 同时按 PLAN §6.5 把 `IntProgression` 路径扩展到其他整数类型（`Long` / `UInt` / `ULong` 等）。
  - `require/check/let/run/also/apply` 等**不**迁——这些跟着 stdlib 一起删（PLAN §4.2）。
- 当前实现入口：
  - `stdlib/prelude.scoop` line 79-138
  - `sysroot/core.scoop` line 462（`IntProgression` 声明）
- 必须实现的内容：
  1. 在 sysroot 新建 `sysroot/progression.scoop` 文件（或直接追加到 `sysroot/core.scoop` 末尾——建议独立文件，便于后续扩展）：
     - `package scoop.core`
     - `import scoop.core.*`
     - 把 `Int.rangeTo` / `Int.downTo` / `Int.until` / `IntProgression.forEach` / `__scoop_range_default_step` 从 prelude 迁入。
     - 删去 `__scoop_range_default_step` 中"避免字面量"的 trick（参考现 `__scoop_range_default_step(sample: Int): Int { val word: Int = sizeOf(sample); return word / word }`）—— 现在自动 prelude 与 P3 后 sysroot literal 限制是否还在需要 P0-T01 baseline 时确认；如果不再有限制，直接写 `1` 而非 `sizeOf(sample) / sizeOf(sample)`。
  2. 多类型扩展：为 `Long` / `UInt` / `ULong` 各加一个 progression 类型 + `rangeTo/downTo/until/forEach`：
     - `struct LongProgression(val first: Long, val last: Long, val step: Long, val increasing: Bool)`
     - `fun Long.rangeTo(endInclusive: Long, step: Long): LongProgression { ... }`
     - `fun LongProgression.forEach(action: (Long) -> Unit / E): Unit / E { ... }`
     - 同理 UInt / ULong
     - **注意**：`for (x in 0..n)` desugar 当前应当是用 `Int.rangeTo`；如要支持 `for (x in 0L..nL)` 走 LongProgression，desugar 路径需要按 receiver type 分流——确认当前 desugar 实现位置，必要时扩展。
  3. owner 测试：
     - `tests/fixtures/run-pass/range_int_for.scoop`：`for (x in 0..10) { ... }` 现有形态保持工作。
     - `tests/fixtures/run-pass/range_long_for.scoop`：`for (x in 0L..10L) { ... }` 工作。
     - 同理 UInt / ULong。
- 必须遵从的约束：
  - 不在本任务删 `stdlib/prelude.scoop`（P9-T03）—— 但要把 stdlib 中的 progression helper 注释掉或保留（被 sysroot 同名 helper 覆盖时的解析优先级）。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/range_*.scoop`
  2. `cargo run -p scoop -- test`（全量 baseline）—— for-loop 相关 fixture 无回退。
- 完成条件：
  - core 自身已包含 desugar 必需的 progression helper；删 stdlib 不会破坏 for-loop。
- 依赖：P8-T06。

完成记录：

- 改动范围：
  - `stdlib/prelude.scoop` 中旧 progression helper block 已移除，仅保留 scope/precondition 等待 P9-T03 删除的 stdlib surface。
  - `sysroot/core.scoop` 现在包含 `Int/Long/UInt/ULongProgression`、`__scoop_range_default_step`、四组 `rangeTo/downTo/until/forEach` helper；`__scoop_range_default_step` 直接返回 `1` / `1u` / `1UL`，不再使用 `sizeOf(sample) / sizeOf(sample)` trick。
  - `..` range HIR lowering 改为按 progression 结果类型直接构造对应 progression struct；`rangeTo/downTo/until` extension call lowering 同步 inline 为 progression struct 构造，避免 overloaded top-level helper 在 HIR/codegen 阶段重新选择错误版本。
  - typecheck / HIR for-loop lowering 增加 `LongProgression` / `UIntProgression` / `ULongProgression` 分流；整数字面量增加 `L` / `u` / `UL` suffix 支持，满足 `0L..10L` / `0u..10u` / `0UL..10UL` owner fixtures。
  - LLVM type FQN fallback 补齐 `Byte/Short/UShort/Long/ULong/Double` 标准 alias 映射，避免 alias 出现在 layout field fallback 时退化为未知类型。
  - 新增 owner fixtures：`range_int_for.scoop`、`range_long_for.scoop`、`range_uint_for.scoop`、`range_ulong_for.scoop` 及对应 stdout。
- 核心决策：
  - 选择直接追加到 `sysroot/core.scoop`，未保留独立 `sysroot/progression.scoop`。验证中发现独立 compilable sysroot file 会让 progression struct 在当前 sysroot/support 双路径中产生不同 codegen identity；本任务允许“或直接追加到 core”，因此采用 core 内落地，避免引入新的 sysroot file 身份问题。
  - 对 unsigned `until(0u/0UL)` 返回空 progression；`forEach` 在命中 `last` 后先 `break` 再加减 step，避免端点处 unsigned 下溢导致无限循环。
  - `..` 与 `rangeTo/downTo/until` helper call 在 HIR 中直接构造 progression struct；sysroot 仍保留普通 Scoop helper surface，供 source-level API 与 P9-T03 后 core-only 环境使用。
- 验证结果：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/range_int_for.scoop` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/range_long_for.scoop` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/range_uint_for.scoop` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/range_ulong_for.scoop` 通过。
  - 既有 range/progression 回归 `for_in_int_progression_basic.scoop`、`stdlib_ranges_enhanced_basic.scoop`、`stdlib_smoke_ranges_and_io.scoop`、`kotlin_ranges_progressions_basic.scoop` 均通过。
  - `cargo build` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all --all-targets` 通过（858 passed）。
  - `cargo run -p scoop -- test` 完整执行，结果为 1357/1358 targets passed、1394 checks passed；唯一失败仍是既有 `run-pass/mutable_array_ops_basic.scoop`。
- 与 `PLAN.md` 对应闭合：
  - 闭合 PLAN §6.5：`IntProgression` 扩展为 `Int/Long/UInt/ULong` 四组 progression surface。
  - 闭合 PLAN §9 / P9-T01：range / for-loop desugar 不再依赖 `stdlib/prelude.scoop` 中的 progression helper；后续 P9-T03 删除 `stdlib/` 不会破坏 range/for-loop core path。
- 暂时性 failing fixture：
  - `tests/fixtures/run-pass/mutable_array_ops_basic.scoop` 是 P4-T02 / P8-T04c 等完成记录中已列出的既有失败，覆盖已删除的旧 `MutableArray<Int>.pop/insert/removeAt/splice` copy-style API；继续由 P9-T02 三分类清单处理，最终由 P13-T04 收尾。本任务未新增 failing fixture。

### [DONE] P9-T02：按 P0-T01 三分类清单批量改写 / 合并 / 删除 fixture

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P9
  - `target/reshape-baseline/stdlib-fixtures.txt`（P0-T01 产出）
- 目标：
  - 对 P0-T01 中分类为 `KEEP-RENAME` 的 fixture：批量改 `import` 语句（`import scoop.core.array.*` / `import scoop.core.collections.*` 等改为依赖自动 prelude 或显式 `import scoop.lang.string.*`）。
  - 对 `MERGE-INTO:<target>` 的 fixture：把内容并入 `<target>` fixture，删除原文件。
  - 对 `DELETE` 的 fixture：直接删除文件。
- 当前实现入口：
  - `tests/fixtures/run-pass/stdlib_*.scoop` 等
  - `target/reshape-baseline/stdlib-fixtures.txt`
- 必须实现的内容：
  1. 按清单分类逐条处理：
     - `KEEP-RENAME`：保留文件、改 import / 改语法形式直至能 pass；如 fixture 名以 `stdlib_` 开头但内容仍有效，**不**重命名文件（避免大量 fixture 文件改名带来的 git noise；只改内容）。
     - `MERGE-INTO:<target>`：把本文件的 fun main 内容合并到目标 fixture 的 main 内（按 section 注释隔开），更新 EXPECTED stdout，删除本文件。
     - `DELETE`：直接 `git rm`，且必须在完成记录中按下面的"DELETE 论证规则"逐条说明。
  2. **DELETE 分类的论证规则（与 [`TODO.md`](./TODO.md) "fixture 删除标准" 配对）**：本任务期间允许删除的 fixture **必须**满足"测试意图本身已失效"——即 fixture 验证的对象（API / 类型 / 语义）在本轮 reshape 后**真的不复存在**。完成记录中对每条 DELETE 必须写一行论证，形如：
     - `stdlib_collections_set.scoop` → DELETE：被测对象 `Set.contains/insert/remove` 等 stdlib API 已整体随 stdlib 删除（P9-T03），且新 stdlib 重设计前没有等价对象需要被覆盖。
     - `stdlib_mutable_array_splice.scoop` → DELETE：被测对象 `MutableArray<Int>.splice` 已删（旧 stdlib 实现，新 MutableArray 不再支持 splice）；如未来新 stdlib 引入等价 API，应当**当时**重写测试，而不是现在删 fixture。
     - **不**允许仅以"fixture 跑不通且改起来麻烦"为理由删除——这种情况一律走 KEEP-RENAME 路径，重写到能 pass。
  3. 每条 KEEP-RENAME / MERGE-INTO 改动必须保证 fixture 在新 stdlib（即 core + lang.string + 自动 prelude）下能 pass。改完后立即跑 `cargo run -p scoop -- test --fixtures <path>` 确认。
  4. 完成记录中报告：原 stdlib fixture 数 / 处理后 KEEP-RENAME 数 / MERGE-INTO 数 / DELETE 数 + 每条 DELETE 的论证（按上述规则）。
- 必须遵从的约束：
  - 不允许改 fixture 的运行结果（stdout / exit code）—— 如果原 fixture 期望 `42` 输出，迁移后仍需输出 `42`。
  - 不允许把原本 run-pass 的 fixture 降级为 typecheck-fail（除非分类是 DELETE 且符合 DELETE 论证规则）。
  - 严格按 P0-T01 的分类执行——任何反向变更（如发现 P0-T01 标 DELETE 的 fixture 实际上测试意图仍有效，应当 KEEP-RENAME）必须**先**回写 P0-T01 完成记录、说明误判原因，再改本任务的处理。
  - 不允许"批量 DELETE 一组带 `stdlib_` 前缀的 fixture 而不逐条论证"——每条 DELETE 决定都是独立的设计决定。
- 验证：
  1. 逐条改完后跑 `cargo run -p scoop -- test --fixtures <path>`。
  2. 全量：`cargo run -p scoop -- test` —— pass 数 = P0-T01 baseline pass 数 - DELETE 类数 - MERGE 类数。
- 完成条件：
  - fixture tree 中不再有引用 `import scoop.core.array.*` / `import scoop.core.collections.*` / `import scoop.core.math.*` / `import scoop.core.range.*` 的文件。
  - 不再有 `require()` / `check()` / `let()` / `run()` / `also()` / `apply()` 调用（这些 helper 在 P9-T03 删除前会失效；如 fixture 依赖须改为内联表达式）。
- 依赖：P9-T01。

完成记录：

- 改动范围：
  - 按 `target/reshape-baseline/stdlib-fixtures.txt` 处理 21 条 stdlib-dependent fixture：`KEEP-RENAME` 8、`MERGE-INTO` 0、`DELETE` 13。
  - 删除 13 个 DELETE-class `.scoop` fixture 及对应 `.stdout` sidecar；保留 8 个 KEEP-RENAME fixture，未改名，全部在 core + lang.string 自动 prelude 下通过。
  - 清理整个 `tests/fixtures/**` 中旧 stdlib helper surface：`require/check/requireLazy/checkLazy` 断言改为显式 `panic` guard；自定义测试函数 `run/apply` 改名，避免 P9-T03 删除 scope helpers 后产生歧义；同步更新 effect/MIR golden。
  - 移除 build fixture overlay sysroot 中过期的 `Int.let/run/also/apply` 声明。
  - 更新 Rust 单元测试中对已删除 stdlib fixtures 与已改名 helper fixture 的硬编码引用，改为覆盖保留的 `MutableArray.push` materialization 行为。
- 核心决策：
  - DELETE 只用于测试对象本身已随 stdlib 删除的 fixture；保留的语言/runtime 行为（range/progression、String helpers、StringBuilder、operator assertions、effect/typecheck 行为）改写到 core/lang.string 或测试本地命名，不引入兼容 shim。
  - `require(...)` 成功路径原本无 stdout；改为 `if (!(...)) { panic("fixture assertion failed") }` 后保持原 run-pass stdout 与 exit code。
  - 旧 `run/apply` 名称在部分 fixture 中只是本地测试函数，不是 stdlib helper；为满足 P9-T03 前的 source-level 清理，改名而不删除测试意图。
- DELETE 论证：
  - `kotlin_require_check_basic.scoop` -> DELETE：被测对象 `require/check` 是旧 stdlib precondition helper，本轮随 stdlib 删除；异常/try-catch 语义由现有 `try_catch_*` 与 `Raise` fixture 覆盖。
  - `kotlin_require_check_lazy_message_basic.scoop` -> DELETE：被测对象 `requireLazy/checkLazy` 是旧 stdlib lazy precondition helper，本轮无 retained API。
  - `kotlin_scope_functions_basic.scoop` -> DELETE：被测对象 `let/run/also/apply` scope helper 随 stdlib 删除；lambda/effect 传播语义由重命名后的 typecheck/effect fixture 覆盖。
  - `list_and_mutable_list_basic.scoop` -> DELETE：被测对象 `MutableList.add` 与旧 `List/MutableList` stdlib surface 删除；保留的 `MutableArray.push/freeze` 由 P3/P4/P5 owner fixtures 与 `lang_string_builder_basic.scoop` 覆盖。
  - `mutable_array_ops_basic.scoop` -> DELETE：被测对象 `MutableArray<Int>.pop/insert/removeAt/splice` 属于旧 stdlib copy-style helper，新 `MutableArray` surface 不再提供这些 API；`push/freeze` 已有独立覆盖。
  - `stdlib_collections_algorithms_basic.scoop` -> DELETE：被测对象 `sort/reduce/zip` 等旧 collection algorithm helper 随 stdlib 删除，当前 reshape 无等价 retained API。
  - `stdlib_hash_set_map_basic.scoop` -> DELETE：被测对象旧 `Set/Map` hash-table helper surface 随 stdlib 删除，未来新 collections 设计应另补测试。
  - `stdlib_iter_algorithms_basic.scoop` -> DELETE：被测对象 `Array/MutableArray/List.map/filter/fold` 等旧 stdlib iteration algorithms 删除，当前无 retained API。
  - `stdlib_math_basic.scoop` -> DELETE：被测对象 `min/max` 旧 stdlib math helper 删除；保留的标量算术/比较行为由 P8 operator fixtures 覆盖。
  - `stdlib_set_map_basic.scoop` -> DELETE：被测对象旧 `Set/Map` surface 删除，当前无等价 retained API。
  - `stdlib_smoke_collections_and_iteration.scoop` -> DELETE：fixture 组合覆盖旧 collection algorithms 与 Set/Map helper；这些被测对象均随 stdlib 删除，range/iteration 保留行为由 range/progression fixtures 覆盖。
  - `stdlib_smoke_test_and_preconditions.scoop` -> DELETE：fixture 组合覆盖 `require/check` 与旧 Array fold/filter helper；这些 helper 删除，保留异常与数组行为由现有专门 fixture 覆盖。
  - `stdlib_string_builder_basic.scoop` -> DELETE：被测对象是假旧 `StringBuilder/joinToString` stdlib helper；真实 `scoop.lang.string.StringBuilder` 由 `lang_string_builder_basic.scoop` 覆盖。
- 验证结果：
  - `cargo run -p scoop -- test --fixtures <each KEEP-RENAME fixture>`：8/8 通过。
  - 代表性改写 fixture：operator / StringBuilder / effect / typecheck / overlay sysroot / MIR golden 相关 targeted runs 均通过。
  - `! rg -n --glob '*.scoop' 'import scoop\.core\.(array|collections|math|range)' tests/fixtures` 通过（无命中）。
  - `! rg -n --glob '*.scoop' '\b(require|check|requireLazy|checkLazy|let|run|also|apply)\s*\(' tests/fixtures` 通过（无命中）。
  - `cargo run -p scoop -- test` 通过：1345/1345 targets，1382 checks。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all --all-targets` 通过。
- 与 `PLAN.md` 对应闭合：
  - 闭合 PLAN §9 / P9-T02：stdlib-dependent fixtures 已按 P0-T01 清单保留、删除或改写；P9-T03 删除 `stdlib/` 前，fixture tree 已不再依赖旧 stdlib imports、precondition helpers 或 scope helpers。
- 暂时性 failing fixture：无。本任务删除了 P9-T01 完成记录中列出的既有失败 `mutable_array_ops_basic.scoop`，并未新增 failing fixture。

### [DONE] P9-T03：删除 `stdlib/` 目录与 frontend stdlib 注入路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P9
  - `stdlib/`（9 个 `.scoop` 文件）
  - `crates/scoopc/src/frontend.rs::default_stdlib_path`（line 763-764）
  - `crates/scoopc/src/frontend.rs` line 770-778：stdlib 注入到 `support_paths` 的代码
- 目标：
  - 删除 `stdlib/` 整目录。
  - 删除 frontend 中所有 stdlib 注入路径。
- 当前实现入口：
  - `stdlib/` 目录
  - `crates/scoopc/src/frontend.rs` line 763-778 一段
- 必须实现的内容：
  1. `git rm -rf stdlib/`
  2. 在 `crates/scoopc/src/frontend.rs` 删除：
     - `default_stdlib_path()` 函数（line 763-764）
     - 调用处的 stdlib 注入逻辑（line 770-778）：
       ```
       let root = default_stdlib_path()...
       let mut stdlib_paths = Vec::new();
       collect_scoop_files(&root, &mut stdlib_paths)?;
       support_paths.extend(...);
       ```
       全部删除。
     - 注意 `collect_scoop_files`（line 810）是否还有其它 caller—— 如有保留，否则也删除。
  3. 删除任何对 `stdlib/` 路径的硬编码引用（grep `stdlib/` 在 `crates/`、`tests/`、`build.rs`、`*.toml` 中）。
  4. `crates/scoopc/build.rs`：检查是否有 stdlib path embedding；如有，删除。
- 必须遵从的约束：
  - 必须在 P9-T02 完成后开工——所有 stdlib-dependent fixture 已被处理。
  - 删除是不可逆的；P9-T02 任何遗漏都会导致全量 baseline 大批失败。
- 验证：
  1. `ls stdlib/` —— 目录不存在。
  2. `grep -rn "stdlib" crates/scoopc/src/frontend.rs` —— 无关键命中（注释 / 字符串字面量除外）。
  3. `cargo build` —— 编译通过。
  4. `cargo run -p scoop -- test`（全量 baseline）—— pass 数与 P9-T02 后一致。
- 完成条件：
  - 仓库内不再有 `stdlib/` 路径或代码引用。
- 依赖：P9-T02。

完成记录：

- 改动范围：
  - 删除 tracked `stdlib/` 目录下 8 个旧 `.scoop` 文件：`array_iter.scoop`、`collections_iter.scoop`、`collections_map.scoop`、`collections_set.scoop`、`math.scoop`、`mutable_array_iter.scoop`、`mutable_list.scoop`、`prelude.scoop`。
  - `crates/scoopc/src/frontend.rs` 不再定位或注入 `../../stdlib`；默认 support sources 只来自 compilable sysroot files。
  - `crates/scoopc/src/comptime/interpreter.rs` 不再为 const/comptime pipeline 额外加载 stdlib sources。
  - `crates/scoop/src/commands/build/incremental.rs` 从 cone build fingerprint 与 `build.json` debug inputs 中移除 stdlib source hash；sysroot 与 runtime 仍计入 fingerprint。
  - 清理代码与 fixture 注释中对已删除项目 stdlib 路径/实现来源的过期描述。
- 核心决策：
  - 删除路径一次性收口，不保留兼容副本、shim 或旧 `stdlib/` fallback。
  - build fingerprint 直接以 sysroot/runtime/project sources 作为标准输入集合；删除 stdlib hash 会自然使旧 `build.json` fingerprint miss，无需额外迁移逻辑。
  - 保留 P9-T02 已决定保留的历史 fixture 文件名（如 `stdlib_string_basic.scoop`），它们已改为验证 core / lang.string 行为，不再依赖 `stdlib/` 目录。
- 验证结果：
  - `ls stdlib/` 报告 `No such file or directory`。
  - `git ls-files "stdlib/*"` 无输出。
  - `grep`/`rg` 等价检查：`crates/scoopc/src/frontend.rs` 无 `stdlib` 命中；Rust sources / TOML / `build.rs` 中无 `stdlib/` 路径命中。
  - `cargo build` 通过。
  - `cargo run -p scoop -- test` 通过：1345/1345 targets，1382 checks。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all --all-targets` 通过（857 passed）。
- 与 `PLAN.md` 对应闭合：
  - 闭合 PLAN §9 / P9-T03：旧 stdlib 物理目录与 frontend 注入路径已删除，P9-T02 处理后的 fixture baseline 在 core + lang.string + sysroot support sources 下保持通过。
- 暂时性 failing fixture：无。

## P10：core 中 thread/sync/atomic 引用清理

### P10-T01：把 `__AtomicInt` 系列从 core 迁到 `scoop.unsafe`

- 参考：
  - [`PLAN.md`](./PLAN.md) §4.2 / §9 / P10
  - `sysroot/core.scoop`（搜 `__AtomicInt` / `__atomicIntLoad` / `__atomicIntStore` / `__atomicIntCompareExchange`，约 line 472-484）
  - `sysroot/unsafe.scoop`（line 128-152，已有"Internal atomics" 注释段）
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 中 atomic intrinsic lowering
- 目标：
  - 把 `__AtomicInt` typealias + `__atomicIntLoad/Store/CompareExchange` 三个 intrinsic 从 `sysroot/core.scoop` 迁到 `sysroot/unsafe.scoop`。
  - core 不再含 atomic surface。
- 当前实现入口：
  - `sysroot/core.scoop`（找 `__AtomicInt` 系列，约 line 472-484）
  - `sysroot/unsafe.scoop`
  - `crates/scoopc/src/intrinsics.rs` 中 atomic dispatch entry
- 必须实现的内容：
  1. 把 `__AtomicInt` typealias + `__atomicIntLoad/Store/CompareExchange` 声明从 `sysroot/core.scoop` 剪切到 `sysroot/unsafe.scoop`。
  2. `sysroot/unsafe.scoop` 中 line 128-138（"Internal atomics" 注释段）已经预留位置；放在那段后面。
  3. core 中所有 `__AtomicInt` 调用方（如有）改为显式 `import scoop.unsafe.*` 或 fully qualified `scoop.unsafe.__AtomicInt`。grep `__AtomicInt` 在 `sysroot/` 与 `tests/fixtures/` 找出 callers。
  4. `crates/scoopc/src/intrinsics.rs` 中 atomic intrinsic 的 FQN dispatch（如 `__atomicIntLoad` 的 FQN 路径）从 `scoop.core.__atomicIntLoad` 改为 `scoop.unsafe.__atomicIntLoad`。
  5. owner 测试：现有 atomic fixture（grep `__atomicInt` 在 `tests/fixtures/`）改 import 后仍 pass。
- 必须遵从的约束：
  - 不修改 atomic 的 lowering 行为（仍是 SeqCst LLVM atomic 指令）。
  - 不删除 atomic intrinsic（这一类是 §3.3 (a) 真 intrinsic）。
- 验证：
  1. `grep -n "__AtomicInt\|__atomicInt" sysroot/core.scoop` —— 完全无命中。
  2. `cargo run -p scoop -- test --fixtures <atomic-related-fixtures>` —— 通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - core 不再含任何 atomic surface；`scoop.unsafe` 是 atomic 的唯一来源。
- 依赖：P9-T03。

### P10-T02：删除 `__scoop_thread_spawn_join_resume*` 与相关 runtime 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §4.2 / §9 / P10
  - `sysroot/core.scoop` line 557-560：`__scoop_thread_spawn_join_resume` / `__scoop_thread_spawn_join_resume_u64` 声明
  - `runtime/c/scoop_runtime.c` 中 `scoop_thread_spawn_join_resume*` 实现（grep 定位）
  - `runtime/c/scoop_runtime_api.h` 中对应 X-macro 行
- 目标：
  - 删除 `__scoop_thread_spawn_join_resume` / `__scoop_thread_spawn_join_resume_u64` 在 sysroot / 编译器 / runtime 三处的全部痕迹。
  - 这两个 helper 是 spec §5.5 跨线程 resume 的测试 fixture 用 helper，不属于 Continuation 自身。
- 当前实现入口：
  - `sysroot/core.scoop` line 557-560
  - `runtime/c/scoop_runtime.c`（grep `scoop_thread_spawn_join_resume`）
  - `runtime/c/scoop_runtime_api.h`
  - 测试 fixture（grep `__scoop_thread_spawn_join_resume` 在 `tests/fixtures/`）
- 必须实现的内容：
  1. 删 `sysroot/core.scoop` line 557-560 两个声明。
  2. 删 `runtime/c/scoop_runtime.c` 中 `scoop_thread_spawn_join_resume` / `scoop_thread_spawn_join_resume_u64` / `scoop_thread_spawn_join_compat_resume_u64` / `scoop_thread_spawn_join_resume_transport` 一组实现（按 grep 找出全集；line 不固定）。
  3. 删 `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST` 中对应 X-macro 行（line ~146-150 附近，含 `X(scoop_thread_spawn_join_compat_resume_u64)` / `X(scoop_thread_spawn_join_resume_transport)` / `X(scoop_thread_spawn_join_resume_u64)`）。
  4. 删 `crates/scoopc/src/intrinsics.rs` / `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中对应 dispatch / `declare_runtime_*` 函数。
  5. 处理使用方 fixture：
     - 跨线程 resume 的 spec §5.5 测试 fixture 大概率失效。按 P0-T01 三分类原则处理：
       - 如 fixture 是验证 Continuation 本身（不依赖跨线程语义），改用单线程 resume 验证替代。
       - 如 fixture 是验证跨线程 resume 语义本身，**直接删除**——这部分回归在 thread/sync 重设计时（下一轮）补回。
- 必须遵从的约束：
  - 不删除 `Continuation<R, A, eff E>` 类型本身（保留在 core）。
  - `Continuation` 的 atomic 重入检测（PLAN §3.3 (b) 隐含）仍走原有 intrinsic 路径——本任务**不**触碰。
- 验证：
  1. `grep -rn "scoop_thread_spawn_join_resume\|__scoop_thread_spawn_join_resume" crates/ runtime/ sysroot/` —— 完全无命中。
  2. `cargo build` —— 编译通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 跨线程 resume fixture 已删；其它 Continuation fixture 仍 pass。
- 完成条件：
  - 跨线程 resume 测试 helper 完整退场。
- 依赖：P10-T01。

### P10-T03：验证 core / lang.string 不再隐式依赖 `scoop.thread` / `scoop.sync`

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P10 / 风险条目
  - `sysroot/thread.scoop` / `sysroot/sync.scoop`
  - `sysroot/delegates.scoop`（line 39-63：`lazy(Synchronized)` / `observable` / `vetoable` 当前依赖 `Mutex`）
- 目标：
  - 用 grep / 静态分析方式确认 `sysroot/core.scoop` / `sysroot/string.scoop` / `sysroot/lang_string.scoop` / `sysroot/print.scoop` / `sysroot/progression.scoop`（如已建）等 core 与 lang.string 文件中**不**直接 import 或引用 `scoop.thread.*` / `scoop.sync.*` 类型 / 函数。
  - `scoop.delegates` 自身的依赖原样保留（本轮不动，留给下一轮）。
- 当前实现入口：
  - sysroot 全部 `.scoop` 文件
- 必须实现的内容：
  1. 对 core / lang.string 相关文件运行：
     ```
     grep -nE "scoop\.thread|scoop\.sync|Mutex|CondVar|Once|Thread\b|threadSpawn|mutexCreate|condVarCreate|onceCreate" \
         sysroot/core.scoop sysroot/string.scoop sysroot/lang_string.scoop sysroot/print.scoop sysroot/progression.scoop
     ```
     —— 应完全无命中（除了在注释中引用名字，那些可保留）。
  2. 编译器侧间接依赖检查：
     - `crates/scoopc/src/typecheck/`：搜 `scoop.thread` / `scoop.sync` 的隐式依赖路径（如默认 `lazy` 时 typecheck 期望某类型），如有命中确认是 `scoop.delegates` 路径而非 core/lang.string。
  3. 如发现意外依赖，回到 P10-T01 / P10-T02 修复，再回放本任务。
- 必须遵从的约束：
  - 本任务只验证不修改；如发现违规，回到上游任务修。
- 验证：
  1. 上述 grep 命令无关键命中。
  2. `cargo build` 编译通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - core / lang.string 完全不依赖 scoop.thread / scoop.sync；下一轮重设计这两个 cone 时不会被反向拉扯。
- 依赖：P10-T02。

## P11：测试 helper 迁移

### P11-T01：审查 `__scoop_stackmap_statepoint_smoke` / `__scoop_gc_debug_*` 实际使用方

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P10
  - `sysroot/core.scoop` 中 `__scoop_gc_collect` / `__scoop_gc_debug_alloc_garbage` / `__scoop_gc_debug_heap_object_count` / `__scoop_stackmap_statepoint_smoke` 声明（line 304-312）
  - `tests/fixtures/runtime_gc/`、`tests/fixtures/run-pass/` 中相关 fixture
- 目标：
  - 列出 4 个 helper 的所有 fixture 使用方。
  - 对每个 helper 决策：
    - 迁到 test-only cone（如 `scoop.runtime.test`）
    - 转 `@Extern(abi = "c") @NoGC`（如不涉及 GC ref 进出）
    - 直接删除（如该 helper 已无 fixture 依赖）
- 当前实现入口：
  - `sysroot/core.scoop` line 304-312
  - `tests/fixtures/`
- 必须实现的内容：
  1. grep 4 个 helper 名字在 `tests/fixtures/`，列出每个 fixture 是验证什么行为。
  2. 对每个 helper 写决策（写入完成记录）：
     - `__scoop_gc_collect`：在 P7-T01 已经转 scoop ABI（重命名 `panic` 等的同时处理）。本任务期间应当已经是 `@Extern(abi = "scoop")`；如未，迁移到 `scoop.runtime.test` cone（不再放 core）。
     - `__scoop_gc_debug_alloc_garbage` / `__scoop_gc_debug_heap_object_count`：调试用，`@NoGC` 性质 → 转 `@Extern(abi = "c") @NoGC`，迁到 `scoop.runtime.test` cone。
     - `__scoop_stackmap_statepoint_smoke`：是 stackmap registry 的端到端 smoke，参数 / 返回都不含 GC ref（看声明 `Int`）→ 转 `@Extern(abi = "c") @NoGC`，迁 `scoop.runtime.test` cone。
- 必须遵从的约束：
  - 决策必须基于实际 fixture 使用情况——不**预设**结论。
  - 完成记录必须包含：每个 helper 的 fixture 列表 + 决策 + 决策理由。
- 验证：
  1. grep 4 个 helper 的 fixture 命中数。
  2. 决策表 commit 到 P11-T01 完成记录。
- 完成条件：
  - 4 个 helper 的命运已明确，可由 P11-T02 落实。
- 依赖：P10-T03。

### P11-T02：测试 helper 迁移到 test cone 或 C ABI extern 或删除

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P11
  - P11-T01 决策表
- 目标：
  - 按 P11-T01 决策实施迁移：建立 `scoop.runtime.test` cone（如需要）、改 `@Extern` 形态、改 fixture import、删除无用 helper。
- 当前实现入口：
  - `sysroot/core.scoop`
  - 视决策可能新建 `sysroot/runtime_test.scoop`（package `scoop.runtime.test`）
  - `tests/fixtures/` 中相关 fixture
- 必须实现的内容：
  1. 如决策包含"迁到 test cone"：
     - 新建 `sysroot/runtime_test.scoop`，`package scoop.runtime.test`，声明 `import scoop.core.*`。
     - 迁入对应 helper 声明（按 decided 形态：`@Extern(abi = "c") @NoGC` 等）。
     - `scoop.runtime.test` **不**进入自动 prelude——使用方 fixture 必须显式 `import scoop.runtime.test.*`。
     - 修改使用方 fixture 加 import。
  2. 如决策包含"转 C ABI"：
     - 把 sysroot 声明改为 `@Extern(name = "scoop_xxx", abi = "c")`（不显式叠加 `@NoGC` / `@Unsafe`，因为 C ABI 隐含）。
     - 验证 runtime side 符号名一致。
  3. 如决策包含"删除"：
     - 从 sysroot 删声明。
     - 从 runtime side 删 implementation（如果只此 caller）。
     - 从 fixture 删使用方测试。
  4. owner 测试：
     - `tests/fixtures/runtime_gc/` 中所有 GC 端到端 smoke 仍 pass。
     - `scoop.runtime.test` cone 中的 helper 在用户文件不显式 import 时**不**可见——即默认用户代码用不到这些 helper。
- 必须遵从的约束：
  - 严格按 P11-T01 决策；任何反向变更必须先回写 P11-T01。
- 验证：
  1. `grep -n "__scoop_stackmap_statepoint_smoke\|__scoop_gc_debug_" sysroot/core.scoop` —— 应完全无命中（已迁出或已删）。
  2. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - sysroot core 不再含调试 / 测试 helper；GC fixture 仍 pass。
- 依赖：P11-T01。

## P12：core 真正成为 cone（去 sysroot 化）

### P12-T01：sysroot 全 file 审计——每个 method/fun 满足 body / `@Intrinsic` / `@Extern` 三选一

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P12 + §10 风险条目"P12 去 sysroot 化的隐性依赖"
  - 现 sysroot 全部 file（重组前位于 `sysroot/*.scoop`）
- 目标：
  - 在物理目录重组（P12-T02）与 `signature_only_sysroot_ast` 拆除（P12-T03）之前，先验证 sysroot 中每个 method/fun 都满足"body / `@Intrinsic` / `@Extern` 三选一"。
  - 这是 P12-T03 拆 AST stripping 的前置条件——如有"光声明无 body 也无 `@Intrinsic`/`@Extern`"的 surface，P12-T03 后会编译失败。
- 当前实现入口：
  - `sysroot/`（重组前的扁平结构）：`core.scoop` / `string.scoop` / `print.scoop` / `progression.scoop`（如 P9-T01 已建）/ `lang_string.scoop`（如 P5-T02 已建，可能命名不同）/ `unsafe.scoop` / `thread.scoop` / `sync.scoop` / `delegates.scoop` / `collections.scoop`
- 必须实现的内容：
  1. 对 sysroot 下每个 `.scoop` file 跑结构扫描，列出每个顶层 fun / type body 内 method 的形态：
     - 有 body（普通 Scoop 函数）→ ✓
     - 标 `@Intrinsic` 或 `@Intrinsic("name")`（无 body）→ ✓
     - 标 `@Extern(name = "...", abi = "...")`（无 body）→ ✓
     - 其它（既无 body 也无 `@Intrinsic` / `@Extern`）→ **违规**
  2. 对每条违规：定位它来自哪个 P 阶段任务（在 P5/P7/P8/P10 等任务中是否漏了 body 或 `@Extern` 标注），返回该任务**先**修补，再回到本任务回放。
  3. 完成记录中列出审计结果：file × method 矩阵 + 全部 ✓。
  4. 若审计期间发现某 sysroot file 是空 placeholder（如 P1-T01 建的空 lang_string）——P5-T02 之后该 file 应当已有内容；如仍为空，说明实施中漏了，先回填再回放本任务。
- 必须遵从的约束：
  - 仅做审计 + 必要时返回上游修补；**不**改代码（除非审计期间发现的违规需要立刻回填）。
  - 不引入"sysroot file 暂时光声明"的临时妥协——任何违规必须在本任务结束前彻底修。
- 验证：
  1. 审计脚本 / 手动扫描覆盖 sysroot 全部 `.scoop` file。
  2. 对每个 file 输出 ✓ 或具体违规清单。
- 完成条件：
  - sysroot 全 file 满足"三选一"，可以推进 P12-T02。
- 依赖：P11-T02。

### P12-T02：sysroot 物理目录按 cone FQN 重组

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.1 / §9 / P12
  - `crates/scoopc/src/sysroot/mod.rs::collect_scoop_files`（line 339-356，已经递归扫描子目录）
  - `crates/scoopc/src/sysroot/mod.rs::Sysroot::default_path`（line 53）
  - `crates/scoopc/src/sysroot/mod.rs` tests（line 396 `load_default_sysroot` 等）
- 目标：
  - 把 `sysroot/*.scoop` 按文件的 `package` 声明搬到 `sysroot/<cone-fqn>/<file>.scoop`。
  - loader 已支持递归扫描子目录，无需改加载实现；但 sysroot 内部 tests 与 fixture / 编译器测试中的硬编码路径需要修。
- 当前实现入口：
  - `sysroot/`（顶层扁平结构）
  - `crates/scoopc/src/sysroot/mod.rs::collect_scoop_files`（递归扫描，无需改）
  - `crates/scoopc/src/sysroot/mod.rs` 内 tests
  - 仓库范围内对 `sysroot/<file>.scoop` 路径的硬编码（grep 范围见下）
- 必须实现的内容：
  1. 决定每个 file 的目标位置（按其 `package` 声明）：
     - `sysroot/core.scoop` → `sysroot/scoop.core/core.scoop`（package `scoop.core`）
     - `sysroot/string.scoop` → `sysroot/scoop.core/string.scoop`（package `scoop.core`，是内部 helper file，留在 core 子目录）
     - `sysroot/print.scoop` → `sysroot/scoop.core/print.scoop`（package `scoop.core`）
     - `sysroot/progression.scoop`（如 P9-T01 已建）→ `sysroot/scoop.core/progression.scoop`
     - `sysroot/lang_string.scoop`（P1-T01 / P5-T02 引入）→ 拆成 `sysroot/scoop.lang.string/builder.scoop`（StringBuilder + 三个 string-from-... 入口）+ `sysroot/scoop.lang.string/helpers.scoop`（substring / indexOf / split / trim 等）。**也可以**保持单文件 `sysroot/scoop.lang.string/lang_string.scoop`，按工作进度选——文件粒度无强约束，只要 package 声明正确。
     - `sysroot/unsafe.scoop` → `sysroot/scoop.unsafe/unsafe.scoop`（package `scoop.unsafe`）
     - `sysroot/thread.scoop` → `sysroot/scoop.thread/thread.scoop`（package `scoop.thread`）
     - `sysroot/sync.scoop` → `sysroot/scoop.sync/sync.scoop`（package `scoop.sync`）
     - `sysroot/delegates.scoop` → `sysroot/scoop.delegates/delegates.scoop`（package `scoop.delegates`）
     - `sysroot/collections.scoop` → 视 P10/P11 后还剩什么内容；如 `Iterable/Iterator` 已迁 core、`IntIterable` 已删、剩下只有 `Map`，可以并入 `sysroot/scoop.collections/collections.scoop`（package `scoop.collections`），或者 `Map` 也迁 `scoop.delegates`（它本来就是 delegated property 用的最小 Map 表面）后整文件删。
     - `sysroot/scalar_string_bridge.scoop` 已在 P7-T02 删除，无需迁移。
  2. 用 `git mv` 实施迁移（保留文件历史）。**注意 `git mv` 必须先 `mkdir -p` 目标目录**。
  3. 修硬编码路径：grep 整仓 `sysroot/[a-z_]+\.scoop` 模式：
     - `crates/scoopc/src/sysroot/mod.rs::tests`（line 396 `load_default_sysroot` / 其它 sysroot 测试）—— 多半 hardcoded path 检查
     - `crates/scoopc/src/llvm/tests/`：可能有 `single_file_minimal_ir_includes_compilable_sysroot_string_helpers` 类对 sysroot file 路径的断言
     - `crates/scoopc/src/comptime/interpreter.rs`（line 162 / 177，如有 path string 拼接）
     - 任何 `tests/fixtures/` 中 EXPECT-ERROR 信息或 IR snapshot 含 sysroot file 路径的位置
     - 任何文档 / `MANAGED_ABI.md` / `SCOOP_FULL_SPEC.md` / `PLAN-managed-abi.md` 中提到的 sysroot 文件路径（这些只更新引用即可，不需要本任务改文档主体——P13-T02 / P13-T03 处理）
  4. 验证 loader 仍正确加载所有 cone：跑 `Sysroot::load_from(default_path())`，确认 `index_files()` 返回的 file 集合数量与重组前一致，每个 file 的 `package` 声明能被 resolver 正确识别。
- 必须遵从的约束：
  - 重组**只**是物理位置变化；不改任何 sysroot file 内容（package 声明、import、body 都不变）。
  - 不允许保留 `sysroot/<file>.scoop` 顶层兼容副本——一次性迁移，无桥接。
  - 重组后**仍**保留 `sysroot/` 顶层目录名（不改成 `cones/`）—— 与"将来加 `--sysroot` 参数"的命名一致。
- 验证：
  1. `cargo build` —— 编译通过。
  2. `cargo test -p scoopc sysroot::tests -- --nocapture` —— sysroot 内部测试通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
  4. `find sysroot/ -name '*.scoop' -maxdepth 1`—— 应该没有顶层 `.scoop` 文件（全部已下沉到 `scoop.<cone>/` 子目录）。
  5. `find sysroot/ -mindepth 2 -name '*.scoop' | head -20`—— 列出新位置。
- 完成条件：
  - sysroot 物理结构按 cone FQN 组织；编译器加载行为不变；硬编码路径全部修完。
- 依赖：P12-T01。

### P12-T03：取消 `signature_only_sysroot_ast` / `is_compilable_sysroot_file` 整套 AST stripping

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P12
  - `crates/scoopc/src/sysroot/mod.rs::signature_only_sysroot_ast`（line 197）+ `strip_item_bodies`（line 204）+ `strip_comptime_else_bodies`（line 225）+ `strip_type_decl_bodies`（line 243）+ `strip_object_decl_bodies`（line 255）+ `strip_type_member_bodies`（line 264）
  - `crates/scoopc/src/sysroot/mod.rs::is_compilable_sysroot_file`（line 146）+ `is_always_compilable_sysroot_file`（line 150）+ `has_bodied_intrinsic_nominal_method`（line 160）+ `has_intrinsic_annotation`（line 180）
  - `crates/scoopc/src/sysroot/mod.rs::collect_compilable_sysroot_files`（line 280）
- 目标：
  - 删除 `signature_only_sysroot_ast` 与全部 `strip_*_bodies` helper —— sysroot file 不再有"光签名"形态，与用户 file 一样全编译。
  - 删除 `is_compilable_sysroot_file` 过滤 —— sysroot 全部 file 都参与编译，不区分"哪些 sysroot file 编译、哪些只贡献声明"。
  - 调整 `collect_compilable_sysroot_files` 调用方：所有 sysroot file 都进入"编译列表"。
- 当前实现入口：
  - `crates/scoopc/src/sysroot/mod.rs`（前述行号）
  - `crates/scoopc/src/frontend.rs` line 800（`for (path, is_sysroot) in support_paths`）
  - `crates/scoopc/src/comptime/interpreter.rs` line 162 / 177（载入 sysroot file 的 caller）
- 必须实现的内容：
  1. 删除 `signature_only_sysroot_ast` 函数及其所有 caller。
  2. 删除 `strip_item_bodies` / `strip_comptime_else_bodies` / `strip_type_decl_bodies` / `strip_object_decl_bodies` / `strip_type_member_bodies` 5 个 helper。
  3. 删除 `is_compilable_sysroot_file` / `is_always_compilable_sysroot_file` / `has_bodied_intrinsic_nominal_method` / `has_intrinsic_annotation` 4 个 helper。
  4. 改 `collect_compilable_sysroot_files`（line 280）：
     - 改名 `collect_sysroot_files`（不再有"compilable / non-compilable"二分）。
     - 内部不再做 `is_compilable_sysroot_file` 过滤——所有 `.scoop` file 都返回。
     - 同步更新所有 caller（`crates/scoopc/src/frontend.rs` line 785 等）。
  5. 检查 `sysroot/mod.rs::tests` 中是否有针对 `signature_only` / `compilable filtering` 的测试；删除或改写。
  6. 跑 `cargo build` 确认所有原本依赖"光签名" sysroot file 的编译路径在 P12-T01 审计后已确实无依赖。
- 必须遵从的约束：
  - 不允许保留任何 `strip_*_bodies` / `signature_only_*` / `is_compilable_sysroot_file` 残留代码或别名。
  - P12-T01 审计是 absolute 前置——若 build 失败提示某 sysroot method 缺 body，立即停手回到 P12-T01 修补，**不**临时给该 method 加假 body 或加 `@Intrinsic` 标注。
- 验证：
  1. `grep -rn "signature_only_sysroot_ast\|strip_item_bodies\|strip_type_member_bodies\|is_compilable_sysroot_file\|is_always_compilable_sysroot_file" crates/scoopc/src/`—— 完全无命中。
  2. `cargo build` —— 编译通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - sysroot file 与用户 file 在 AST / 编译路径上完全一致。
- 依赖：P12-T02。

### P12-T04：body 缺失策略统一——sysroot file 与用户 file 用同一规则

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P12
  - `crates/scoopc/src/typecheck/annotations.rs` line 2280（`if !source.is_sysroot() && regular_fun_requires_body(...)`）
- 目标：
  - 删除 typecheck 中 "sysroot file 不要求 body" 的特殊豁免。
  - 所有 file（无论 sysroot / 用户）的 method/fun 必须满足 body / `@Intrinsic` / `@Extern` 三选一——这是前一轮 PLAN-managed-abi P4-T01q 已锁定的"通用约束"，本任务把它的实现层应用到 sysroot file 也无差别。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/annotations.rs` line 2280 附近 `if !source.is_sysroot() && regular_fun_requires_body(fun, ...)` 的整段
  - `regular_fun_requires_body` 函数（同文件，定位需 grep）
- 必须实现的内容：
  1. 删除 `if !source.is_sysroot() && regular_fun_requires_body(...)` 中的 `!source.is_sysroot() &&` 短路条件。改为：
     ```
     if regular_fun_requires_body(fun, &flags, missing_body_policy) {
         // 报错
     }
     ```
     即，无条件应用 `regular_fun_requires_body`，不再区分 sysroot / 用户。
  2. 检查 `regular_fun_requires_body` 内部逻辑——它应当已经识别"`@Intrinsic` 或 `@Extern` 都不要求 body"。如其内部仍有 sysroot 特例，一并清理。
  3. 跑 `cargo build` —— 此时若 sysroot 中仍有"光声明"surface 会立刻报错。前置条件 P12-T01 审计 + P12-T03 拆 stripping 后，应当无报错。
- 必须遵从的约束：
  - 不允许"先把 sysroot 报错临时静默"——任何报错都回到 P12-T01 重审。
- 验证：
  1. `grep -n "is_sysroot()" crates/scoopc/src/typecheck/annotations.rs`—— 仅命中 `source_is_sysroot`（用于 `@AllowIntrinsic` gate；P12-T05 处理）；不再命中 body 缺失策略路径。
  2. `cargo build` 编译通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - sysroot 与用户 file 在 body 缺失策略上完全一致；唯一保留的语义差异在 `@AllowIntrinsic` 自动开 gate（P12-T05 收尾）。
- 依赖：P12-T03。

### P12-T05：`is_sysroot()` 语义收窄——仅保留在 `@file:AllowIntrinsic` 自动开 gate 处使用

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P12
  - `crates/scoopc/src/source.rs::SourceFile::is_sysroot`（line 86）
  - `crates/scoopc/src/typecheck/builtin_annotations.rs::file_allows_intrinsic`（line 135 附近）
  - `crates/scoopc/src/typecheck/annotations.rs::source_is_sysroot`（line 3192-3193）+ line 3180 调用点
- 目标：
  - 把 `SourceFile::is_sysroot()` 的所有 call site 收窄到只剩两处：
    - `crates/scoopc/src/typecheck/builtin_annotations.rs::file_allows_intrinsic`（标准 cone 自动开 `@AllowIntrinsic` gate）
    - `crates/scoopc/src/typecheck/annotations.rs::source_is_sysroot`（同上 gate 路径的 helper）
  - 其它任何位置如果还有 `is_sysroot()` 检查，删除（应当在 P12-T03 / P12-T04 时已经处理掉）。
  - `is_sysroot()` 自身的 API / 命名保留（`SourceFile::is_sysroot`、`SourceFile::load_sysroot`）—— 它仍是表达"该 file 来自标准 cone"的唯一渠道，但语义上从"sysroot 特权"收窄到"标准 cone 标识 + AllowIntrinsic 自动开 gate"。
- 当前实现入口：
  - `crates/scoopc/src/source.rs::SourceFile::{is_sysroot, load_sysroot}`
  - 整仓 grep `is_sysroot()` / `is_sysroot\(\)` 找全 caller
- 必须实现的内容：
  1. grep `is_sysroot` 在 `crates/scoopc/src/`—— 应当只在以下位置剩：
     - `source.rs`（定义 `is_sysroot()` 与 `load_sysroot()`）
     - `frontend.rs` line 800（loader 路径分流——这是物理加载，可保留作为"标识 sysroot file"的源头）
     - `comptime/interpreter.rs`（同上加载路径）
     - `sysroot/mod.rs`（loader 内部调用 `SourceFile::load_sysroot`）
     - `typecheck/builtin_annotations.rs::file_allows_intrinsic`（gate）
     - `typecheck/annotations.rs::source_is_sysroot` 与其 caller line 3180（gate）
  2. 任何**其他**位置如有 `is_sysroot()` 检查影响 typecheck / lowering 行为，删除（应当在 P12-T03/P12-T04 已被清理；这一步是兜底审计）。
  3. 在 `source.rs::is_sysroot` 函数定义旁加一行注释说明：
     ```
     /// 标识该 file 是否来自标准 cone（sysroot）。
     ///
     /// **语义边界**：从 P12 起，本标志的唯一行为影响是"自动开启 `@file:AllowIntrinsic`
     /// gate"——这是标准 cone 作者撰写 intrinsic 声明的便利特权，不是语言后门。其它
     /// 位置（typecheck body 缺失策略、AST stripping、编译列表过滤等）已经统一对待
     /// sysroot 与用户 file。
     ```
  4. 不重命名 `is_sysroot()` —— 命名保留（仍叫 sysroot），但语义收窄。
- 必须遵从的约束：
  - 严格的 grep 审计是验证手段——不允许遗漏任何 caller。
  - 不允许把 `is_sysroot()` 的语义"扩大回去"（如再在 typecheck 中加入"sysroot 视为不同生物"的检查）。
- 验证：
  1. `grep -rn "is_sysroot\(\)\|is_sysroot()" crates/scoopc/src/`—— 命中位置必须严格落在上述列表中。
  2. `cargo build` —— 编译通过。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - `is_sysroot()` 的影响面收窄到唯一一处（`@AllowIntrinsic` gate）。
  - sysroot file 与用户 file 在所有其他维度（AST 形态、body 缺失策略、可见性、编译路径）完全一致。
- 依赖：P12-T04。

## P13：spec 与文档更新

### P13-T01：spec §10.3 删除 `var StringBuilder.lastChar` 示例 + 加入 `scoop.lang` 简介

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P12
  - `SCOOP_FULL_SPEC.md` §10.3（line 1643-1660，`Extension Properties`，含 `var StringBuilder.lastChar` 示例）
- 目标：
  - 删除 §10.3 中错误示例 `var StringBuilder.lastChar: Char get() = ... set(value) { ... }`（StringBuilder 不支持 O(1) indexing，此 demo 是错误的）。
  - 加入 `scoop.lang` 简介（说明 `scoop.lang.string` 与 StringBuilder 最小表面）。
- 当前实现入口：
  - `SCOOP_FULL_SPEC.md` line 1643-1660
- 必须实现的内容：
  1. 删除 §10.3 中 `var StringBuilder.lastChar: Char` 三行示例（line 1654-1657）。可替换为另一个合法的 ext property 示例（如 `var List<T>.lastIndex: Int` —— 等下，line 1651-1652 已有 `val <T> List<T>.lastIndex`；可以不再补新示例）。
  2. 在 `SCOOP_FULL_SPEC.md` 末尾或合适章节加 "scoop.lang 简介" 小节（建议作为独立的 §17 或附加 cone introduction）：
     ```
     ## scoop.lang

     `scoop.lang` 是 Scoop 标准库中的"语言核心扩展"层，提供与语言特性紧密耦合的 surface。

     当前包含子 cone：

     - `scoop.lang.string`：含 `StringBuilder`（f-string desugar 目标）+ 高级 String helper（substring / indexOf / split / trim* 等）。

     `scoop.lang.*` 与 `scoop.core.*` 一同进入自动 prelude——用户源文件不需要显式 import 即可使用。

     ### scoop.lang.string.StringBuilder

     最小表面：

     ```kotlin
     class StringBuilder {
         fun add(s: String): StringBuilder
         fun toString(): String
     }
     ```

     用法：`StringBuilder().add("a").add("b").toString()` 返回 `"ab"`。

     f-string 表达式 `f"a={x}"` 在编译期被 desugar 为 `StringBuilder().add("a=").add(x.toString()).toString()`。
     ```
- 必须遵从的约束：
  - 仅 spec 文档修改，不改代码。
- 验证：
  1. `grep "StringBuilder\.lastChar" SCOOP_FULL_SPEC.md` —— 应无命中。
  2. spec 渲染（如 `mdcat SCOOP_FULL_SPEC.md` 或类似工具）通过。
- 完成条件：
  - spec 不再含错误 StringBuilder.lastChar 示例；StringBuilder 最小表面已正式记入。
- 依赖：P11-T02。

### P13-T02：更新 `MANAGED_ABI.md` §2.2 typical example 列表

- 参考：
  - `MANAGED_ABI.md` §2.2（行 166-186）
- 目标：
  - 更新 §2.2 "典型例子" 列表，反映本轮把 `String.concat` / `String.replace` / `String.repeat` / 标量 toString / `print` / `println` / `panic` 等已经从 "应当走 scoop ABI" 转为 "已经是 scoop ABI helper"。
- 当前实现入口：
  - `MANAGED_ABI.md` §2.2
- 必须实现的内容：
  1. 把 §2.2 列表中 "Int.toString / Bool.toString / Char.toString / Float.toString / String.concat / String.replace / String.repeat" 改为标注"（已落地，本轮 P7）"。
  2. 把"将来的一批 path/io/env/fs/process/time 表面 helper"保留——这些仍是未来工作。
  3. 在 §2.2 下方加一段总结："本轮（core/stdlib reshape）后，scoop ABI 已承接 sysroot 中所有'仅包装 runtime symbol'的 helper；剩下的 sysroot intrinsic 仅限三类：inline IR、GC discipline 特殊待遇、compile-time eval。"
- 必须遵从的约束：
  - 不改 MANAGED_ABI.md 其他章节内容（§3 / §5 / §10 等）。
- 验证：
  1. `cat MANAGED_ABI.md` —— §2.2 段落显示更新后的列表。
- 完成条件：
  - MANAGED_ABI.md 与本轮实施状态对齐。
- 依赖：P13-T01。

### P13-T03：清理 sysroot 文件中的过期 TODO 注释

- 参考：
  - 现 sysroot 各文件中的 `TODO T0143` / `TODO T1317` / `T1325` / `T1502` / `T0146` 等历史工单引用
- 目标：
  - sysroot 中所有引用历史工单 ID（特别是前一轮 PLAN-managed-abi 已经完成的工单）的注释清理。
  - 保留仍有效的设计说明（架构决策、不变式说明等）。
- 当前实现入口：
  - `sysroot/core.scoop` / `sysroot/string.scoop` / `sysroot/lang_string.scoop` / `sysroot/unsafe.scoop` / `sysroot/print.scoop` / `sysroot/progression.scoop` / `sysroot/runtime_test.scoop`（如已建）
- 必须实现的内容：
  1. 对每个 sysroot 文件 grep `TODO T\|T[0-9]+\|TODO`：
  2. 对每条命中：
     - 如引用的工单是前一轮 PLAN-managed-abi 中标记为 [DONE] 的：删除注释或改写为非工单引用形态（保留架构说明部分）。
     - 如引用的工单是已不存在 / 已废弃：删除整段注释。
     - 如是仍有效的"待 future work" 提示：保留但去掉具体工单 ID（改为"待后续优化"等中性描述）。
  3. 完成后再 grep `TODO T[0-9]+` 应该完全无命中（或仅命中本轮 PLAN/TODO 中真实有效的 task ID）。
- 必须遵从的约束：
  - 仅清理过期注释，不改代码逻辑。
  - 保留有架构信息价值的注释（如"该函数因 GC discipline 必须为 intrinsic" 这类说明）。
- 验证：
  1. `grep -rn "TODO T\|T01[0-9][0-9]\|T13[0-9][0-9]\|T15[0-9][0-9]" sysroot/` —— 仅命中本轮真实有效的 task ID。
  2. `cargo build` —— 编译通过（注释删改不影响编译）。
- 完成条件：
  - sysroot 文件不再保留过期工单引用，注释精简清晰。
- 依赖：P13-T02。

### P13-T04：最终 fixture 收尾——所有 fixture 必须通过

- 参考：
  - [`TODO.md`](./TODO.md) "fixture 终态铁律" + "fixture 删除标准" 两条全局约束
  - 各 P 阶段完成记录中"待 P13-T04 处理"清单的累积
  - P0-T01 baseline pass 列表（`target/reshape-baseline/baseline-pass.txt`）作为"原本应当跑通"的参考集合
- 目标：
  - 在 P0~P13 全部其它任务完成、整轮 reshape 即将收尾时，把 `tests/fixtures/` 完整树清扫一遍：
    - 凡是仍存在于仓库中的 fixture，**必须** pass。
    - 凡是 reshape 期间累积的暂时性 failing fixture（前面任务完成记录中"待 P9-T02 / P13-T04 处理"清单），**全部**在本任务结束前处理完毕。
    - 处理方式：**改写至 pass** 或 **按删除论证规则删除**（见 §"必须实现的内容" 第 3 条）。
  - 仓库内不允许留下任何"明知跑不通"的 fixture。
- 当前实现入口：
  - `tests/fixtures/` 全部子目录：`run-pass/` / `typecheck/` / `llvm/` / `build/` / `runtime_gc/` 等
  - 各前置任务完成记录（在 TODO-1.md ~ TODO-5.md 中以 "完成记录" 段标注）
- 必须实现的内容：
  1. **收集 failing fixture 全集**：
     - 跑 `cargo run -p scoop -- test`（全量 fixture）。
     - 把所有 failing fixture 路径汇总成清单 `target/reshape-baseline/p13t04-failing.txt`。
     - 交叉对照各前置任务完成记录中累积的"待处理" fixture，确认无遗漏。
  2. **逐条分类**：对清单中每条 failing fixture，回答两个问题：
     - Q1：fixture 验证的功能 / API / 语义在本轮 reshape 后**是否**仍然存在？
     - Q2：如果存在，fixture 失败的原因是表面 API 改了（import / method 形式 / 语法），还是底层语义本身被有意改变了？
  3. **按答案处理**：
     - **Q1 = 是**：必须**改写** fixture（改 import、改 syntax form、改 method 调用形式至匹配新 surface），让 fixture 在新仓库下 pass。**不允许**删除。
     - **Q1 = 否，且没有等价新对象**：满足 [`TODO.md`](./TODO.md) "fixture 删除标准"。允许删除，但完成记录中必须按 P9-T02 同样的规则写明"被测对象 X 已不存在 / 已被 Y 替代且 Y 由 fixture Z 覆盖"。
     - **Q1 = 否，但有等价新对象**：必须改写到测试新对象（不删）。例：旧 `Set.contains` 删了，但 `scoop.lang.string.StringBuilder.add` 类似行为可作为通用 collection contains 的"替身"——这种情况下 fixture 改写为测试新对象。
     - **Q2 = 底层语义被有意改变**：如果改变本身已经被前面阶段的 baseline 短文（如 P8-T01）锁定，fixture 必须改写至匹配新基线；如果改变没有 baseline 锁定，回到上游任务先回写 baseline 文档，再回到本任务。
  4. **逐条 fix 后立即验证**：每改一条 fixture，跑 `cargo run -p scoop -- test --fixtures <path>` 确认 pass。
  5. **最终全量验证**：
     - `cargo run -p scoop -- test` —— 全量必须 pass，**零 failing fixture**。
     - `cargo build` —— 编译通过、无 warning（前一轮 PLAN-managed-abi 已锁住 `clippy --all-targets -- -D warnings`，本轮保持）。
     - `cargo clippy --all-targets -- -D warnings` 无新增 warning。
  6. 完成记录必须包含：
     - failing fixture 全集（来自步骤 1）
     - 每条 fixture 的 Q1 / Q2 答案与处理决定（改写 / 删除 + 论证）
     - 改写后的运行结果对照
     - 删除条目与上游 P 阶段"待处理" 清单的逐条闭合（每条上游"待处理" fixture 必须能在本完成记录中找到对应处理决定）。
- 必须遵从的约束：
  - **不允许**通过删除 fixture 的方式"消除" failing 状态——必须先逐条回答 Q1 / Q2，再决定改写还是删除。
  - **不允许**留下任何 failing fixture，包括标 `// EXPECT: fail` 但实际行为不符的 fixture——这类 fixture 也属于 failing 状态。
  - **不允许**在本任务期间发现 baseline / spec 与实现不一致就"顺手改实现"——任何实现层变更必须回到上游 P 阶段任务（必要时新建任务），本任务只做 fixture 收尾。
  - 删除决定的论证标准与 [`TODO.md`](./TODO.md) "fixture 删除标准" 严格一致；不接受"fixture 已过时"这种笼统理由。
- 验证：
  1. `cargo run -p scoop -- test` —— **必须** 0 failing。
  2. `cargo clippy --all-targets -- -D warnings` 通过。
  3. `wc -l target/reshape-baseline/p13t04-failing.txt` 在 fix 后应当为 0（或文件可删除）。
  4. 完成记录中"待处理" fixture 与上游各任务的清单逐条对账，无遗漏、无新增。
- 完成条件：
  - 仓库内**零** failing fixture。
  - 所有上游"待处理"清单已闭合。
  - 整轮 reshape 至此完整收尾。
- 依赖：P13-T03 + P0~P12 全部任务。本任务是整轮 reshape 的最后一站。
