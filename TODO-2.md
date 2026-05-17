# TODO（core / stdlib reshape）：P4 + P5：数组字面量 desugar + scoop.lang.string cone 建立

> 计划基线：[`PLAN.md`](./PLAN.md)
> 任务索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。
> 全局约束：见 [`TODO.md`](./TODO.md) `## 全局约束` 一节。
## P4：数组字面量 desugar 切换 + 删除 builder

### [DONE] P4-T01：数组字面量 HIR desugar 切换到 `mutableArrayNew + push + freeze` 路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §6.4 / §9 / P4
  - `crates/scoopc/src/hir/lower/expr/canonical_call.rs` 中数组字面量 lowering（约 line 359-619；含 `ArrayLitTarget` enum）
  - `crates/scoopc/src/hir/lower/main/impl_lowering.rs::ARRAY_BUILDER_*_FQN`（line 9-21，5 个 well-known FQN 常量）
  - P3-T03 引入的 `mutableArrayNew<T>` / `MutableArray<T>.push` / `MutableArray<T>.freeze`
- 目标：
  - 把 `[a, b, c]: Array<T>` 的 HIR desugar 从"`__scoop_array_builder_new + push + build_array`"切换为"`mutableArrayNew<T>(capacity = 3).push(a).push(b).push(c).freeze()`"。
  - 把 `[a, b, c]: MutableArray<T>` desugar 切换为"`mutableArrayNew<T>(capacity = 3).push(a).push(b).push(c)`"（不调 `freeze`）。
  - 字面量长度作为 capacity hint 传入，避免 push 期间扩容。
- 当前实现入口：
  - `crates/scoopc/src/hir/lower/expr/canonical_call.rs`：现有 `ArrayLitTarget::Array` / `ArrayLitTarget::MutableArray` 分流
  - `crates/scoopc/src/hir/lower/expr/canonical_call.rs` line 762-851：vararg spread 也走同一条 builder 路径（vararg 属于另一种使用场景，按 P4-T01 步骤 6 处理）
- 必须实现的内容：
  1. 在 HIR 阶段加新的 desugar helper `lower_array_literal_via_mutable_array(elements: &[Expr], target: ArrayLitTarget, span: Span) -> Expr`：
     - emit `let __array_lit_tmp = mutableArrayNew<T>(capacity = N)`，N 是 element 个数
     - 对每个 element emit `__array_lit_tmp.push(elem_i)`（顺序）
     - `target == Array`：最终 emit `__array_lit_tmp.freeze()`
     - `target == MutableArray`：最终 emit `__array_lit_tmp`
     - 返回 desugared 表达式
  2. 把 `canonical_call.rs` 中 `ArrayLitTarget::Array` / `ArrayLitTarget::MutableArray` 两个分支的 desugar 入口切换到新 helper。
  3. T 的推断：保留现有"基于 element type 联合 + LHS expected type"的推断路径——desugar 切换不改类型推断，只换 desugar 形态。把推出的 T 作为 `mutableArrayNew<T>` 的显式 type argument。
  4. 空字面量 `[]`：desugar 为 `mutableArrayNew<T>(capacity = 0).freeze()`（或 `mutableArrayNew<T>()` 取默认 capacity）。runtime 端 P3-T01 已经处理 `capacity == 0`（缺省 4）。空字面量不期望立即扩容。
  5. **不**触碰 `__scoop_array_builder_*` 旧符号——本任务期间它们仍存在，但所有数组字面量的 desugar 都不再使用它们。P4-T02 才统一删。
  6. vararg spread / 函数调用展开的 builder 路径（line 762-851）：
     - 如果 vararg 的实现需要"先收集到一个可变缓冲再 build"，本任务一并切换到 `mutableArrayNew + push + freeze`。
     - 该路径与数组字面量是同一类语义，应当复用 P4-T01 的 desugar helper（参数化"是否 freeze"）。
  7. 加 owner 测试：
     - `array_literal_desugars_to_mutable_array_freeze`（HIR snapshot 测试）：`val xs: Array<Int> = [1, 2, 3]` 的 lowered HIR 含 `mutableArrayNew<Int>` 调用 + 3 次 `push` + `freeze`，**不**含 `__scoop_array_builder_*`。
     - `mutable_array_literal_skips_freeze`：`val xs: MutableArray<Int> = [1, 2, 3]` 的 lowered HIR 不含 `freeze` 调用。
     - `array_literal_capacity_matches_element_count`：snapshot 中 `mutableArrayNew<Int>(capacity = 3)` 的 capacity 实参确为 3。
     - `empty_array_literal_desugars`：`val xs: Array<Int> = []` 通过编译且 `xs.size() == 0`。
