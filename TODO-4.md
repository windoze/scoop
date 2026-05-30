# TODO-4：P5-P6 通用常量化折叠器与 Platform 收敛

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P5-P6
> 包目标：用类型特征驱动的 `is_immutable` 谓词 + `try_emit_immortal` 折叠器替换三个手写 immortal 路径，让 String literal 零分配并对 String 开内容池；Platform 作为消费者自动落入，删除一切专用 codegen。

## P5：通用谓词、折叠器与 String immortal

### [DONE] P5-T01：实现 `is_immutable(T)` 谓词

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The constantization predicate `is_immutable(T)`”
- 目标：
  - 实现一个结构、递归、可 memo 的不可变性谓词，作为常量化的通用门。
- 必须修改的文件/位置：
  - codegen 侧谓词落点（新增，靠近 `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/`）
  - 类型/字段元数据查询：`crates/scoopc_hir/src/hir/mod.rs:205`（`FieldDecl.mutable`）、`TypeStore`
- 必须实现的内容：
  1. `is_immutable(T)`：
     - 带 `@InteriorMutable` → false；
     - 值标量（Int/Bool/Float64/32/Char/Unit）→ true；
     - 值 struct / tuple → 所有字段类型 `is_immutable`（struct 字段必 `val`，无需查可变性）；
     - ref class → 所有字段 `val` 且 所有字段类型 `is_immutable`。
  2. 递归进字段类型并 memo，处理循环引用（自引用类型不可变性需有终止策略）。
- 必须遵从的约束：
  - 决策由类型特征驱动，不得名字匹配或类型白名单。
  - `var` 字段检查只在 ref class 分支有意义；`@InteriorMutable` 是值类型层唯一守卫。
- 验证：
  1. 单元：String 与合成全-val class 为 true；`RefCell` / `AtomicInt`（`var`）与 `__AtomicInt`（`@InteriorMutable`）为 false；含 `var x: RefCell` 的类型为 false。
  2. `cargo test --all --all-targets`
- 完成条件：
  - 谓词正确区分可常量化与不可常量化类型。
- 依赖：P3-T02R、P4-T02R
- 完成记录：
  - 2026-05-30：新增 `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/immutability.rs`，实现结构化、递归、带 memo 的 `TypeImmutability::is_immutable(T)`。
  - 谓词按类型特征判定：`@InteriorMutable` nominal 直接 false；标量、`String`、tuple、`Option`、value struct 递归字段；ref class 要求全部字段为 `val` 且字段类型递归不可变；未知/接口/函数/union/缺元数据保守 false。
  - 通过 `CompilationUnitCodegenCx` 接入 `InteriorMutableIndex`，后续折叠器可直接消费该谓词，不需要名字匹配或类型白名单。
  - 单元覆盖：`String`、tuple、合成 all-val struct/class 为 true；`RefCell`/`AtomicInt` var 字段、`__AtomicInt` / marked class `@InteriorMutable`、含 `RefCell` 字段的 class 为 false；自引用 all-val class 可终止且为 true。
  - 验证：`cargo fmt`；`cargo test -p scoopc_codegen_llvm immutability --all-targets`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P5-T01R：Review `is_immutable` 谓词

- 参考：
  - P5-T01 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The constantization predicate”
- 目标：
  - 复核谓词的递归正确性、循环终止、`@InteriorMutable` 与 `var` 分工。
- 必须检查的文件/位置：
  - P5-T01 谓词实现与单元测试
- 必须实现的内容：
  1. 确认值 struct 分支不依赖 `var` 检查（struct 必 val），ref class 分支正确处理 `var`。
  2. 确认自引用/循环类型不会无限递归。
  3. 确认 `@InteriorMutable` 在值类型层确实是唯一否决路径。
- 必须遵从的约束：
  - 若谓词退回名字匹配或漏循环终止，必须修正后才进入 P5-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - 谓词可靠。
