# TODO（core / stdlib reshape）：P0 + P1 + P2 + P3：基础设施 / 反射 / MutableArray 升级

> 计划基线：[`PLAN.md`](./PLAN.md)
> 任务索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。
> 全局约束：见 [`TODO.md`](./TODO.md) `## 全局约束` 一节。

## 已落地基线（前一轮成果，本轮不动）

- `ExternAbi::Scoop` 与 native ABI surface 收口：
  - `crates/scoopc/src/hir/mod.rs::ExternAbi`
  - `crates/scoopc/src/typecheck/annotations.rs`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs`
- method-level `@Intrinsic("name")` 表机制：
  - `crates/scoopc/src/intrinsics.rs::{NamedIntrinsicAuditEntry, named_intrinsic_audit_entries, named_intrinsic_audit_entry, fallback_named_intrinsic_entry_name_for_fqn}`
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs::{lower_array_size, lower_array_get, lower_array_set, lower_array_data_ptr}`
- 已迁移到 sysroot ordinary helper / `@Intrinsic` body method 的 String 表面：
  - `sysroot/string.scoop`（`substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` + 内部 `__scoop_string_*` helper）
  - `sysroot/core.scoop` 中 `@Intrinsic class String` body method（`length/toInt/concat/hash/isEmpty/replace/charAt/repeat/compareTo/trimIndent`）
  - `sysroot/scalar_string_bridge.scoop`（audited bridge layer）
  - `sysroot/print.scoop`（generic `print<T>/println<T>` body）
- `Array` / `MutableArray` IR-direct intrinsic：
  - `crates/scoopc/src/intrinsics.rs` 中 `array_size/array_get/array_set/array_data_ptr` 四个 entry
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 中对应 `lower_*` 函数
- 现 `runtime/c/scoop_array.c` 提供：
  - `ScoopArray`（inline trailing data）+ `ScoopArrayBuilder`（out-of-line + cap）双类型
  - `scoop_array_alloc / scoop_array_builder_new / grow / push_u64 / push_ref / push_composite / build_array / build_mutable_array / build_array_composite / build_mutable_array_composite`
- 现 sysroot 9 文件：`core.scoop / string.scoop / scalar_string_bridge.scoop / print.scoop / collections.scoop / delegates.scoop / thread.scoop / sync.scoop / unsafe.scoop`
- 现 stdlib 9 文件：`prelude.scoop / mutable_array.scoop / mutable_array_iter.scoop / mutable_list.scoop / array_iter.scoop / collections_iter.scoop / collections_map.scoop / collections_set.scoop / math.scoop`
- 现 stdlib 注入路径：
  - `crates/scoopc/src/frontend.rs::default_stdlib_path`（line 763）
  - `crates/scoopc/src/frontend.rs::collect_scoop_files`（line 810）
  - sysroot 加载入口：`crates/scoopc/src/sysroot/mod.rs::Sysroot::default_path / load_from / load_from_with_overlay / collect_compilable_sysroot_files`
- 现编译器后门（本轮要清理的目标）：
  - f-string codegen：
    - `crates/scoopc/src/llvm/codegen/main/literal.rs::codegen_interpolated_string`（line 236）
    - `crates/scoopc/src/llvm/codegen/mir_body/string.rs::codegen_mir_interpolated_string`（line 8）+ `codegen_mir_interpolated_expr_segment`（line 212）
    - `crates/scoopc/src/mir/lower/fn_lowering_expr.rs::lower_interpolated_string_expr`（line 86）
  - 数组字面量 HIR lowering：
    - `crates/scoopc/src/hir/lower/expr/canonical_call.rs`（搜 `ArrayLitTarget` / `__scoop_array_builder_*`，约 line 359-851）
    - `crates/scoopc/src/hir/lower/main/impl_lowering.rs::ARRAY_BUILDER_*_FQN`（line 9-21）
  - 二元 / 一元 operator codegen：
    - `crates/scoopc/src/llvm/codegen/mir_body/op.rs::codegen_mir_unary`（line 8）+ `codegen_mir_binary`（line 58）

## P0：冻结 reshape baseline

### [DONE] P0-T01：冻结 reshape baseline 与 fixture 三分类清单

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P0
  - 现 `tests/fixtures/run-pass/`、`tests/fixtures/typecheck/`、`tests/fixtures/llvm/` 全集
- 目标：
  - 把"现在能跑通"的 fixture 集合写成一份白名单；后续每个 P 阶段完成后回放。
  - 把所有 stdlib-dependent fixture 做"保留 / 合并 / 删除"三分类，避免 P9 时漏改或误删。
  - 列出所有 f-string-dependent fixture，确保 P6 desugar 切换后回归覆盖面不下降。
- 当前实现入口：
  - `tests/fixtures/run-pass/stdlib_*.scoop`（14 个文件，见 P0-T01 步骤 2）
  - `tests/fixtures/run-pass/`、`tests/fixtures/typecheck/`、`tests/fixtures/llvm/`、`tests/fixtures/build/`、`tests/fixtures/runtime_gc/` 等子目录全集
  - `crates/scoopc/src/frontend.rs::default_stdlib_path`（line 763）
  - `crates/scoopc/src/frontend.rs::collect_scoop_files`（line 810）—— 用于了解当前 stdlib 注入扫描根