- 必须遵从的约束：
  - 推断逻辑零变更——T 怎么推现在怎么推。
  - capacity hint 必须是字面量的元素个数；不允许"取一个固定值 + 后续扩容"的次优实现。
  - vararg spread / 数组字面量必须共用 desugar helper，避免出现两条形态略有差异的路径。
- 验证：
  1. `cargo test -p scoopc array_literal_desugar -- --nocapture`
  2. `cargo run -p scoop -- test`（全量 baseline）—— 数组字面量相关 fixture 应全部 pass；IR snapshot 形态会大量变化，但运行结果应一致。
  3. 抽样：选 5 个含 `[...]` 字面量的 P0-T01 baseline fixture 单独跑一遍，确认 stdout 与 baseline 一致。
- 完成条件：
  - HIR 阶段不再 emit `__scoop_array_builder_*` 调用（旧符号在 sysroot/runtime 里仍存在但已无 caller）。
  - 全量 fixture 运行结果不退化。
- 依赖：P3-T03。

完成记录（2026-05-17）：

- 改动范围：`crates/scoopc/src/hir/lower/expr/canonical_call.rs` 中数组字面量和 vararg 合成数组统一改为 `mutableArrayNew(capacity=N)` + `scoop.core.push` + 可选 `scoop.core.freeze`；`crates/scoopc/src/hir/lower/util/generic_funs.rs` 支持从返回类型推断合成泛型调用实例；MIR array transport metadata 覆盖 `scoop.core.push/freeze`；相关 Rust 测试、HIR/MIR golden 和 LLVM 断言同步更新；新增 `tests/fixtures/run-pass/array_literal_empty_desugar.scoop`。
- 核心决策：保留现有数组元素类型推断入口，只替换 lowering 形态；capacity hint 使用元素个数的 `SynthInt`；`Array<T>` 字面量 freeze，`MutableArray<T>` 字面量直接返回临时 `MutableArray<T>`；旧 `__scoop_array_builder_*` 声明/实现不在本任务删除，留给 P4-T02。
- 验证结果：`cargo test -p scoopc array_literal_desugar -- --nocapture` 通过；5 个含数组字面量 run-pass fixture 抽样通过；`cargo run -p scoop -- test` 通过（1375 checks）；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 与 `PLAN.md` 闭合：完成 P4 的数组字面量 desugar 切换部分；P4-T02 继续负责删除旧 builder surface。
- 暂时性 failing fixture：无。

