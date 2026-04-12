## 说明

按要求先记录执行计划。出于安全与协作边界，这里记录的是可审计的决策摘要、假设、检查项与步骤计划，而不是逐字内部推理。

## 当前目标

本次调用只完成 `TODO.md` 中第一个未完成任务，完成后更新计划与任务状态，运行必要验证，提交 git commit，然后停止。

## 执行步骤

1. 检查最新一次 git 提交：
   - 查看提交说明与改动摘要。
   - 判断是否明确提到任何现存问题、已知缺陷、待修复项。
   - 如果发现属于“提交中提到的既有问题”，优先修复并验证，再继续后续步骤。
2. 读取任务与计划文档：
   - 读取 `TODO.md`，定位第一个未完成任务。
   - 读取 `PLAN.md`，理解现有阶段划分、依赖与约束。
   - 视情况读取与该任务直接相关的规范、README、代码位置与测试。
3. 复杂度判断与任务细化：
   - 判断首个未完成任务是否能在本轮完整落地。
   - 如果任务过大或依赖不清，拆成可执行子任务。
   - 更新 `PLAN.md` 与 `TODO.md`，让首个子任务成为当前执行项。
   - 若因依赖阻塞，明确新增前置任务并重排顺序，但不把原任务标记为 blocked。
4. 实现当前任务：
   - 先阅读相关代码与测试，定位修改点。
   - 按最小充分改动原则实现，不引入 workaround，不偏离规范。
   - 如发现新的规范缺口或实现边界问题，立即转化为更前置的 TODO 任务，更新计划并停止本轮。
5. 验证：
   - 运行与改动直接相关的测试。
   - 运行必要的质量检查，至少覆盖：
     - `cargo test --all`（若成本过高，先运行相关子集，再决定是否补全）
     - `cargo clippy --all-targets -- -D warnings`
     - 任务所需的额外命令（例如 fixture/spec 检查）
   - 若失败，先修复再重跑。
6. 文档与状态更新：
   - 更新 `TODO.md`，把本轮完成的任务标记为完成。
   - 更新 `PLAN.md`，记录当前状态、后续顺序、任何新增依赖或风险。
   - 在关键进展发生时同步更新本文件。
7. 提交与停止：
   - 检查工作区改动，确保只包含本轮相关变更和必要的计划文档更新。
   - 使用清晰提交信息创建 commit。
   - 停止，不继续处理下一个任务。

## 关键约束

- 不使用临时兼容、测试专用 hack、fixture-only 绕过方案来冒充完成。
- 如果实现中暴露出规范不匹配，必须先把该缺口转成更前置的任务并更新计划。
- 不回退或覆盖用户已有未说明改动。
- 每到关键里程碑后更新本文件，便于外部检查当前进度。

## 当前状态

- 已完成：初始计划写入。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`。
- 已确认：
  - 最新提交 `[T2003c0c2b3c4] Support no-immediate while-body indirect escape sites` 的提交说明本身未额外登记“必须先修”的既有问题。
  - 初始读取时，`TODO.md` 中首个未完成任务是 `T2003c0c2b3d`：无 immediate-resume 的 multi-arm handle 支持 direct + indirect mixed site-matrix。
  - 通过阅读 `crates/scoopc/src/llvm/codegen/effect/{mixed.rs,matrix.rs}`，确认仓库内已经存在一批通用的 mixed matrix helper；当前缺口主要是 no-immediate 路径的接线与回归，而不是还缺底层基础设施。
- 决策：
  - 进一步审计后改变复杂度判断：原始 `T2003c0c2b3d` 同时覆盖 top-level mixed 与 nested same-stmt mixed（block / if / while），单轮实现面仍然过大。
  - 已按仓库既有拆分模式，把 `T2003c0c2b3d` 细化为 `T2003c0c2b3d1`～`T2003c0c2b3d4`，当前执行项改为 `T2003c0c2b3d1`：top-level direct + indirect mixed site-matrix。
  - `T2003c0c2b3d2`～`T2003c0c2b3d4` 继续留待后续调用处理。
- 已完成：在 `crates/scoopc/src/llvm/codegen/effect/mixed.rs` 为“无 immediate-resume + direct/indirect mixed + top-level only”新增专用 lowering 分流，并通过 `cargo check -p scoopc --features llvm` 验证编译通过。
- 已完成：新增 run-pass fixture `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop`，同一文件覆盖：
  - direct -> indirect -> direct 的 multiple mixed top-level 序列；
  - indirect -> direct 的 top-level mixed 顺序；
  - sibling custom non-resuming arm（`Abort.stop`）以及前一 escape site 结果跨后续 mixed suspension 的保留语义。
- 已完成：用 `cargo run -p scoop --features llvm -- build ...` + 直接执行产物验证新 fixture，输出已固化到对应 `.stdout`。
- 已完成：手工验证 `tests/fixtures/build/effect_multi_escape_direct_indirect_while_is_error.scoop` 仍保持原有失败诊断，说明 while mixed 边界还锁在后续 `T2003c0c2b3d4`。
- 已完成：全量质量门禁通过：
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`（`fixtures: ok (982)`）
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已完成：`TODO.md` 已将 `T2003c0c2b3d1` 标记为完成，并记录实现/回归摘要；`PLAN.md` 已把下一步切换为 `T2003c0c2b3d2`。
- 下一步：
  1. 检查最终 diff，确认仅包含 `T2003c0c2b3d1` 实现、回归与计划文档更新。
  2. 创建本轮 git commit。
  3. 停止，等待下一次调用处理 `T2003c0c2b3d2`。