- 必须实现的内容：
  1. 在 `target/reshape-baseline/` 目录下写入三份清单（不进入 git，但任务记录中固化为参考）：
     - `target/reshape-baseline/baseline-pass.txt`：当前全量 fixture pass 的完整列表（运行 `cargo run -p scoop -- test` 默认全量 + parsing pass set，按相对仓库根的 fixture 路径排序）。
     - `target/reshape-baseline/stdlib-fixtures.txt`：每行一个 fixture 路径，后跟分类标签 `KEEP-RENAME` / `MERGE-INTO:<target>` / `DELETE`。
     - `target/reshape-baseline/fstring-fixtures.txt`：所有含 `f"` 或 `f"""` 字面量的 fixture 路径列表。
  2. `stdlib-fixtures.txt` 三分类原则（在文件顶部写明）：
     - `KEEP-RENAME`：fixture 验证的是某项**语言能力**（如 generic class 实例化、特定 syntax form），其引用 `import scoop.core.array.*` 等 stdlib 路径只是配料；P9 时改 import 为 core / lang.string 即可。
     - `MERGE-INTO:<target>`：fixture 与另一个 fixture 验证同一行为；P9 时合并到 `<target>`，本 fixture 删除。
     - `DELETE`：fixture 仅验证旧 stdlib helper 自身的行为（如 `stdlib_collections_set.scoop` 验证 `Set.contains` 实现细节），新 stdlib 重设计后这些测试都要重写；本轮直接删除。
  3. 至少覆盖以下 14 个 stdlib fixture（这是 P0 时点的列表，扫描时若增减以实际为准）：
     - `tests/fixtures/run-pass/stdlib_collections_algorithms_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_hash_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_int_string_conversion_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_iter_algorithms_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_math_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_ranges_enhanced_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_set_map_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_smoke_collections_and_iteration.scoop`
     - `tests/fixtures/run-pass/stdlib_smoke_ranges_and_io.scoop`
     - `tests/fixtures/run-pass/stdlib_smoke_test_and_preconditions.scoop`
     - `tests/fixtures/run-pass/stdlib_string_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_string_builder_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_string_methods_extended.scoop`
  4. 同时扫描所有 fixture 中"非 `stdlib_` 前缀但仍 import 旧 stdlib"的隐式依赖：用 `grep -lE "import scoop\.core\.array|import scoop\.core\.collections|import scoop\.core\.math|import scoop\.core\.range|require\(|check\(" tests/fixtures/`，把命中文件加入 `stdlib-fixtures.txt`。
  5. `fstring-fixtures.txt`：用 `grep -lE 'f"' tests/fixtures/` 收集；另需手动 spot check 至少 5 个，确认它们覆盖了：单 expr / 多 expr / `{Bool}` / `{Int}` / `{Char}` / `{Float}` / `{String}` / 含 `{{` `}}` 的转义 / raw f-string `f"""..."""` 各类 part 类型。
  6. 在任务完成记录中记下：baseline pass 数、stdlib fixtures 三分类的 KEEP-RENAME / MERGE-INTO / DELETE 数量、f-string fixture 总数 + 各 part-type 覆盖度。
- 必须遵从的约束：
  - 本任务**不**改任何代码。只产出清单文件和分类标签。
  - 三个清单必须是后续 P 阶段（尤其 P6 / P9）的唯一回归依据；任何后续阶段对 fixture 的批量动作（删除 / 合并 / 改 import）都必须能在此清单中找到对应分类。
  - `stdlib-fixtures.txt` 的分类是不可逆决定。如果 P9 时发现某条 fixture 应当从 `DELETE` 改为 `KEEP-RENAME`，必须先回写本任务记录、然后才允许改分类。
- 验证：
  1. 运行 `cargo run -p scoop -- test` 跑 baseline；将结果重定向到 `target/reshape-baseline/baseline-pass.txt`。
  2. `wc -l target/reshape-baseline/{baseline-pass,stdlib-fixtures,fstring-fixtures}.txt` —— 三个文件均非空。
  3. 抽样验证：从 `stdlib-fixtures.txt` 随机选 3 条，人工核对分类标签是否合理。
- 完成条件：
  - 三份清单产出且各自标注了清晰的分类规则；后续任务不需要再扫描 fixture 全集。
- 依赖：无

完成记录（2026-05-17）：

- 改动范围：生成 `target/reshape-baseline/baseline-pass.txt`、`target/reshape-baseline/stdlib-fixtures.txt`、`target/reshape-baseline/fstring-fixtures.txt`；更新 `TODO.md` 索引与本任务完成记录；`PLAN.md` 阶段计划未变化。
- baseline：`cargo run -p scoop -- test` 全量通过，`baseline-pass.txt` 记录 1330 个 pass target；raw summary 为 `fixtures: ok (1367)`。
- stdlib fixtures 三分类：共 21 条；`KEEP-RENAME` 8，`MERGE-INTO` 0，`DELETE` 13。14 个 `tests/fixtures/run-pass/stdlib_*.scoop` 已全部覆盖；额外纳入旧 stdlib 注入相关的 range/progression、precondition、scope function、MutableArray/MutableList fixture。
- 核心分类决策：range/progression、String/hash/toString 类行为保留并在后续阶段改名/改 import；`require/check/requireLazy/checkLazy/let/run/also/apply`、旧 MutableArray/List helper、Set/Map、collection algorithm、`joinToString`、`min/max` 等随 `stdlib/` 删除。
- f-string fixtures：实际 f-string fixture 文件 61 个；原始 `f"` 扫描有 72 个命中，其中 11 个是普通字符串或注释中的假阳性，未纳入 `fstring-fixtures.txt`。
- f-string 覆盖度：spot check 覆盖单 expr（`parse/f_string_interpolation.scoop`、`println_string_statepoint_basic.scoop`）、多 expr（`codegen/f_string_interpolation.scoop`、`run_pass_cone/literal_multi_file_interpolation_direct_basic`）、`Bool`/`Int`/`Char`/`Float`/`String`、raw f-string（`parse/f_string_interpolation.scoop`、`string_trim_indent_basic.scoop`）。当前 baseline 没有 `{{` / `}}` 转义 fixture；`P6-T01` 的 owner test 已明确要求新增含转义的 `fstring_desugar_basic.scoop`，该缺口由 P6 接手。
- 验证结果：`wc -l target/reshape-baseline/{baseline-pass,stdlib-fixtures,fstring-fixtures}.txt` 输出 1330 / 32 / 61；stdlib coverage check 输出 `listed=21`、`required_stdlib=14`、`missing_stdlib=0`；抽样核对 `kotlin_require_check_basic.scoop`、`stdlib_math_basic.scoop`、`kotlin_require_check_lazy_message_basic.scoop` 分类合理；`cargo clippy --all-targets -- -D warnings` 通过。
- 与 `PLAN.md` 对应闭合：覆盖 P0 的 T0-1 baseline、T0-2 stdlib/import 分类、T0-3 f-string 扫描；TODO 指定清单落点为 `target/reshape-baseline/` 且不进入 git，因此未执行 PLAN 草案中旧的 `docs/reshape-baseline/` 落点。
- 暂时性 failing fixture：无。