### [DONE] P4-T02：删除 `__scoop_array_builder_*` 整套

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P4 任务 T4-3
  - `sysroot/string.scoop` 中 `__scoop_array_builder_push_string` / `__scoop_array_builder_build_array_string` 声明（line 12-17）
  - `sysroot/string.scoop::String.split`（line 481-525）—— builder 唯一剩下的非数组字面量 caller
  - `stdlib/mutable_array.scoop` 全文（line 27-44 声明 + 全部 push/pop/insert/removeAt/splice 实现）—— P9 删除时一并处理，但本任务期间它仍依赖 builder
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs::SCOOP_ARRAY_BUILDER_*`（line 6-16，8 条常量）
  - `crates/scoopc/src/hir/lower/main/impl_lowering.rs::ARRAY_BUILDER_*_FQN`（line 9-21，5 条常量）
  - `runtime/c/scoop_array.c::scoop_array_builder_*`（line 507-820 一组实现 + `ScoopArrayBuilder` struct line 35）
  - `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST`（包含 9 条 `scoop_array_builder_*` 与 `scoop_array_alloc`）
  - `crates/scoopc/src/intrinsics.rs` 中 `__scoop_array_builder_*` 相关 named intrinsic / FQN dispatch（如有）
- 目标：
  - 删除 `__scoop_array_builder_*` 在 sysroot / 编译器 / runtime 三处的全部声明 / 实现 / dispatch。
  - 删除独立的 `ScoopArrayBuilder` runtime 类型（其角色已被 `ScoopMutableArray` 接管）。
  - 把 `String.split` 重写为基于 `MutableArray<String>.push + freeze` 的形式。
- 当前实现入口：
  - `sysroot/string.scoop`（line 11-17）
  - `sysroot/string.scoop::String.split`（line 481-525）
  - `stdlib/mutable_array.scoop`（整文件；P9 删除）
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `crates/scoopc/src/hir/lower/main/impl_lowering.rs`
  - `runtime/c/scoop_array.c`
  - `runtime/c/scoop_runtime_api.h`
- 必须实现的内容：
  1. **重写 `String.split`**（在 `sysroot/string.scoop`）：
     ```
     fun String.split(delimiter: String): Array<String> {
         val slen: Int = this.byteLength()
         val dlen: Int = delimiter.byteLength()

         val parts: MutableArray<String> = mutableArrayNew<String>(capacity = 4)

         if (slen <= 0) {
             val empty: String = @Unsafe do { this.unsafeSliceBytes(0, 0) }
             parts.push(empty)
         } else if (dlen <= 0) {
             parts.push(this)
         } else {
             // ... 沿用原 byte-level 扫描循环；每次匹配处 parts.push(seg)
         }

         return parts.freeze()
     }
     ```
     注意：现 `String.split` 内含 `__scoop_array_builder_push_string` / `__scoop_array_builder_build_array_string` 两个 string-specialized builder 入口（声明在 `sysroot/string.scoop` line 12-17）—— **重写后这两条声明也一并删除**。
  2. **删 sysroot 声明**：
     - `sysroot/string.scoop` line 11-17：两个 `__scoop_array_builder_*_string` 声明删除。
     - `stdlib/mutable_array.scoop` 整文件保持原样（P9 一同删除）；本任务期间 stdlib 还在引用 `__scoop_array_builder_*`——这意味着 P4-T02 时**先**让 stdlib 失效（编译失败），再在 P4-T02 内部把 stdlib `mutable_array.scoop` 这一文件也删掉。理由：P9 才正式"删 stdlib 全目录"，但 stdlib 中 `mutable_array.scoop` 是 builder 的最大用户、不能让它在中间状态拖住主任务；其它 stdlib 文件（`collections_*` 等）不依赖 builder，可继续保留到 P9。
       - 子任务：把 `stdlib/mutable_array.scoop` 整个文件**移除**；把 `tests/fixtures/run-pass/` 中显式调用 `MutableArray<Int>.push/pop/insert/removeAt/splice` 的 fixture 标记为"P4-T02 期间转入 P0-T01 三分类清单的 DELETE 或 KEEP-RENAME"——这一步在 P4-T02 完成记录里登记。
  3. **删编译器 lowering**：
     - `crates/scoopc/src/hir/lower/main/impl_lowering.rs::ARRAY_BUILDER_*_FQN` 5 条常量删除。
     - 任何引用这些 FQN 的代码路径（grep `ARRAY_BUILDER_NEW_FQN` / `ARRAY_BUILDER_PUSH_FQN` 等）一并清理。
     - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs::SCOOP_ARRAY_BUILDER_*` 一组常量删除（line 6-16，8 条）。
     - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中所有 `declare_runtime_array_builder_*` 函数删除（grep `array_builder` 在 codegen/ 下确定）。
     - `crates/scoopc/src/intrinsics.rs` 中如有 `__scoop_array_builder_*` 相关 named intrinsic / FQN fallback 一并删除。
  4. **删 runtime**：
     - `runtime/c/scoop_array.c`：删除 `ScoopArrayBuilder` struct（line 35-45）、`_Static_assert` 块、`scoop_array_builder_grow_impl`（line 507）、`scoop_array_builder_grow / new / push_u64 / push_ref / push_composite / build_array / build_mutable_array / build_array_composite / build_mutable_array_composite`（line 532-820 范围）、`scoop_array_alloc`（line 632，如无其他 caller）、`scoop_array_builder_trace_elems`（line 166 附近，如存在）。
     - 检查 `scoop_array_alloc` 是否还有 caller（被 P3-T01 引入的 `scoop_mutable_array_new` 调用，或被 freeze 路径调用）。如有，保留；否则删除。
     - `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST`：删除 9 条 `X(scoop_array_builder_*)` 与（如适用）`X(scoop_array_alloc)` 行。
     - `runtime/c/scoop_array.c` 中保留 `ScoopArray`（inline）+ `scoop_array_trace_elems`（line 127）；删 builder 后这是文件的主要剩余内容。
  5. **回归 fixture**：
     - 跑 P0-T01 baseline。预期：`stdlib_*` 中依赖 `MutableArray.push` 的 fixture（如 `stdlib_collections_*`）会失败（因为 `mutable_array.scoop` 已删）。在完成记录中**列出**这些 fixture，**不**修复——它们的命运在 P9-T02 由三分类清单决定（DELETE 或 KEEP-RENAME）。
     - **不依赖**旧 stdlib `MutableArray.push` 的 fixture 必须仍 pass。