- 依赖：P5-T01
- 完成记录：
  - 2026-05-30：已完成。
  - Review 结论：P5-T01 谓词保持结构化类型特征判定，未退回 `__AtomicInt` 等名字匹配；value struct 分支只递归字段类型、不检查 `var`；ref class 分支同时要求字段非 `var` 且字段类型递归不可变；`@InteriorMutable` 按 nominal metadata 查询并优先否决。
  - 修正：发现并修复递归 memo 的 stale optimistic true 问题。`Visiting -> true` 现在会携带 optimistic 标志，只用于打断当前递归边；依赖 optimistic cycle 的 positive 结果不会写入永久 `Done(true)` 缓存，避免互递归类型中祖先后续判 false 时留下过期 true。
  - 测试：新增 `recursive_class_cycle_with_mutable_member_does_not_cache_optimistic_true`，覆盖 `A -> B -> A` 且 `A` 含 `var` 字段时同一 analyzer 先查 `A` 再查 `B` 仍均为 false；保留 all-val 自引用 class 可终止且为 true 的覆盖。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_codegen_llvm immutability --all-targets`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P5-T02：实现 `try_emit_immortal` 折叠器并路由 String literal

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The generic immortal folder”“Emission shapes”、Phasing 4
- 目标：
  - 用一个 content-hash 缓存的递归折叠器替换 String literal 的 per-use 分配，并为聚合提供提升门。
- 必须修改的文件/位置：
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`（`codegen_string_literal_from_bytes`）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs`（`codegen_mir_const` 的 String/SynthString 分支）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs`（`codegen_mir_rvalue` 的 `StructLit`/`MakeTuple` arm）
  - `crates/scoopc_mir/src/mir/transport.rs`（`AggregateTransportMetadata`/`MirBoxingIntent` 查询）
- 必须实现的内容：
  1. `try_emit_immortal(value) -> Option<GlobalValue>`（content-hash 缓存）：
     - 标量 `ConstValue::*` → LLVM 标量常量；
     - `ConstValue::String/SynthString` 与 `TypeMetadataLiteral::TypeNameString` → 带 header + `SCOOP_GC_FLAG_IMMORTAL` + `SCOOP_GC_MARK_IMMORTAL`、`next=null` 的 immortal `ScoopString` 全局（`set_constant`+`set_unnamed_addr`）；
     - `Rvalue::StructLit/MakeTuple` 过提升门则发射常量聚合全局（值类型层无 header；ref 类型层带 header），否则 `None`。
  2. 提升门：① 字段全 `Operand::Const`；② 每字段 `transport.boxing.is_none()`；③ `transport.kind` 为 `Tuple`/`Struct`；④ ref 类型聚合再加 `is_immutable(aggregate_ty)`。
  3. `codegen_string_literal_from_bytes` 与 `const_pat.rs` String/SynthString 分支改走折叠器；`terminator.rs` 的 `StructLit`/`MakeTuple` arm 先试折叠器再回退现有动态路径。
- 必须遵从的约束：
  - 遇到非平凡 transport（boxing/value-erasure）或非 `Const` 字段必须安全回退 `None`。
  - 本版不追 `Local`→定义它的 StructLit（嵌套聚合回退）；不含 `EnumVariant`。
  - 决策由类型特征驱动，不逐类型特判。
- 验证：
  1. 单元：`codegen_string_literal_from_bytes("hello")` 零 `scoop_alloc_typed`；含 boxing 的聚合回退动态路径。
  2. 集成（P0-T03 度量）：literal 在 10M 循环里首个 cycle 后 `bytes_allocated` 零增长（除 print 自身）。
  3. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - String literal / `__type_name` 零堆分配，聚合按门提升或安全回退。
- 依赖：P5-T01R
- 完成记录：
  - 2026-05-30：新增 `crates/scoopc_codegen_llvm/src/llvm/codegen/main/immortal.rs`，实现 immortal String wrapper 全局发射与 `StructLit` / `MakeTuple` 的安全提升入口。
  - String / SynthString / `TypeMetadataLiteral::TypeNameString` 现在通过 content-hash 命名的 `@__scoop_str_lit_<hash>` 全局返回 `ScoopString addrspace(1)*`，header 设置 `next=null`、`SCOOP_GC_FLAG_IMMORTAL`、`SCOOP_GC_MARK_IMMORTAL`、runtime String type descriptor、对象大小，wrapper 与 byte data 均为 `constant` + `unnamed_addr`。
  - 聚合提升门已接入 `Rvalue::StructLit` / `MakeTuple`：仅当字段全 `Operand::Const`、transport kind 为 Tuple/Struct、字段 transport 无 boxing 时尝试提升；值类型聚合发射 constant global 并按值 load；不支持或不安全的形状安全回退现有动态路径。
  - 发现并修复当前任务触发的 verifier 边界：`SCOOP_GC_VERIFY_ROOTS=1` 现在接受带 immortal header 的 off-heap root，避免全局/栈 root 中的 immortal String 被误报为非 heap root。
  - 测试/fixture：新增 P5-T02 IR fixtures 覆盖 String/TypeNameString 零 `scoop_alloc_typed` 与 aggregate boxing 回退；新增 runtime GC fixture 覆盖 10M 次 String literal evaluation 不增长 `bytes_allocated`；更新 P0 metric、hard-cap OOM、struct String root 相关 fixture 以匹配 String literal immortal 行为且保持原测试目标。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P5-T02R：Review 折叠器与 String immortal