## P1：自动 prelude

### P1-T01：`scoop.lang.string` 空 cone 落地（package + sysroot file + loader 接入）

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.1 / §9 / P1
  - `crates/scoopc/src/sysroot/mod.rs::{Sysroot::default_path, load_from, load_from_with_overlay, index_files, collect_compilable_sysroot_files}`
- 目标：
  - 让 P1-T02 的自动 prelude `import scoop.lang.string.*` 有解析目标，避免引入"待解析的死 import"。
  - 在不引入任何 string-from-... helper 或 StringBuilder 的前提下，把 `scoop.lang.string` cone 的物理位置 / 加载链路落地。
- 当前实现入口：
  - `sysroot/`（目录顶层，9 个 `.scoop` 文件，无子目录）
  - `crates/scoopc/src/sysroot/mod.rs::Sysroot::load_from`（line 57，扫描 sysroot root 下的 `.scoop` 文件）
  - `crates/scoopc/src/sysroot/mod.rs::collect_compilable_sysroot_files`（line 280，driver 收集 sysroot 文件路径）
- 必须实现的内容：
  1. 决定文件布局：在 `sysroot/` 顶层新建 `lang_string.scoop`（**不**新建子目录；当前 `Sysroot::load_from` 是单层扫描，避免本任务顺手扩到递归扫描，那是独立工作）。文件内容仅：
     ```
     // Scoop sysroot: scoop.lang.string cone (placeholder)
     //
     // 说明：本文件目前只声明 package；StringBuilder / string-from-... helper 由 P5 阶段补齐。

     package scoop.lang.string

     import scoop.core.*
     ```
  2. 验证 `Sysroot::load_from(Sysroot::default_path())` 已能识别该文件并放入 `index_files()` 输出。
  3. 在 `crates/scoopc/src/llvm/tests.rs` 或对应 sysroot 测试位置加一条 owner 测试 `lang_string_cone_visible_in_sysroot`，断言：
     - `Sysroot::default_path()` 加载后，`index_files()` 中存在一个 `package == "scoop.lang.string"` 的 file entry。
     - 该 entry 的 export 集合为空（无 type / fun 暴露）。
- 必须遵从的约束：
  - 本任务不改 ImportTable 行为（自动 prelude 在 P1-T02 做）。
  - 不引入 `scoop.lang.string.StringBuilder` 类型声明 / `mutableArrayNew` 引用。
  - 不引入 `scoop.lang.string` cone 的 `@Intrinsic` 或 `@Extern` 声明（同上理由：P5 才有内容）。
- 验证：
  1. `cargo test -p scoopc lang_string_cone_visible_in_sysroot -- --nocapture`
  2. `cargo run -p scoop -- test`（baseline 全量）—— 期望 P0-T01 baseline pass 数无回退。
- 完成条件：
  - `scoop.lang.string` 是一个 well-formed empty cone；后续 P1-T02 / P5 任务可以把 import 与符号往里加。
- 依赖：P0-T01。

### P1-T02：自动 prelude——`scoop.core.*` + `scoop.lang.string.*` 注入 ImportTable

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.2 / §9 / P1
  - `crates/scoopc/src/resolve/imports.rs::ImportTable::build`（line 38）
  - `crates/scoopc/src/source.rs::SourceFile::is_sysroot`（line 86）—— 区分用户文件与 sysroot 文件
- 目标：
  - 用户源文件在 `ImportTable::build` 时自动获得两条 star import：`scoop.core.*` 与 `scoop.lang.string.*`。
  - sysroot 文件**不**自动注入（避免 self-cycle、与现有显式 `import scoop.core.*` 重复）。
  - 用户显式写 `import scoop.core.*` 或 `import scoop.lang.string.*` 等价、不报错、不重复展开。
- 当前实现入口：
  - `crates/scoopc/src/resolve/imports.rs`（整文件；`ImportTable::build` 是核心 API，line 38）
  - `crates/scoopc/src/source.rs::SourceFile::is_sysroot`（line 86）
  - `crates/scoopc/src/resolve/mod.rs::resolve_paths`（line 887 附近的 import 解析路径，用于联调）