- 必须遵从的约束：
  - 不允许保留任何 `__scoop_array_builder_*` 的声明 / 实现 / dispatch / 别名作为兼容层。
  - `ScoopArrayBuilder` 类型必须从 runtime 完整删除（包括 GC trace 注册）。
  - `String.split` 的行为（包括空 / 单分隔 / 多分隔 / 末尾分隔的输出）必须与重写前完全一致；P0-T01 baseline 中 `String.split` 相关 fixture 的 stdout 不变。
- 验证：
  1. `cargo build` —— 整仓编译通过（除已知会失败的 stdlib-dependent fixture 之外）。
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/` 中 string-related fixture（特别是 `stdlib_string_basic.scoop` / `stdlib_string_methods_extended.scoop`，如这些走 `String.split` 路径）。
  3. `grep -r "scoop_array_builder\|ARRAY_BUILDER_\|__scoop_array_builder" crates/ runtime/ sysroot/`—— 应该完全无命中。
- 完成条件：
  - 仓库内不再有 `__scoop_array_builder_*` 任何引用。
  - 数组字面量、`String.split`、（如重写）vararg spread 全部走 `MutableArray.push + freeze` 路径。
  - stdlib 失效 fixture 清单已记录在完成记录中，等待 P9 处理。
- 依赖：P4-T01。

完成记录（2026-05-17）：

- 改动范围：`sysroot/string.scoop::String.split` 改为 `mutableArrayNew<String>` + `push` + `freeze`；删除 string-specialized builder intrinsic 声明；删除 `stdlib/mutable_array.scoop`；将 `stdlib/array_iter.scoop`、`stdlib/mutable_array_iter.scoop`、`stdlib/collections_iter.scoop`、`stdlib/collections_map.scoop`、`stdlib/collections_set.scoop` 中剩余 builder 用法改为 `MutableArray` 路径；把 `__stdlib_int_zero/one` 迁入 `stdlib/prelude.scoop`；删除 compiler/runtime builder lowering、runtime ABI symbol、runtime C `ScoopArrayBuilder` 实现与 `scoop_array_alloc` 导出；同步 runtime/Rust tests、overlay sysroot fixtures、failure-policy audit baseline。
- 核心决策：`ScoopMutableArray` 成为唯一 growable array buffer；`String.split` 保持 byte-level 扫描行为不变，仅替换收集容器；collections 中实际存在的 builder 调用一并重写，避免留下旧 surface；`scoop_array_alloc` 仅剩 audit/runtime API 引用且无实际 caller，因此删除；plain callable ABI 校验不再用冗余 function `TypeId` 身份判定漂移，改以 root/params/return 为准，避免 materialized generic declaration-only callable 的等价 function type 重建误报。
- 验证结果：`cargo build` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_basic.scoop` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_methods_extended.scoop` 通过；`cargo run -p scoop -- test` 完成，结果为 1 个预期失败、1337 个通过、1374 checks 通过；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`rg "scoop_array_builder|ARRAY_BUILDER_|__scoop_array_builder" crates runtime sysroot` 无命中；`rg "scoop_array_builder|ARRAY_BUILDER_|__scoop_array_builder" stdlib` 无命中。
- 与 `PLAN.md` 闭合：完成 P4 的旧 array builder surface 删除；数组字面量、vararg 合成、`String.split` 和 stdlib 内部收集路径均改走 `MutableArray.push + freeze`。
- 暂时性 failing fixture：`tests/fixtures/run-pass/mutable_array_ops_basic.scoop` 仍失败，原因是它专门覆盖已删除的旧 `MutableArray<Int>.pop/insert/removeAt/splice` copy-style API；该 fixture 的 DELETE 或 KEEP-RENAME 命运按 P9-T02 三分类清单处理。

## P5：`scoop.lang.string` cone 建立

### [DONE] P5-T01：runtime 端——三个 `scoop_string_from_*_array` 单态入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §5.1 / §9 / P5
  - `runtime/c/scoop_runtime.c::scoop_char_to_string`（line 897，UTF-8 变长编码逻辑可复用）
  - `runtime/c/scoop_runtime.c::scoop_string_concat`（line 1042，String allocation pattern 参考）
  - `runtime/c/scoop_runtime.c::scoop_string_unsafe_slice_bytes`（line 1117，String allocation + bytes copy 参考）
  - `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST`
  - `ScoopMutableArray`（P3-T01 引入）的 layout
- 目标：
  - 实现 3 个新 runtime 入口：
    - `const ScoopString *scoop_string_from_byte_array(ScoopMutableArray *bytes)`（unchecked，直接 memcpy）
    - `const ScoopString *scoop_string_from_char_array(ScoopMutableArray *chars)`（runtime 内做 codepoint→UTF-8 编码）
    - `const ScoopString *scoop_string_from_string_array(ScoopMutableArray *parts)`（一次扫描求总长 + memcpy 各 slice）
- 当前实现入口：
  - `runtime/c/scoop_runtime.c`（整文件）
  - `runtime/c/scoop_string_*.c`（如有独立文件；当前看 String 函数都在 `scoop_runtime.c`）
- 必须实现的内容：
  1. `scoop_string_from_byte_array(arr)`：
     - 参数 `arr` 的 `elem_kind` 必须是 WORD、`elem_size_bytes` 必须是 1（Byte = UInt8）。如不满足，trap（参考现有 runtime trap 路径）。
     - 分配新 `ScoopString` 对象，bytes 长度 = `arr->len`；`memcpy(new_str->data, arr->data, arr->len)`。
     - **不**做 UTF-8 有效性检查——这是 unchecked 入口（用户态的 `@Unsafe` 标记由 sysroot 加）。
  2. `scoop_string_from_char_array(arr)`：
     - 参数 `arr` 的 `elem_kind` 必须是 WORD、`elem_size_bytes` 必须是 4（Char = i32 codepoint）。
     - 第一遍扫：对每个 codepoint 求其 UTF-8 编码长度（1/2/3/4 字节，参考 `scoop_char_to_string` line 905-924 的分支条件）。累计得到目标 byte 长度。
     - 分配新 `ScoopString`，目标 byte 长度。
     - 第二遍扫：对每个 codepoint emit UTF-8 字节序列写入目标 buffer（编码逻辑直接复用 `scoop_char_to_string` line 905-924 的 4 个分支；非法 codepoint 降级为 U+FFFD）。
  3. `scoop_string_from_string_array(arr)`：
     - 参数 `arr` 的 `elem_kind` 必须是 REF、元素类型是 `String`。
     - 第一遍扫：累加每个 String 的 byte 长度（`((ScoopString*)arr->data[i])->byte_length`），得到总长。
     - 分配新 `ScoopString`，总 byte 长度。
     - 第二遍扫：`memcpy` 每个 part 的 bytes 到目标 buffer 的累计 offset 处。
  4. 在 `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST` 加 3 条 `X(scoop_string_from_*_array)` 行。
  5. runtime C 单测（如项目有）：
     - `string_from_byte_array_basic`
     - `string_from_char_array_handles_4byte_codepoint`：验证 U+1F600 等 4 字节 UTF-8
     - `string_from_char_array_replaces_surrogate_with_replacement_char`
     - `string_from_string_array_concatenates_correct_total_length`
     - `string_from_*_empty_array_returns_empty_string`
- 必须遵从的约束：
  - 三个入口都是 `Pure` scoop ABI（caller 视角无 unrelated 副作用；alloc 触发 GC 是 caller-visible 的）。
  - byte 入口**不**做 UTF-8 校验—— sysroot side 加 `@Unsafe`；用户传入非法 byte 是 unsafe contract violation。
  - char / string 入口构造的 String 必然是 valid UTF-8（well-formed by construction）。
  - 不得在三个入口内部调用其它 `scoop_string_concat` / `scoop_string_unsafe_slice_bytes`——必须是单次分配 + memcpy 路径，否则 StringBuilder 性能不如直接的 `var sb: String = ""; sb = sb.concat(...)`。
- 验证：
  1. `cargo build`
  2. 如有 runtime C 单测：跑 `string_from_*` 一组
- 完成条件：
  - 3 个 runtime 符号编译通过、可被 sysroot 通过 scoop ABI 调用。
- 依赖：P3-T01。

完成记录（2026-05-17）：

- 改动范围：新增 `runtime/c/scoop_array_internal.h` 共享 `ScoopArray` / `ScoopMutableArray` runtime internal layout；`runtime/c/scoop_runtime.c` 实现 `scoop_string_from_byte_array` / `scoop_string_from_char_array` / `scoop_string_from_string_array`，并复用 shared UTF-8 scalar helper 与 owned-byte String 构造；`runtime/c/scoop_runtime_api.h` 登记 3 个新 runtime ABI 符号；`runtime/c/scoop_array.c` 将 WORD `MutableArray` 存储从固定 word-sized 改为遵守 `elem_size` / `elem_align`；`crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 同步数组 intrinsic stride；`crates/scoopc/src/comptime/interpreter.rs` 补齐 `sizeOf/alignOf<Char/Float32/Float64/Double>`；新增 `crates/scoop_runtime/tests/string_from_array_runtime.rs`。
- 核心决策：byte array 入口要求 `elem_kind == WORD` 且 `elem_size_bytes == 1`，只做 unchecked memcpy，不做 UTF-8 校验；char array 入口要求 4-byte WORD slot，两遍扫描，非法 codepoint / surrogate 统一替换为 U+FFFD；string array 入口要求 REF pointer-sized slot，一遍求总字节数、一遍 memcpy，元素的 String 类型由后续 sysroot typed signature 保证，runtime 侧校验当前 layout 可表达的 REF/pointer shape；三个入口均为单次结果分配路径，不调用 `scoop_string_concat` / `scoop_string_unsafe_slice_bytes`。
- 验证结果：`cargo test -p scoop_runtime --test string_from_array_runtime -- --nocapture` 通过（6 tests）；`cargo test -p scoop_runtime --test mutable_array_runtime -- --nocapture` 通过（4 tests）；`cargo build` 通过；`cargo run -p scoop -- test` 完成，结果为 1 个既有失败、1337 个通过、1374 checks 通过；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 与 `PLAN.md` 闭合：完成 P5 §5.1 的三个 runtime 单态 String-from-array 入口；同时补齐 §6 MutableArray out-of-line layout 对 `sizeOf<T>()` 的真实元素大小支持，使 P5-T02 可以通过 `MutableArray<Byte>` / `MutableArray<Char>` / `MutableArray<String>` 直接调用这些 scoop ABI 符号。
- 暂时性 failing fixture：本任务未新增 failing fixture；`tests/fixtures/run-pass/mutable_array_ops_basic.scoop` 仍为 P4-T02 完成记录中已列出的既有失败，原因是它覆盖已删除的旧 `MutableArray<Int>.pop/insert/removeAt/splice` copy-style API，继续由 P9-T02 三分类清单处理。

