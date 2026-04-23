# 本轮执行计划

## 约束说明
- 按要求先写入本文件，再执行任何进一步的仓库检查、构建、测试或代码修改。
- 本文件记录可审计的执行计划、决策依据摘要、进度更新与计划变更。
- 不记录原始逐字思维过程；改为记录足够详细的行动计划、判断结果与变更原因。

## 初始计划
1. 检查最新一次 Git 提交信息，确认是否明确提到已有问题、回归、临时修复或待补问题。
2. 如果最新提交提到需要先修复的既有问题，先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，核对该任务的上下文、依赖、分解情况与当前项目阶段。
5. 评估第一个未完成任务的规模与边界：
   - 若任务足够明确且可在本轮完整完成，直接实施。
   - 若任务过大或实现边界不清，先将其拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
6. 在实施过程中，如果发现任何既有缺陷、规格不匹配、实现边界缺失、测试回归或依赖缺口：
   - 优先把该问题当作当前工作范围内的问题处理；
   - 若它阻塞当前任务且无法在本轮直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前，更新 `PLAN.md`，提交后停止。
7. 实现当前目标任务，确保修改符合规格且不引入临时绕过方案。
8. 运行相关验证，至少覆盖：
   - 受影响模块的最小相关测试；
   - 如有必要，运行更广泛的测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 若任务影响整体行为，补充运行 `cargo test --all` 或相关 fixture 测试。
9. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成的唯一任务标记为完成；
   - 在 `PLAN.md` 中更新当前状态、完成情况和后续剩余工作；
   - 在本文件中记录关键进展与最终结果。
10. 检查工作区变更，避免误改无关文件；如发现用户已有改动，不回退，只在必要范围内协同处理。
11. 使用清晰提交信息提交本轮变更。
12. 停止，不继续执行下一个任务。

## 进度记录
- 已完成：创建计划文件并写入初始执行计划。
- 已完成：检查最新提交 `90c09645 [T4016T4] Sync single-driver task contract docs`，提交说明未额外声明需要先修复的既有 bug。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，确认当前首个未完成任务为 `T4016T5`。
- 当前任务判断：
  - `T4016T5` 的目标是补齐 internal atomic intrinsic 对“对象字段 lvalue”的编译器主线，使 future `Task` 的 claim bit 可以直接作为普通对象字段承载。
  - 目前尚未确认该任务是否仍需继续拆分；下一步先检查现有 atomic intrinsic、member lvalue lowering、sysroot/task 现状与相关 fixtures。
- 已完成：临时探针复现当前 blocker。
  - `class Counter(var claim: __AtomicInt)` + `__atomicIntLoad(c.claim)` 可通过前端与 typecheck，但在 LLVM codegen 失败：`unsupported_main_body: atomicInt target must be an lvalue`。
  - `codegen_atomic_int_lvalue_ptr(...)` 当前只支持 `VarRef::Local` 与 `VarRef::TopLevel`，尚不支持 `MemberAccess` 路径。
  - `struct Counter(var claim: __AtomicInt)` 仍会在 typecheck 处因值类型字段禁止 `var` 而报错；因此当前 claim-bit blocker 的最直接落点是 class/object field addressability，而不是单独的 struct mutability 设计。
- 已完成：第一轮 codegen 修复后，`class` 字段原子探针已可 build/run（输出 `0`、`1`）。
- 新发现的同类基础缺口：
  - `class Holder(var box: ClaimBox)` + `struct ClaimBox(val claim: __AtomicInt)` 下，`__atomicIntLoad(h.box.claim)` 继续暴露 `cg_ty_of_layout_field: missing TypeId ... ty_fqn=Some(\"scoop.unsafe.__AtomicInt\")`，最终报 `unsupported_main_body: struct field type`。
  - 这说明 `struct` layout side table 对 `__AtomicInt` 这类 typealias/内建别名字段仍未稳定恢复 `TypeId`；属于 `T4016T5` 明确要求在 object-field atomic 路径上先补齐的更基础 codegen 缺口。
- 实施方案：
  1. 在 LLVM codegen 中抽出“可寻址 place”辅助逻辑，使 atomic intrinsic 可递归获取 member access 对应的真实槽位地址，而不是先 load 成 rvalue。
  2. 已完成 class-field 路径；下一步补齐 struct layout / field type 恢复，使 nested field place 也能进入主线。
  3. 新增最小 run-pass 与 build LLVM 回归，至少锁定 class-field atomic load/store/CAS；若 struct nested path 修复稳定，再补一个能覆盖递归求址的回归。
  4. 复验格式化、相关 fixtures、全量测试与 clippy，然后更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 已完成：补齐 struct layout type 恢复并扩展回归。

## 本轮完成结果
- 已实现：
  - `crates/scoopc/src/llvm/codegen/mod.rs`：新增 `AddressablePlace` 与递归 place 求址逻辑，`__atomicInt*` 现在可直接作用于 ordinary class field、nested class field，以及由 addressable class field 派生出的 nested struct field。
  - `crates/scoopc/src/hir/lower/util.rs`：为 `scoop.unsafe.__AtomicInt` / `scoop.core.UIntPtr` 补齐 layout alias 到 builtin `TypeId` 的恢复，修复 nested struct field path 的基础缺口。
  - `crates/scoopc/src/llvm/codegen/ty.rs`：补齐 `__AtomicInt` 的 fallback lowering 与 GC-free 判定。
  - 新增回归：
    - `tests/fixtures/run-pass/unsafe_atomic_int_field_lvalue_basic.scoop`
    - `tests/fixtures/run-pass/unsafe_atomic_int_field_lvalue_basic.stdout`
    - `tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop`
- 已验证：
  - `cargo fmt`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build` → `fixtures: ok (16)`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` → `fixtures: ok (389)`
  - `cargo run -p scoop -- test` → `fixtures: ok (1162)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已同步任务状态：
  - `TODO.md` 已将 `T4016T5` 标记为完成，并把当前剩余顺序推进到 `T4016T6 -> T4016T7 -> T4016T8 -> T4016T9 -> T4016T4R`。
  - `PLAN.md` 已记录本轮实际修复点、回归与新的当前状态。
- 待完成：
  - 生成本轮 Git commit，然后停止。