- 必须实现的内容：
  1. 在 `ImportTable::build(...)` 入口处，根据 `source.is_sysroot()` 分流：
     - sysroot 文件：保持现有行为不变；不注入。
     - 用户文件：在解析用户 `file.imports` 之前，向内部 import 列表预置两条合成 `Import { path: ["scoop", "core"], has_star: true, alias: None, span: <synthetic> }` 与同形 `scoop.lang.string`。
  2. dedup 策略：
     - 用户显式写的 `import scoop.core.*` / `import scoop.lang.string.*` 与合成项视为等价。
     - `ImportTable` 内部储存采用"合成在前、用户显式在后"的顺序；resolve 阶段按现有"显式优先"规则处理（参考 `resolve/mod.rs` line 2131 附近 "T1310" 注释——显式 import 含 alias 时优先于 star import）。
  3. 合成 import 的 span：定义一个 well-known sentinel span（比如 `Span::synthetic_prelude()`），让诊断信息能区分"自动注入"与"用户写的"。
     - 如果 `Span` 当前没有 sentinel 形态，最小改动是用一个文件起始处 `(0, 0)` 的 zero-width span，并在错误格式化时检查 zero-width + sysroot-virtual flag。
  4. 在 `crates/scoopc/src/resolve/imports.rs` 末尾加 owner 测试：
     - `auto_prelude_injects_core_for_user_file`：用户 `package a` 文件，no explicit imports；构建后 `ImportTable` 至少包含 `scoop.core.*` 与 `scoop.lang.string.*` 两条。
     - `auto_prelude_skips_sysroot_file`：sysroot file（`SourceFile::new_virtual` 加 `mark_sysroot()`，或现有 sysroot loader 路径）—— 构建后 `ImportTable` 不含合成项。
     - `auto_prelude_dedup_with_explicit_user_import`：用户写了 `import scoop.core.*`，构建后该 cone 解析仍为单义（不出现"重复 import"诊断）。
- 必须遵从的约束：
  - 不改 `ImportTable` 公共 API 形态（保持 `pub fn build(...) -> Result<...>` 签名）。
  - 不改 sysroot 文件的 import 解析路径——sysroot 文件继续显式写 `import scoop.core.*` 等，对 `core.scoop` 自身可以省略（self-package）。
  - 不调整其他 cone（`scoop.unsafe` / `scoop.thread` / `scoop.sync` / `scoop.delegates` / `scoop.collections`）的可见性——它们仍需用户显式 import。
- 验证：
  1. `cargo test -p scoopc resolve::imports -- --nocapture`
  2. `cargo run -p scoop -- test`（全量 baseline）—— P0-T01 baseline pass 数无回退；带显式 `import scoop.core.*` 的 fixture 仍 pass。
  3. 抽样：用 `grep -l "^import scoop\.core\.\*$" tests/fixtures/run-pass/*.scoop` 选 5 条，运行单条确认 pass。
- 完成条件：
  - 用户写的 `.scoop` 文件不再需要 `import scoop.core.*` / `import scoop.lang.string.*`，不写也能直接用 `String` / `Int` / `println` / 未来的 `StringBuilder` 等。
  - 所有现有 fixture 仍 pass（显式 import 与合成 import 共存等价）。
- 依赖：P1-T01。

## P2：反射 const fun 补全

### P2-T01：补 `kindOf<T>` / `descOf<T>` + `ARRAY_ELEM_KIND_*` 常量

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.3 (c) / §9 / P2
  - `runtime/c/scoop_array.c` line 19-22：`SCOOP_ARRAY_ELEM_KIND_WORD/REF/COMPOSITE` 三常量定义
  - `sysroot/core.scoop` 中现有反射 intrinsic 声明（`fieldsOf` 等，line 491-505）
  - `crates/scoopc/src/intrinsics.rs` 中现有反射 entry / dispatch
- 目标：
  - 在 core 暴露两个新反射 const fun：`kindOf<T>(): Int` 与 `descOf<T>(): UIntPtr`。
  - 在 core 暴露三个 well-known `const val` 常量：`ARRAY_ELEM_KIND_WORD = 1` / `ARRAY_ELEM_KIND_REF = 2` / `ARRAY_ELEM_KIND_COMPOSITE = 3`。
  - 编译期 const eval 路径补齐：`kindOf<T>` 按 T 的 layout / GC kind 选 1/2/3；`descOf<T>` 在 composite 时返回 transport descriptor 全局地址（`ptrtoint`），其它情形返 0。
- 当前实现入口：
  - `sysroot/core.scoop`：现有反射 intrinsic 区域（约 line 480-505），紧接 `Compiler intrinsics` 注释段
  - `crates/scoopc/src/intrinsics.rs`：根据现有 reflection helper 注册位置追加（grep `nameOf\|sizeOf\|alignOf\|fieldsOf` 确定具体行）
  - `runtime/c/scoop_array.c::scoop_array_element_size`（line 87）/ `scoop_array_element_align`（line 98）—— `kind` 常量来源；保持值与 sysroot 暴露一致