### [DONE] P5-T02：sysroot 端——`scoop.lang.string` cone 内三个 scoop ABI 声明 + StringBuilder

- 参考：
  - [`PLAN.md`](./PLAN.md) §5.1 / §5.2 / §9 / P5
  - `sysroot/lang_string.scoop`（P1-T01 创建的 placeholder）
  - P3-T03 引入的 `mutableArrayNew<T>` / `MutableArray<T>.push`
  - P5-T01 引入的 3 个 runtime 符号
- 目标：
  - 在 `sysroot/lang_string.scoop` 暴露 3 个 scoop ABI 声明 + 1 个用户可见 class `StringBuilder`。
  - StringBuilder 内部用 `MutableArray<String>` 收集片段，`toString()` 调 `scoop_string_from_string_array` 一次性合成。
- 当前实现入口：
  - `sysroot/lang_string.scoop`（P1-T01 后是空 cone placeholder）
- 必须实现的内容：
  1. 在 `sysroot/lang_string.scoop` 加 3 个 scoop ABI 声明：
     ```
     @Extern(name = "scoop_string_from_byte_array", abi = "scoop")
     @Unsafe
     fun __scoop_string_from_byte_array(bytes: MutableArray<Byte>): String

     @Extern(name = "scoop_string_from_char_array", abi = "scoop")
     fun __scoop_string_from_char_array(chars: MutableArray<Char>): String

     @Extern(name = "scoop_string_from_string_array", abi = "scoop")
     fun __scoop_string_from_string_array(parts: MutableArray<String>): String
     ```
     注意：byte 版需要 `@Unsafe`——但 scoop ABI 不允许 `@Extern` 显式叠加 `@Unsafe`（参考前一轮 PLAN-managed-abi.md §3.2）。如果该约束仍生效，本任务**不**给 byte 版加 `@Unsafe`，而是把 byte 入口在 sysroot 中定义为 `private` 的 internal helper（比如以 `__scoop_lang_string_unsafe_*` 命名 + 用 file-level `@AllowIntrinsic` / sysroot internal visibility 隐藏），并在 wrapper 函数侧加 `@Unsafe` 体。任务开工时确认当前 surface gate，记入完成记录。
  2. 加 `class StringBuilder`：
     ```
     class StringBuilder {
         private val parts: MutableArray<String> = mutableArrayNew<String>(capacity = 8)

         fun add(s: String): StringBuilder {
             this.parts.push(s)
             return this
         }

         fun toString(): String {
             return __scoop_string_from_string_array(this.parts)
         }
     }
     ```
     注意：当前 `class` 是否允许 `private val` 字段 + 字段默认值是 `mutableArrayNew<String>(capacity = 8)` 表达式（含字面量 `8`）—— P0-T01 baseline 应能确认这两点。如果"non-入口文件中字面量"限制仍在（参考 `stdlib/mutable_array.scoop` 的 `__stdlib_int_one` trick），用 `sizeOf<UIntPtr>()` 派生 8（一次乘法或类似）。任务开工时确认。
  3. owner 测试：
     - `tests/fixtures/run-pass/lang_string_builder_basic.scoop`：
       - 单 add：`StringBuilder().add("hello").toString() == "hello"`
       - 链式：`StringBuilder().add("a").add("b").add("c").toString() == "abc"`
       - 空：`StringBuilder().toString() == ""`
       - 大量 add（20+）触发内部 grow，结果仍正确
       - 含 4-byte UTF-8 codepoint 的字符串拼接仍正确