- 参考：
  - P5-T02 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The generic immortal folder”
- 目标：
  - 复核提升门完整性、回退安全性、immortal header 字段正确性。
- 必须检查的文件/位置：
  - P5-T02 折叠器与各 arm 改动
- 必须实现的内容：
  1. 确认 boxing/非 Const/EnumVariant/嵌套聚合都安全回退。
  2. 确认 immortal `ScoopString` 的 header 字段（`next=null`、flag、mark sentinel、type_desc、size）正确。
  3. 确认 String 零分配在 IR 与运行期度量上都成立。
- 必须遵从的约束：
  - 若提升门有不安全提升（漏 boxing 判断等），必须修正后才进入 P5-T03。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - 折叠器正确、String immortal 安全。
- 依赖：P5-T02
- 完成记录：
  - 2026-05-30：已完成。
  - Review 结论：P5-T02 折叠器入口保持安全提升门；`StructLit` / `MakeTuple` 仅在字段全 `Operand::Const`、aggregate transport kind 为 Tuple/Struct、字段 transport 无 boxing 时尝试提升；非 Const、嵌套聚合（经 Local）、EnumVariant、boxing/value-erasure 均回退动态路径或既有 enum 路径。
  - Header 复核：immortal `ScoopString` 发射为 `addrspace(1) constant` 全局，header 为 `next=null`、`type_desc=@__scoop_type_desc_runtime__ScoopString`、目标 store size、`SCOOP_GC_FLAG_IMMORTAL`、`SCOOP_GC_MARK_IMMORTAL`；byte data 与 wrapper 均保持 content-keyed `unnamed_addr` 常量。
  - 测试/fixture：补强 `pos_string_literal_immortal_ir.scoop`，锁定 addrspace(1) constant 与 header 字段形状；新增 `pos_aggregate_fallback_shapes_ir.scoop`，覆盖非 Const 字段、嵌套聚合和 EnumVariant 不生成 `@__scoop_immortal_agg_`。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P5-T03：String 内容池 dedup 与其它 ref 类型 per-site

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Deduplication (String only, for now)”
- 目标：
  - 把 dedup 这个身份敏感行为限定到 String（内容池），其它可常量化 ref 类型 per-site 一份。
- 必须修改的文件/位置：
  - P5-T02 折叠器的 wrapper 全局缓存键逻辑
- 必须实现的内容：
  1. String：wrapper 与 byte 数组都按 content-hash 键（`__scoop_str_lit_<hash>`），跨 site 合并成一份。
  2. 其它可常量化 ref 类型：每个 literal site 发一份全局，不跨站合并，保持 per-site 身份。
  3. 在完成记录说明 dedup 边界与理由（Scoop 无身份运算符 ⇒ String 值语义 ⇒ 合并不可观测）。
- 必须遵从的约束：
  - dedup 仅对 String；不得对其它类型跨站合并（除非未来评估身份哈希通道后另开任务）。
- 验证：
  1. 单元：同函数两个 `"hello"` 引用同一全局；两次 `__type_name(T)` 指针相等。
  2. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - String 内容池生效，其它 ref 类型 per-site。
- 依赖：P5-T02R
- 完成记录：
  - 2026-05-30：已完成。
  - String wrapper 命名继续由 byte data content key 派生，`__scoop_str_data_<hash>` 与 `__scoop_str_lit_<hash>` 按字节内容跨 site 合并，并保持 `unnamed_addr`。
  - 折叠器的 aggregate 全局命名改为显式 key mode：值类型聚合仍按内容 key 复用；ref 类型聚合会把当前 codegen body + literal span 纳入 key，并且不设置 `unnamed_addr`，避免非 String ref 类型跨站点合并。
  - 新增单元测试覆盖 String wrapper 命名、值聚合 content mode 复用、ref 聚合 site mode 区分 literal site。
  - 新增 `tests/fixtures/umb_fix/P5-T03-dedup/pos_string_content_pool_ir.scoop` 与 `pos_type_name_content_pool_ir.scoop`，锁定同函数重复 `"hello"` 与重复 `Point::class` 均引用同一个 content-keyed String wrapper，且零 `scoop_alloc_typed`。
  - Dedup 边界：当前仅 String 走内容池；这是安全的，因为 String 不可变且 Scoop 无身份运算符，值语义下合并不可观测。其它 ref 类型即使将来进入 immortal 折叠，也保持 per-site 身份，不跨站合并。
  - 验证：`cargo fmt`；`cargo test -p scoopc_codegen_llvm immortal --all-targets`；`python3 tools/run_fixtures.py tests/fixtures/umb_fix/P5-T03-dedup --exit-on-failure`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P5-T03R：Review dedup 策略