- 必须实现的内容：
  1. 在 `sysroot/core.scoop` 反射 intrinsic 区域添加两个 const fun 声明：
     ```
     @Intrinsic
     const fun <T> kindOf(): Int

     @Intrinsic
     const fun <T> descOf(): UIntPtr
     ```
  2. 在同区域添加三个 `const val`：
     ```
     const val ARRAY_ELEM_KIND_WORD: Int = 1
     const val ARRAY_ELEM_KIND_REF: Int = 2
     const val ARRAY_ELEM_KIND_COMPOSITE: Int = 3
     ```
     字面量的处理：当前 multi-file lowering 对"非入口文件中的 source-backed literals"是否仍有限制需在任务开工时复核（参考 stdlib `__stdlib_int_one` 派生 trick，那里用 `sizeOf(sample) / sizeOf(sample)` 避开字面量）。如该限制仍在，三个常量改写为：
     ```
     const val ARRAY_ELEM_KIND_WORD: Int = sizeOf<Bool>() / sizeOf<Bool>()
     const val ARRAY_ELEM_KIND_REF: Int = ARRAY_ELEM_KIND_WORD + ARRAY_ELEM_KIND_WORD
     const val ARRAY_ELEM_KIND_COMPOSITE: Int = ARRAY_ELEM_KIND_REF + ARRAY_ELEM_KIND_WORD
     ```
     具体方案在任务开工后第一步确认；记入完成记录。
  3. 编译器端 const eval：在 `crates/scoopc/src/intrinsics.rs` 的反射 dispatch 区域加入 `kindOf<T>` / `descOf<T>` 两条 entry。eval 规则：
     - `kindOf<T>`：
       - T 是值类型 scalar（Int/UInt/各 fixed-width int/Float32/Float64/Bool/Char）→ `1`
       - T 是 reference type（class）或 word-sized reference container（`Any` / `String` 等）→ `2`
       - T 是 composite struct（`@CLayout` 或带字段的普通 struct）→ `3`
       - T 是 enum：按其 representation 决定（payload-less enum 是 word；带 payload 的 enum 按现有 representation 路径）—— 任务开工时与 P0-T01 baseline 中 enum-representing fixture 对照确定
       - T 是 generic type parameter（未实例化）→ const eval 失败（编译错误）
     - `descOf<T>`：
       - T 不是 composite → 返 `0`（typed as `UIntPtr`）
       - T 是 composite → 返该类型 transport descriptor 的全局地址。该地址当前在 codegen 阶段才 emit（`ScoopCompositeTransportDescriptor`）；const eval 阶段应当 emit 一个 forward reference，由 codegen pass 在最终发布时替换成实际地址。如果当前 const eval 框架不支持 forward reference，**先**在 P2-T01 暴露声明 + dummy `0` 实现，并在记录中标出 "descOf composite forward-ref 待 P3-T03 实装"；P3-T03 任务依赖 `descOf` 真实工作时再回填。
  4. 加 owner 测试 `crates/scoopc/src/typecheck/...` 或对应 const eval 测试位置：
     - `kind_of_int_returns_word`：`kindOf<Int>()` 在 const context 下 eval 为 1。
     - `kind_of_string_returns_ref`：`kindOf<String>()` 为 2。
     - `kind_of_composite_struct_returns_composite`：定义本地 struct，`kindOf<S>()` 为 3。
     - `desc_of_non_composite_returns_zero`：`descOf<Int>() == 0`。
- 必须遵从的约束：
  - 不改其他反射 intrinsic 的语义。
  - 不引入新的 enum representation；如 enum 的 kind 分类边界模糊，必须先在 P0-T01 baseline 上找有 enum 的回归 fixture 确认现状，再决定。
  - `kindOf` 返回 `Int` 而非新 enum 类型——保持调用方简单（`when (kindOf<T>()) { 1 -> ... 2 -> ... 3 -> ... }`）。
- 验证：
  1. `cargo test -p scoopc kind_of -- --nocapture`
  2. `cargo test -p scoopc desc_of -- --nocapture`
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - P3-T03 的 sysroot 泛型 wrapper 可以直接调用 `kindOf<T>()` / `descOf<T>()`。
- 依赖：P1-T02。

## P3：MutableArray layout 升级

### P3-T01：runtime 端——`ScoopMutableArray` out-of-line layout + 单态 new/push/freeze 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §6.1 / §6.3 / §9 / P3
  - `runtime/c/scoop_array.c::ScoopArray`（line 24，inline trailing data，保留不动）
  - `runtime/c/scoop_array.c::ScoopArrayBuilder`（line 35，out-of-line + cap，本任务的 layout 模板）
  - `runtime/c/scoop_array.c::scoop_array_builder_grow_impl`（line 507，倍数扩容逻辑可直接复用）
  - `runtime/c/scoop_array.c::scoop_array_builder_push_u64/ref/composite`（line 554/568/583）
  - `runtime/c/scoop_array.c::scoop_array_builder_build_common`（line 685，复制到 inline ScoopArray 的逻辑）
  - `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST`（行 80+，所有 runtime 导出符号的 X-macro 总表）
- 目标：
  - 引入新的 `ScoopMutableArray` runtime 类型（layout 见下）。
  - 暴露 6 个 scoop ABI 单态入口：`scoop_mutable_array_new` / `scoop_mutable_array_push_word` / `scoop_mutable_array_push_ref` / `scoop_mutable_array_push_composite` / `scoop_mutable_array_freeze` / `scoop_mutable_array_to_array_data`（如 freeze 拆分实现需要）。
  - 扩容采用倍数策略（ratio = 2，初始 capacity 缺省由 caller 指定；`new(capacity = 0)` 时 runtime 选 `4`）。
  - GC trace 协议：visit `[0, len)` 范围内的 ref / composite-内嵌 ref。
- 当前实现入口：
  - `runtime/c/scoop_array.c`（整文件）
  - `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST`
  - `runtime/c/scoop_gc.h` / `scoop_gc.c` 中 GC trace visitor 接口（参考 `scoop_array_trace_elems`，line 127）