- 必须遵从的约束：
  - StringBuilder 只暴露 `add` 与 `toString` 两个方法。**不**加 `length` / `clear` / `lastChar` / `[]` / 等其它 method—— 这些是未来扩展，不进本轮。
  - StringBuilder 是 ordinary class（非 `@Intrinsic`），编译器**不**为它加任何 special-case。
  - `__scoop_string_from_string_array` 必须用 P3-T03 的 `MutableArray<String>.push` 收集——不允许直接拿一个 raw buffer 喂给 runtime。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/lang_string_builder_basic.scoop`
  2. `cargo run -p scoop -- test`（全量 baseline，无回退）
- 完成条件：
  - 用户代码可以写 `StringBuilder().add("x").add(y.toString()).toString()`（前提是 y 实现 ToString）。
  - P6-T01 的 f-string desugar 可以直接 emit `StringBuilder` 调用链。
- 依赖：P1-T01、P3-T03、P5-T01。

完成记录（2026-05-17）：

- 改动范围：`sysroot/lang_string.scoop` 从 placeholder 扩展为 `scoop.lang.string` 实现文件，新增 `scoop_string_from_byte_array` / `scoop_string_from_char_array` / `scoop_string_from_string_array` 的 scoop ABI surface 与 `StringBuilder` class；新增 `tests/fixtures/run-pass/lang_string_builder_basic.scoop` 覆盖单 add、链式 add、空 builder、grow、4-byte UTF-8、char-array 与 byte-array 构造路径；更新 sysroot loader、member-call typecheck、HIR scalar alias lowering、frontend monomorph binding 收集与 MIR materializer class-init reachability，支撑 `StringBuilder` 作为普通 sysroot class 编译运行。
- 核心决策：`abi = "scoop"` 的 `@Extern` 仍不允许叠加 `@Unsafe`，因此 byte-array runtime symbol 暴露为 private raw extern `__scoop_lang_string_from_byte_array_unchecked`，公开 wrapper `__scoop_string_from_byte_array` 标记 `@Unsafe`；`StringBuilder` 是 ordinary class，只有 `add` 与 `toString` 两个方法，内部 `parts` 为 private `MutableArray<String>`，`toString()` 一次性调用 string-array runtime 入口；direct receiver member 在 typecheck 中优先于预解析 extension function，保证 `StringBuilder.add` 不被旧 `MutableList.add` 扩展遮蔽。
- 验证结果：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/lang_string_builder_basic.scoop` 通过；`cargo run -p scoop -- test` 完成，结果为 1 个既有失败、1338 个通过、1375 checks 通过，唯一失败为 P4-T02 已记录待 P9-T02 处理的 `tests/fixtures/run-pass/mutable_array_ops_basic.scoop`；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 与 `PLAN.md` 闭合：完成 P5 §5.1/§5.2 的 sysroot string-from-array ABI surface 与 `StringBuilder` cone 落地；P6-T01 可直接生成 `StringBuilder().add(...).toString()` 调用链。
- 暂时性 failing fixture：本任务未新增 failing fixture；`tests/fixtures/run-pass/mutable_array_ops_basic.scoop` 仍为 P4-T02 完成记录中已列出的既有失败，继续由 P9-T02 三分类清单处理。

