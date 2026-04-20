# 执行计划与决策日志

## 说明

按用户要求，本文件用于记录本轮执行的计划、关键决策、进展与必要的调整。

出于协作和可审阅性的考虑，这里记录的是可执行计划、检查项、结论与变更理由，而不是不可复用的原始思维草稿。

## 初始目标

本轮只完成 `TODO.md` 中**第一个未完成任务**，完成后停止。

在处理任务前，先检查最新提交是否提到任何已知问题；若有，需先修复这些问题，再进入 `TODO.md` 的任务执行。

## 初始执行步骤

1. 检查最新一次 git 提交
   - 查看提交信息
   - 查看提交涉及的改动摘要
   - 判断是否提到了待修复的问题、已知缺陷、`FIXME`、`TODO`、回退说明或临时方案

2. 读取任务与计划文件
   - 读取 `TODO.md`
   - 读取 `PLAN.md`
   - 如有必要，读取 `README.md`、`AGENTS.md` 以及与首个未完成任务直接相关的文档

3. 确定本轮目标
   - 找到 `TODO.md` 中第一个未完成任务
   - 判断任务是否过大、是否依赖未实现能力、是否被规范不匹配阻塞
   - 如果任务过大或被阻塞，则先拆分/重排 `TODO.md` 与 `PLAN.md`，本轮仅处理拆分后的第一个子任务或阻塞修复

4. 实施任务
   - 定位相关模块、测试与规范
   - 修改代码
   - 如发现任何规范偏差、现有 bug、缺失功能或依赖问题，按要求将其显式加入 `TODO.md` 并调整顺序

5. 验证
   - 运行与改动直接相关的测试
   - 运行必要的全局检查，至少包括适当范围内的 `cargo test`
   - 按要求尽量保证 `cargo clippy --all-targets -- -D warnings` 无警告；若成本过高或出现与本任务无关的既有问题，需要明确记录

6. 文档与计划同步
   - 更新 `TODO.md`：标记本轮完成的任务，或在阻塞时按依赖顺序重排
   - 更新 `PLAN.md`：记录当前状态、拆分结果、阻塞原因或后续计划
   - 继续更新本文件，记录关键结论与执行进度

7. 提交
   - 生成清晰的 git commit
   - 本轮结束，不继续下一个任务

## 当前状态

- 已完成：创建本计划文件
- 已完成：检查最新提交与任务清单
- 已完成：定位 `T4010b1a` 的实现入口与复现用例
- 已完成：实现 `T4010b1a`、补充回归、完成验证
- 待执行：更新 git 暂存区并提交本轮任务

## 进展日志

- 2026-04-20：初始化本文件，准备进入仓库检查阶段。
- 2026-04-20：检查 `git log -1` 后确认最新提交标题为 `[T4010b1] Lower value computed property access through getters`，提交正文未附带额外已知问题说明，因此没有“提交信息中明确要求先修复”的独立 issue 需要在任务前插入。
- 2026-04-20：读取 `TODO.md` / `PLAN.md` 后确认当前第一个未完成任务为 `T4010b1a`：具体化泛型值类型 member access / getter 读取的结果类型。当前已知最小复现为 `struct Box<T>(val value: T) { val readBack: T get() = this.value }` 下，`Box(9).readBack == 9` 在无 expected type 帮助时仍把读取结果保留为抽象 `T`。
- 2026-04-20：完成实现。`typecheck/expr/member.rs` 现会在值成员读取时，沿 receiver 及其已具体化的 direct supertypes 查找成员所属 nominal 实例，并回到声明处文件重新 lowering 成员原始 `TypeRef`，把 owner type params 用使用点 concrete args 统一替换为结果类型。这样 direct field 与 getter-only property 都不再把读取结果停留在抽象 `T`。
- 2026-04-20：新增回归：
  - `tests/fixtures/typecheck/struct_generic_member_access_result_type_ok.scoop`
  - `tests/fixtures/run-pass/struct_generic_member_access_result_type_basic.scoop`
  - `tests/fixtures/typecheck_multi/generic_value_member_access_cross_file/defs.scoop`
  - `tests/fixtures/typecheck_multi/generic_value_member_access_cross_file/use.scoop`
- 2026-04-20：验证通过：
  - `cargo run -q -p scoop -- test --fixtures target/t4010b1a-fixtures/typecheck`（`fixtures: ok (1)`）
  - `cargo run -q -p scoop -- test --fixtures target/t4010b1a-fixtures/run-pass`（`fixtures: ok (1)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck_multi/generic_value_member_access_cross_file`（`fixtures: ok (2)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (346)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (366)`）
  - `cargo test --all -- --test-threads=1`
  - `cargo clippy --all-targets -- -D warnings`
- 2026-04-20：注意到工作区里已有用户改动 `.github/workflows/ci.yml`，本轮未修改也不会回退该文件；提交时只纳入本轮任务相关文件。

## 当前任务理解

`T4010b1a` 的目标不是只修 computed property，而是把“基于具体 receiver nominal type args 推导成员读取结果类型”的逻辑收口为统一主线，至少覆盖：

- direct field：如 `Box(9).value`
- getter-only property：如 `Box(9).readBack`
- 必要的跨文件 generic nominal member 读取

并保证这些读取在没有 expected-type 帮助时，后续比较/运算能看到 concrete type（例如 `Int`），而不是继续保留抽象 `T`。

## 下一步执行计划

1. 复核 `git status`，确认只暂存本轮任务相关文件。
2. 生成提交，提交信息使用任务号 `T4010b1a`。
3. 本轮结束，不继续处理 `T4010R`。