- 必须实现的内容：
  1. 在 `runtime/c/scoop_array.c` 加入新 struct：
     ```c
     typedef struct ScoopMutableArray {
       ScoopGcObjectHeader header;
       uint64_t len;
       uint64_t cap;
       uint64_t elem_size_bytes;
       uint64_t elem_align_bytes;
       const ScoopCompositeTransportDescriptor *elem_desc;
       uint8_t *data;             // out-of-line; freed in finalizer or replaced on grow
       uint32_t elem_kind;        // SCOOP_ARRAY_ELEM_KIND_WORD/REF/COMPOSITE
       uint32_t _reserved_u32;
     } ScoopMutableArray;
     ```
     与 `ScoopArrayBuilder` 几乎一致——本任务实质是把 builder 类型从"内部辅助"晋升为"用户可见的 MutableArray"。
  2. 加 `_Static_assert` 锁定字段 offset（参考现 `ScoopArray` 的一组 assert，line 47-66）。
  3. 实现 6 个 runtime 入口：
     - `void *scoop_mutable_array_new(uint32_t elem_kind, uint64_t elem_size, uint64_t elem_align, const void *elem_desc, uint64_t capacity)` —— 分配 `ScoopMutableArray` GC 对象 + 初始 `data` 缓冲（`max(capacity, 4) * elem_size`）。`capacity == 0` 时缺省为 4。
     - `void scoop_mutable_array_push_word(void *arr, uint64_t value)` —— 扩容 / `((uint64_t*)arr->data)[arr->len++] = value`。
     - `void scoop_mutable_array_push_ref(void *arr, void *value)` —— 同上 + GC write barrier（参考现 `scoop_array_builder_push_ref` 的 barrier 调用）。
     - `void scoop_mutable_array_push_composite(void *arr, const void *slot_ptr, uint64_t elem_size)` —— `memcpy(arr->data + arr->len * elem_size, slot_ptr, elem_size)` + 若 composite 内嵌 ref 触发 barrier（细节遵循现有 composite handling）。
     - `const void *scoop_mutable_array_freeze(void *arr)` —— 把 MutableArray 内容拷贝到一个新分配的 inline `ScoopArray` 对象（按 `len` 大小，**不**带 capacity slack），返回 `const ScoopArray *`。源 MutableArray 不修改（caller 决定是否丢弃）。
     - 内部 helper `scoop_mutable_array_grow`（私有）：复用 `scoop_array_builder_grow_impl` 的算法（line 507）；ratio = 2、初始 4。
  4. GC trace：在 `scoop_array.c` 实现 `static uint64_t scoop_mutable_array_trace_elems(void *object, ScoopGcTraceVisitor visitor, void *ctx)` —— 遍历 `[0, len)`、按 `elem_kind` 分流（WORD 不 visit、REF 直接 visit、COMPOSITE 委托 `elem_desc` 的 transport descriptor）。注册到 GC backend（参考 `ScoopArrayBuilder` 的注册路径——但 `ScoopArrayBuilder` 当前是否有独立 trace 还是借用？需先看代码再选最一致的注册形式）。
  5. 在 `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST` 中加入 6 个新符号的 `X(scoop_mutable_array_*)` 行。
  6. 写 runtime 端 C 单元测试（如果项目当前有 runtime C 单测的话，参考 `runtime/c/scoop_test.c` 风格）：
     - `mutable_array_new_creates_with_capacity`
     - `mutable_array_push_word_grows_amortized`：连续 push 1024 次，断言 grow 次数 == 9（log2(1024/4) + 1 取上界，具体值在实现中确认）
     - `mutable_array_freeze_yields_correct_inline_array`
- 必须遵从的约束：
  - **不**修改 `ScoopArray`（inline trailing data）的 layout—— `Array<T>` 仍需 cache-friendly 紧凑布局。
  - **不**移除 `ScoopArrayBuilder` 类型（这一步在 P4-T02 做；本任务期间 `ScoopArrayBuilder` 与 `ScoopMutableArray` 并存）。
  - 新 6 个入口必须满足 scoop ABI v1 contract：顶层函数、`Pure`（caller 视角不会再触发 unrelated 副作用，alloc 触发 GC 是 caller-visible 的）、参数 / 返回值通过 ordinary managed call boundary。
  - 扩容期间不允许 visit 旧 `data` buffer（避免 GC 看到野指针）—— 实现时确保 `len/cap/data` 三字段的更新顺序对 GC 安全（可以是 prepare new buffer → memcpy → atomic swap data ptr → free old buffer，或借用 stop-the-world allocation point 的语义；参考现 `scoop_array_builder_grow_impl` 是怎么做的）。
- 验证：
  1. runtime C 编译：从仓库根 `cd runtime/c && <按 build.rs 调用方式编译>`；P3 任务期间允许通过 `cargo build -p scoop_runtime` 触发。
  2. `cargo build` —— 整仓编译通过。
  3. 如果有 runtime C 单测，跑这一组：`<具体 invocation 待 P3-T01 开工时确认>`
  4. P0-T01 baseline 全量 —— 此时 `ScoopMutableArray` 还没有 sysroot surface，应保持 baseline 数。
- 完成条件：
  - 6 个新 runtime 符号在 `scoop_runtime_api.h` 中可见、编译通过。
  - `ScoopArrayBuilder` 仍然工作（数组字面量与 stdlib `MutableArray.push` 等都不变）。
- 依赖：P0-T01。

### P3-T02：编译器端——`array_size/get/set/data_ptr` 按 receiver layout 分流