### P5-T03：sysroot/string.scoop 中高级 String helper 迁入 `scoop.lang.string`，更新 `String.split`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5.3 / §5.4 / §9 / P5
  - `sysroot/string.scoop`（约 line 392-572，扩展函数 `substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim`）
- 目标：
  - 把 `String.substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` 这 9 个扩展函数从 `sysroot/string.scoop` 迁到 `sysroot/lang_string.scoop`。
  - `sysroot/string.scoop` 仅保留 `@Intrinsic class String` body method 所依赖的内部 helper（`__scoop_string_*` 一组，line 25-390）。
  - `String.split` 在 P4-T02 已被重写为基于 `MutableArray<String>.push + freeze`；本任务把它一同搬到 `lang_string`。
- 当前实现入口：
  - `sysroot/string.scoop` line 392-572：9 个 `fun String.<name>` 扩展函数
  - `sysroot/string.scoop` line 25-390：`__scoop_string_*` 内部 helper（迁出后由 core String body method 继续调用，**留**在 `sysroot/string.scoop`）
- 必须实现的内容：
  1. 把 9 个扩展函数从 `sysroot/string.scoop` 剪切到 `sysroot/lang_string.scoop`。`package` 改成 `scoop.lang.string`，`import scoop.core.*` 保持。
  2. 检查每个函数的依赖：
     - `substring` 用 `byteLength` / `unsafeSliceBytes`（core intrinsic / scoop ABI helper），可见性正常。
     - `split`（P4-T02 重写后）用 `mutableArrayNew<String>` + `MutableArray<String>.push` + `freeze`（在 core），可见性正常。
     - `indexOf/contains/startsWith/endsWith/trim*` 都仅用 `byteLength/getByte/unsafeSliceBytes` —— 可见性正常。
     - `__scoop_string_matches_at` / `__scoop_string_is_indent_whitespace` 等内部 helper：当前在 `sysroot/string.scoop`，被多个高级 helper 共用。本任务期间这些保持在 `sysroot/string.scoop`，由 lang_string 通过 `import scoop.core.*` 看到（前提是它们暴露在 `scoop.core` package 下；当前确实如此）。
  3. `sysroot/string.scoop` 删除被迁出的 9 个函数；保留剩下的 `__scoop_string_*` 内部 helper 与 `__scoop_runtime_string_concat_bridge` 等 audited bridge（后者在 P7-T02 删）。
  4. owner 测试：
     - 把 P0-T01 baseline 中所有 `String.substring/indexOf/contains/startsWith/endsWith/split/trim*` 的 fixture 单跑一遍，stdout 与 baseline 完全一致。
     - 加一条 visibility 测试：在用户文件不显式 `import scoop.lang.string.*` 的情况下（依靠自动 prelude）能直接调 `"abc".substring(0, 1)`。
- 必须遵从的约束：
  - 函数签名 / 行为 / 边界值处理完全保持现状。
  - `sysroot/string.scoop` 的 `__scoop_string_*` 内部 helper 名字与 visibility 不变（core String body method 还在调用它们）。
- 验证：
  1. `cargo run -p scoop -- test`（全量 baseline）—— String 高级 helper 相关 fixture 应全部 pass。
  2. `grep -n "fun String\." sysroot/string.scoop`—— 应该完全无命中（9 个扩展函数全迁出）。
- 完成条件：
  - String 高级 helper 物理上位于 `scoop.lang.string`，但用户调用形态不变（自动 prelude 让它们可见）。
- 依赖：P1-T02、P4-T02、P5-T02。