- 参考：
  - P5-T03 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Deduplication”
- 目标：
  - 复核 dedup 仅限 String，其它类型未被误合并。
- 必须检查的文件/位置：
  - P5-T03 缓存键逻辑
- 必须实现的内容：
  1. 确认非 String 可常量化 ref 类型确实 per-site，不跨站合并。
  2. 确认 String 跨站合并、指针相等。
- 必须遵从的约束：
  - 若非 String 类型被误合并，必须修正后才进入 P6-T01。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - dedup 边界正确。
- 依赖：P5-T03
- 完成记录：
  - 2026-05-30：已完成。
  - Review 结论：P5-T03 的 dedup 边界正确；String wrapper 继续由 byte data content key 派生，重复 String literal 与 `TypeMetadataLiteral::TypeNameString` 均复用同一个 content-keyed `ScoopString` 全局。
  - 非 String ref 边界复核：aggregate key mode 已显式区分 `Content` 与 `Site`；ref aggregate 使用当前 codegen body + literal span 参与 key，且 site mode 不设置 `unnamed_addr`，不会被折叠器按内容跨站合并。
  - 测试复核：`immortal` 单元覆盖 String content key、value aggregate content mode 复用、ref aggregate site mode 区分 literal site；P5-T03 dedup fixtures 覆盖重复 `"hello"` 与重复 `Point::class` 引用同一 content-keyed wrapper 且零 `scoop_alloc_typed`。
  - 验证：`cargo test -p scoopc_codegen_llvm immortal --all-targets`；`python3 tools/run_fixtures.py tests/fixtures/umb_fix/P5-T03-dedup --exit-on-failure`；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；`python3 tools/spec_fixtures.py check`。

## P6：Platform 折叠与 TypeMetadataLiteral 审计

### [DONE] P6-T01：Platform lower 成 StructLit 并删除专用 codegen

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Consumers (recast) — Platform”、Phasing 5
- 目标：
  - 让 `Platform` 作为通用折叠器的消费者自动落入，删除一切 Platform 专用 codegen。