- 参考：
  - [`PLAN.md`](./PLAN.md) §6.2 / §9 / P3
  - `crates/scoopc/src/intrinsics.rs` line 94-115：`array_size/get/set/data_ptr` entry 的 NamedIntrinsicAuditEntry 注册
  - `crates/scoopc/src/intrinsics.rs` line 274-278：`fallback_named_intrinsic_entry_name_for_fqn` 中 `Array.size/get` vs `MutableArray.size/get/set` 的 FQN 映射
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs::lower_array_size/get/set/data_ptr`（line 50-65 附近）
  - `runtime/c/scoop_array.c::ScoopArray`（inline）vs `ScoopMutableArray`（out-of-line，by P3-T01 引入）
- 目标：
  - 让 `array_size` 等四个 entry 在 lowering 阶段按 receiver 类型分流：Array 走 inline 路径（GEP 到 trailing data）、MutableArray 走 indirect 路径（先 load `data` 字段，再 GEP）。
- 当前实现入口：
  - `crates/scoopc/src/intrinsics.rs::NamedIntrinsicAuditEntry`（line 80 附近的 struct + line 94-117 的四条 array entry）
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs::{lower_array_size, lower_array_get, lower_array_set, lower_array_data_ptr}`
- 必须实现的内容：
  1. 决定 dispatch 形态。两条路（任务开工时选其一并记入完成记录）：
     - **(a)** 在四个现有 entry 内部按 receiver 类型分流（`lower_array_size` 内 `match` receiver 是 Array 还是 MutableArray）。表保持 4 条 entry。
     - **(b)** 拆成 8 条 entry：`array_size_inline / array_size_outofline / array_get_inline / ...`，由 `fallback_named_intrinsic_entry_name_for_fqn` 按 `scoop.core.Array.size` / `scoop.core.MutableArray.size` 分别返回不同 entry name。
     倾向 **(b)**——更符合 method-level intrinsic 表"每条 entry 是一个独立 lowering 单元"的精神，且把"当前 entry 内部多分支"的隐式特判明示化。
  2. 实现 lowering：
     - inline 路径（`Array<T>`）：保持现有 `ScoopArray` GEP 行为，无变化。
     - out-of-line 路径（`MutableArray<T>`）：
       - `size`：load `len` 字段（offset = sizeof(ScoopGcObjectHeader)）。
       - `get`：load `data` 指针字段 → GEP 到 `data + idx * elem_size` → load 元素。
       - `set`：同上，且当 `elem_kind == REF` 时 emit GC write barrier 调用（与 `scoop_mutable_array_push_ref` 的 barrier 路径**保持一致**——查看 P3-T01 实现，复用 barrier helper symbol）。
       - `data_ptr`：直接返回 `data` 字段的值（`UIntPtr`）。
  3. 把 `Array<T>` 与 `MutableArray<T>` 的 LLVM struct 类型放进 `crates/scoopc/src/llvm/codegen/types.rs` 或对应类型 builder 模块。`Array<T>` 沿用现 `ScoopArray` 形状；`MutableArray<T>` 按 P3-T01 的 `ScoopMutableArray` 形状（out-of-line + cap + data ptr）。
  4. owner 测试（可加在 `crates/scoopc/src/llvm/tests/`）：
     - `mutable_array_size_loads_len_field`
     - `mutable_array_get_indirect_through_data_ptr`
     - `mutable_array_set_emits_write_barrier_for_ref_element`
     - `array_size_still_inline_after_dispatch_split`
- 必须遵从的约束：
  - **不**改 `Array<T>` 的 LLVM struct 形态——任何 IR snapshot 中针对 Array 的 GEP 序列必须保持完全一致。
  - 不在本任务暴露 `MutableArray.push` / `mutableArrayNew`（这些是 P3-T03）。
  - 不删 `ScoopArrayBuilder` 编译器端 lowering（P4-T02）。
- 验证：
  1. `cargo test -p scoopc llvm_tests -- mutable_array_ -- --nocapture`
  2. `cargo test -p scoopc array_size_still_inline -- --nocapture`
  3. `cargo run -p scoop -- test`（全量 baseline）—— 此时仍走旧 array literal 路径（`__scoop_array_builder_*`），且 `MutableArray.set` 已可工作；预期 baseline 数无回退。
- 完成条件：
  - 同一对 `Array.size` / `MutableArray.size` 调用产生不同 IR shape；`Array<T>` 路径无 IR drift。
- 依赖：P3-T01。

### P3-T03：sysroot 泛型 wrapper——`mutableArrayNew<T>` / `MutableArray<T>.push` / `MutableArray<T>.freeze`

- 参考：
  - [`PLAN.md`](./PLAN.md) §6.3 / §9 / P3
  - `sysroot/core.scoop::MutableArray<T>`（line 147，现有 intrinsic class 声明）
  - `sysroot/unsafe.scoop`（raw pointer ops，stackAlloc/store/load 等可用 primitive）
  - P2-T01 引入的 `kindOf<T>` / `descOf<T>` / `ARRAY_ELEM_KIND_*` 常量
  - P3-T01 引入的 6 个 runtime 单态入口
- 目标：
  - 在 `sysroot/core.scoop` 暴露三个普通 Scoop 泛型函数：`mutableArrayNew<T>(capacity: Int = 0): MutableArray<T>`、`MutableArray<T>.push(value: T): Unit`、`MutableArray<T>.freeze(): Array<T>`。
  - 三者**不是**新 intrinsic——它们是普通 Scoop 函数，body 内：调反射 const fun + 调单态 scoop ABI runtime 入口。编译器无需为它们加任何 special-case。
- 当前实现入口：
  - `sysroot/core.scoop::MutableArray<T>`（class 声明区域）
  - `sysroot/unsafe.scoop::stackAlloc<T>`（line 62）
  - `sysroot/unsafe.scoop::Ptr<T>.store/load/cast`（line 35-44）
  - `sysroot/unsafe.scoop::ptrToUIntPtr`（line 81）
