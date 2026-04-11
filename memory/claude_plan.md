# 执行计划与进度记录

## 说明

此文件记录可审计的执行计划、决策依据摘要与进度更新，不记录不可见的内部推理细节。

## 初始计划

1. 检查最新一次 Git 提交，确认是否明确提到已有问题、待修复项或已知回归。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否过大：
   - 若可直接完成，则进入实现。
   - 若过大，则先更新 `PLAN.md` 与 `TODO.md`，拆成更小的可执行子任务，并只完成第一个子任务。
4. 在实现前审查相关代码、测试与文档，确认影响范围。
5. 实现该任务，并补充必要测试、注释与文档。
6. 运行相关验证：
   - 最小相关测试
   - 必要时运行 `cargo test --all`
   - 运行 `cargo clippy --all-targets -- -D warnings`
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或依赖调整。
8. 提交 Git commit，然后停止，不继续执行下一个任务。

## 进度

- 已创建本计划文件，准备开始检查最新提交与任务列表。
- 已检查最新提交 `aa23e9f`：提交标题为 `[T0150i] 审计字面量边界值与诊断路径`，未在提交信息中显式声明待修复遗留问题。
- 已定位首个未完成任务：`T0144 审计：编译器 codegen 限制全面排查与任务拆分`。

## 当前执行方案（T0144）

1. 在 `crates/scoopc/src/llvm/`、`crates/scoopc/src/hir/`、`crates/scoopc/src/resolve/`、`crates/scoopc/src/typecheck/` 中搜索以下限制信号：
   - `UnsupportedMainBody` / `Unsupported` / `Todo`
   - `todo!` / `unimplemented!`
   - `HACK` / `FIXME` / 与能力缺口相关的 `TODO`
   - 泛型/复杂类型降级到 `Any`、跳过 type params、硬编码过滤等路径
2. 对每个出现点做分类：
   - 已由现有任务覆盖
   - 刻意保留且非用户可见缺口
   - 需要新增后续任务
3. 产出审计文档，记录代码位置、限制描述、影响范围、建议优先级与分类结果。
4. 将值得修复的限制补充到 `TODO.md`，任务编号续接当前序列。
5. 在 `PLAN.md` 记录本次审计结论与新增任务入口。
6. 运行与本次修改相匹配的验证（至少文档/任务文件自检；若无代码变更则说明原因），随后提交。

## 当前进度更新

- 已完成四个主目录的限制信号扫描，并形成审计文档 `COMPILER_LIMITS_AUDIT.md`。
- 已将 `T0144` 标记为完成，并在 `TODO.md` 中新增 4 个后续任务：
  1. `T0151`：Custom iterator `for` lowering + codegen
  2. `T0152`：safe member access parity（ref receiver / extension property）
  3. `T0153`：receiver function value invocation
  4. `T0154`：higher-order aggregate returns
- 已更新 `PLAN.md`，同步记录审计统计、分类结论与新增任务入口。
- 已修正 1 处过期注释：`crates/scoopc/src/typecheck/expr/call.rs` 中关于 interface dispatch 的说明。
- 已完成验证：
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
  - `cargo run -p scoop -- test`
- 下一步：查看工作区差异，提交本轮变更，然后停止。
