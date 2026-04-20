# 执行计划记录

## 说明

根据当前回合要求，我会在这个文件中持续记录：

- 当前目标
- 已确认的约束
- 执行步骤
- 关键决策
- 进度更新

出于安全约束，这里不记录逐字内部思维链条，只记录可审计的计划、依据和结论。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，然后停止。

在开始实现任务前，先执行以下前置检查：

1. 检查最新一次 Git 提交信息，确认是否提到已知问题或待修复事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的那个任务。
5. 运行相关测试，并补齐必要测试。
6. 更新 `TODO.md` / `PLAN.md` / 本文件。
7. 提交 Git commit。
8. 停止，不继续下一个任务。

## 已知约束

- 必须优先处理“最新提交中提到的既有问题”。
- 不能以规避、兼容层、仅夹具修补等方式宣称完成。
- 如遇规格缺口或实现边界，必须把真实问题前置写入 `TODO.md`，调整依赖顺序，并停止。
- 需要保证实现质量，至少关注：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 必要时补充更小粒度测试
- 修改代码前后应更新本文件，记录关键进展。

## 初始执行步骤

### 步骤 A：检查仓库现状

- 查看最新一次提交信息与提交内容摘要。
- 查看 `TODO.md`、`PLAN.md` 当前状态。
- 查看工作区是否已有未提交修改，避免误覆盖用户改动。

### 步骤 B：确定当前任务

- 识别 `TODO.md` 的第一个未完成条目。
- 判断任务是否足够具体、是否存在前置依赖未满足。
- 若需要拆分：
  - 在 `PLAN.md` 写入拆分后的执行方案。
  - 在 `TODO.md` 中把原任务替换/补充为子任务，并把当前回合只执行第一个子任务。

### 步骤 C：实现与验证

- 先阅读相关代码与测试。
- 仅在理解现有实现后修改。
- 修改完成后运行最小相关测试，再运行更完整测试。
- 若发现前置缺陷，停止“硬做”，改为把缺陷前置成任务并更新计划。

### 步骤 D：收尾

- 更新 `TODO.md`：将本回合任务标记完成。
- 更新 `PLAN.md`：反映已完成项和后续状态。
- 更新本文件：记录完成情况、测试结果、是否存在剩余风险。
- 提交 Git commit，提交信息清晰描述本回合变更。

## 进度更新

- 已完成：创建本计划文件并写入初始执行方案。
- 已完成：检查最新提交；最新提交仅包含标题 `[T4010a2b] Define enum with copy-update semantics`，未在提交说明中额外记录需优先修复的遗留 issue。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前可执行的首个未完成子任务为 `T4010b`，即“补齐值类型字段默认值与 immutable-friendly 声明人体工学”。
- 当前判断：
  - `T4010` 已经拆分完成，本回合不需要继续拆总任务；当前应直接执行 `T4010b`。
  - 现有实现中，`crates/scoopc/src/typecheck/structs.rs` 仍明确拒绝 struct 字段默认值。
  - `crates/scoopc/src/typecheck/expr/infer.rs` 仍要求 struct literal 显式提供全部字段。
  - 现有 class ctor 默认参数主线已经具备 typecheck / lowering / LLVM 求值补齐逻辑，可作为参考。
- 当前执行策略（细化版）：
  1. 先确认 `T4010b` 的真实落点：默认值应覆盖哪些值类型声明入口（至少 struct 主构造字段 / type-body 字段；并确认 struct literal 与 ctor call 是否都应支持缺省补齐）。
  2. 做最小 probe，验证 struct ctor call 当前是否可执行；若这条等价构造主线本身不通，则按 blocker 流程先更新 `TODO.md` / `PLAN.md`，不能直接绕过。
  3. 若无前置 blocker，则实现默认字段：
     - 放开声明检查；
     - 在 typecheck 中允许 struct literal 缺失字段由默认值补齐；
     - 在 lowering / codegen 中把默认字段补齐到统一 struct literal / ctor 主线；
     - 补充 typecheck / run-pass / 必要单测，并同步规范文档。
  4. 完成后更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 已完成：最小 probe `struct Point(val x: Int, val y: Int); fun main(): Int { val p: Point = Point(1, 2); return p.x + p.y }` 已验证失败；`cargo run -q -p scoop -- build memory/t4010b_struct_ctor_probe.scoop -o /tmp/t4010b_struct_ctor_probe.out` 在 typecheck 阶段报 `scoop::typecheck::callee_not_callable: Point`。
- 结论：
  - 这是 `T4010b` 的真实前置 blocker，而不是默认值实现细节。
  - 若继续只给 struct literal 补默认字段，会违反 spec §2.3.1 对 `Point { ... }` 与 `Point(...)` 等价的要求。
  - 因此当前回合不应直接实现 `T4010b`，而应把“收口 struct ctor call 与 struct literal 的统一构造语义”前置为独立任务。
- 已完成：`TODO.md` 已新增 `T4010b0 -> T4010b0R`，并把 `T4010b` 调整为依赖 `T4010b0R`。
- 已完成：`PLAN.md` 已同步记录该 blocker、调整 P7 顺序，并将下一项更新为 `T4010b0`。
- 已完成：临时 probe 文件已清理，不会把临时探针带入提交。
- 下一步：提交本轮“发现 blocker 并重排任务”的变更，然后停止。