- 必须实现的内容：
  1. 在 `sysroot/core.scoop` 加 6 个 `@Extern(abi = "scoop")` 声明（顶层函数）：
     ```
     @Extern(name = "scoop_mutable_array_new", abi = "scoop")
     fun __scoop_mutable_array_new(
         elemKind: Int,
         elemSize: Int,
         elemAlign: Int,
         elemDesc: UIntPtr,
         capacity: Int,
     ): MutableArray<Any>

     @Extern(name = "scoop_mutable_array_push_word", abi = "scoop")
     fun __scoop_mutable_array_push_word(arr: MutableArray<Any>, value: UIntPtr): Unit

     @Extern(name = "scoop_mutable_array_push_ref", abi = "scoop")
     fun __scoop_mutable_array_push_ref(arr: MutableArray<Any>, value: Any): Unit

     @Extern(name = "scoop_mutable_array_push_composite", abi = "scoop")
     fun __scoop_mutable_array_push_composite(arr: MutableArray<Any>, slot: UIntPtr, elemSize: Int): Unit

     @Extern(name = "scoop_mutable_array_freeze", abi = "scoop")
     fun __scoop_mutable_array_freeze(arr: MutableArray<Any>): Array<Any>
     ```
  2. 在 `sysroot/core.scoop` 加 3 个普通 Scoop 泛型 wrapper：
     ```
     fun <T> mutableArrayNew(capacity: Int = 0): MutableArray<T> {
         val raw: MutableArray<Any> = __scoop_mutable_array_new(
             kindOf<T>(),
             sizeOf<T>(),
             alignOf<T>(),
             descOf<T>(),
             capacity,
         )
         return @Unsafe do { __scoop_unsafe_mutable_array_cast<T>(raw) }
     }

     fun <T> MutableArray<T>.push(value: T): Unit {
         when (kindOf<T>()) {
             ARRAY_ELEM_KIND_WORD -> {
                 val word: UIntPtr = @Unsafe do { __scoop_unsafe_value_to_word<T>(value) }
                 __scoop_mutable_array_push_word(@Unsafe do { __scoop_unsafe_mutable_array_erase<T>(this) }, word)
             }
             ARRAY_ELEM_KIND_REF -> {
                 __scoop_mutable_array_push_ref(@Unsafe do { __scoop_unsafe_mutable_array_erase<T>(this) }, value as Any)
             }
             ARRAY_ELEM_KIND_COMPOSITE -> {
                 val slot: Ptr<T> = stackAlloc<T>()
                 slot.store(value)
                 __scoop_mutable_array_push_composite(
                     @Unsafe do { __scoop_unsafe_mutable_array_erase<T>(this) },
                     ptrToUIntPtr(slot),
                     sizeOf<T>(),
                 )
             }
         }
     }

     fun <T> MutableArray<T>.freeze(): Array<T> {
         val raw: Array<Any> = __scoop_mutable_array_freeze(@Unsafe do { __scoop_unsafe_mutable_array_erase<T>(this) })
         return @Unsafe do { __scoop_unsafe_array_cast<T>(raw) }
     }
     ```
  3. `__scoop_unsafe_mutable_array_cast` / `__scoop_unsafe_mutable_array_erase` / `__scoop_unsafe_value_to_word` / `__scoop_unsafe_array_cast` 这一组 primitive 的实装：
     - 选项 A：直接 `as` cast（如果当前 `as` 已经支持 ref-type erase / specialize）
     - 选项 B：在 `sysroot/unsafe.scoop` 暴露这一组（标 `@Intrinsic @NoGC @Unsafe`），编译器 lowering 成 ref-cast / `bitcast` no-op
     - 任务开工时先尝试 A；若 typecheck 拒绝（generic ref cast 不允许），再切到 B。**不**为 MutableArray 这一个泛型 wrapper 引入新的 special-case lowering——B 是通用 substrate（与现有 `Ptr<T>.cast<U>` 同等地位）。
  4. owner 测试（位于 `tests/fixtures/run-pass/`）：
     - `lang_mutable_array_new_int_word`：`mutableArrayNew<Int>(capacity = 4)` 后 push 4 个 Int，`size()` 返 4，逐项 `get` 与原值一致。
     - `lang_mutable_array_new_string_ref`：同上，T = String，验证 ref 路径 + write barrier 不破坏 GC。
     - `lang_mutable_array_new_struct_composite`：定义本地 `struct Point(val x: Int, val y: Int)`，push + 取回，验证 composite path。
     - `lang_mutable_array_grow_amortized`：push 1024 次，统计 GC 触发次数应有上界（具体值参考 P3-T01 实现）。
     - `lang_mutable_array_freeze_to_immutable`：push 后 `freeze()` 得到 `Array<T>`，对原 MutableArray 继续 push 不影响 frozen 副本。
- 必须遵从的约束：
  - 三个 wrapper 都是普通 Scoop 函数；编译器不为它们加 dispatch 表 entry。
  - `kindOf<T>()` 的 `when` 分支应当被 const eval 裁剪掉无关 case（编译期常量分支消除）—— 这是 P2-T01 const eval 的能力。如果当前 const eval 对 generic instantiation 后的 `kindOf<T>()` 还无法做 dead-branch elimination，本任务**不**为此扩展 const eval；wrapper 接受 runtime branch 的额外开销，留作后续优化。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/lang_mutable_array_*.scoop`
  2. `cargo run -p scoop -- test`（全量 baseline）
- 完成条件：
  - 用户 / sysroot 代码可以写 `val xs = mutableArrayNew<String>(capacity = 8); xs.push("a"); xs.push("b"); val arr = xs.freeze()`。
  - `__scoop_array_builder_*` 旧路径仍存在（数组字面量不变），与新 wrapper 并存。
- 依赖：P2-T01、P3-T01、P3-T02。