- 必须修改的文件/位置：
  - HIR→MIR `TypedIntrinsicKind::Platform` lowering 落点
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:203-292`（`codegen_platform_literal`）
  - `sysroot/lib/scoop.core/src/core.scoop`（`Platform` struct，参考）
- 必须实现的内容：
  1. 在 HIR→MIR 把 `TypedIntrinsicKind::Platform` lower 成 `Rvalue::StructLit`，5 字段为 `Operand::Const(ConstValue::SynthString(...))`，transport kind=`Struct`、各字段 `boxing: None`。
  2. 删除 `codegen_platform_literal` 及任何 `get_or_create_immortal_platform_global` 专用 helper。
  3. 确认 Platform（值类型 struct）走折叠器值类型层（无 header），5 字段引用 ref 层 immortal `ScoopString`。
- 必须遵从的约束：
  - 不得保留 Platform 专用常量化路径；它必须与普通值 struct 同路。
  - Platform 聚合本身无 GC header。
- 验证：
  1. 单元：`Platform` 访问零 `scoop_alloc_typed`。
  2. 集成：`Platform.os` 读 10M 次不触发 GC。
  3. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - Platform 由通用机制处理，无专用代码。
- 依赖：P5-T03R
- 完成记录：
  - 2026-05-30：已完成。
  - MIR lowering：`TypedIntrinsicKind::Platform` / `getPlatform()` 现在直接生成 `Rvalue::StructLit`，字段为 `triple` / `arch` / `vendor` / `os` / `env` 五个 `Operand::Const(ConstValue::SynthString(...))`，aggregate transport kind 为 `Struct`，各字段 transport 保持 `boxing: None`。
  - Codegen 收敛：删除 LLVM 侧 `codegen_platform_literal` 专用 helper、effect-lowered `getPlatform` 分支和只服务该 helper 的 target-triple 拆分逻辑；Platform 不再有专用常量化 codegen。
  - Folding 结果：Platform 作为普通值类型 struct 进入通用 `try_emit_immortal_struct`，Platform 聚合全局是 `%scoop.core.Platform` constant，无 GC header；五个字段分别指向 immortal `ScoopString` wrapper。
  - 测试/fixture：新增 MIR stage 单元 `mir_platform_intrinsic_lowers_to_structlit` 覆盖 Platform StructLit lowering，新增 LLVM stage 单元 `platform_literal_stage_ir_uses_immortal_structlit_without_alloc`；更新 `call_contracts` MIR golden 以保持平台无关；新增 `tests/fixtures/umb_fix/P6-T01-platform/pos_platform_structlit_immortal_ir.scoop` 锁定 `@__scoop_immortal_agg_`、Platform value global、字段 immortal String 和零 `scoop_alloc_typed`；新增 `tests/fixtures/runtime_gc/platform_immortal_no_alloc_loop.scoop` 覆盖 10M 次 `Platform.os` 读取不增长 heap bytes。
  - 验证：`cargo fmt`；`cargo test -p scoopc mir_call_contract_lowers_typed_call_sites --all-targets`；`cargo test -p scoopc mir_platform_intrinsic_lowers_to_structlit --all-targets`；`cargo test -p scoopc platform_literal_stage_ir_uses_immortal_structlit_without_alloc --all-targets`；`python3 tools/run_fixtures.py tests/fixtures/mir_lowered/call_contracts.scoop --exit-on-failure`；`python3 tools/run_fixtures.py tests/fixtures/umb_fix/P6-T01-platform --exit-on-failure`；`python3 tools/run_fixtures.py tests/fixtures/runtime_gc/platform_immortal_no_alloc_loop.scoop --exit-on-failure`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [DONE] P6-T01R：Review Platform 折叠

- 参考：
  - P6-T01 完成记录
- 目标：
  - 复核 Platform 专用 codegen 已删尽、走值类型层、字段引用 immortal String。
- 必须检查的文件/位置：
  - P6-T01 的 lowering 与删除改动
- 必须实现的内容：
  1. 反向 grep 确认无 Platform 专用常量化残留。
  2. 确认 Platform 聚合无 GC header、字段指向 ref 层 immortal。
  3. 确认零 `scoop_alloc_typed`。
- 必须遵从的约束：
  - 若残留专用路径或聚合误带 header，必须修正后才进入 P6-T02。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - Platform 收敛正确。
- 依赖：P6-T01
- 完成记录：
  - 2026-05-30：已完成。
  - Review 结论：P6-T01 Platform 收敛正确；`TypedIntrinsicKind::Platform` / `scoop.core.getPlatform` 在 MIR lowering 阶段直接生成普通 `Rvalue::StructLit`，五个字段为 `Operand::Const(ConstValue::SynthString(...))`，aggregate transport kind 为 `Struct`，字段 transport 均无 boxing。
  - 反向 grep 结论：源码中已无 `codegen_platform_literal`、`get_or_create_immortal_platform` 或 `immortal_platform` 专用 LLVM 常量化路径；`getPlatform` 的源码残留限于 MIR intrinsic lowering、前端/测试引用和历史文档记录。
  - IR 复核：Platform 聚合通过通用 `try_emit_immortal_struct` 产生 `%scoop.core.Platform` value constant global，不带 GC header；五个字段指向 ref 层 immortal `ScoopString` wrapper（`@__scoop_str_lit_...`），fixture 锁定零 `scoop_alloc_typed`。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。

### [TODO] P6-T02：`TypeMetadataLiteral` 审计与指针相等断言

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Consumers (recast) — TypeMetadataLiteral”、Phasing 6
- 目标：
  - 审计 `TypeMetadataLiteral` 消费者不 mutate，并断言 dedup 后两次 `__type_name(T)` 指针相等。
- 必须修改的文件/位置：
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201`
  - MIR `TypeMetadataLiteral` 消费者
- 必须实现的内容：
  1. 审计所有 `TypeMetadataLiteral` 消费者，确认无人 mutate 其结果。
  2. 新增断言/测试：两次 `__type_name(T)` 读返回指针相等的 `ScoopString`。
- 必须遵从的约束：
  - 若发现消费者 mutate，必须先修正其语义或退出该消费者的 immortal 路径。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - TypeMetadataLiteral 不可变性确认，指针相等成立。
- 依赖：P6-T01R
- 完成记录：
  - （待执行）

### [TODO] P6-T02R：Review TypeMetadata 审计

- 参考：
  - P6-T02 完成记录
- 目标：
  - 复核审计覆盖面与指针相等断言。
- 必须检查的文件/位置：
  - P6-T02 审计范围与新增断言
- 必须实现的内容：
  1. 确认审计覆盖所有消费者。
  2. 确认指针相等断言真实成立。
- 必须遵从的约束：
  - 若审计有遗漏，必须补全后才进入 TODO-5。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - immortal codegen 线收口（P5-P6 完成）。
- 依赖：P6-T02
- 完成记录：
  - （待执行）
