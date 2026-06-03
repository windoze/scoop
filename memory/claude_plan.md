# 当前任务执行计划

## 约束说明

- 不记录私有内部推理；本文件记录可审查的执行计划、决策点和进度。
- `TODO.md` 是任务顺序和完成状态的唯一权威来源。
- 本次只处理第一个标题未带 `[DONE]` 的任务；若发现阻塞前置缺口，则更新 `TODO.md`、提交并停止。

## 本轮计划

1. 阅读 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交信息，确认是否有与该任务直接相关的未完成事项。
3. 根据任务内容读取必要的代码、规格和测试上下文。
4. 对当前 review 范围执行针对性审查，确认 T3-04 fact-only / fail-fast / dependency gate 是否闭合。
5. 如发现阻塞当前 review 的缺失功能或不符合规格行为，在 `TODO.md` 插入最小前置任务并提交后停止。
6. 如无阻塞，则按 review 要求运行验证，更新 `TODO.md` 将 `T3-04R` 标记 `[DONE]`，提交并停止。

## 本轮进度

- 已读取 `TODO.md` / `TODO-3.md`，本轮第一个未完成任务是 `T3-04R`（Review T3-04）。
- 已检查最近提交：`[T3-04I] Close ninth fallback gaps`，与当前 review 直接相关。
- 审查发现 `T3-04I` 后仍有阻塞残留：LLVM HIR/source class constructor lowering 仍会从 result type / unresolved callee fallback 进入 `codegen_class_ctor_call`，在 `class_ctor.rs` 中通过 source payload span、参数个数/default 参数选择构造器，并通过 span 后缀唯一匹配恢复 init body。
- 已在 `TODO-3.md` 中新增前置任务 `T3-04J`，放在 `T3-04R` 之前；已将 `T3-04R` 的依赖改为 `T3-04J`，并在 review 阻塞记录中登记第十次审查发现。
- 已更新 `TODO.md` 当前活跃任务为 `T3-04J`。本次不需要更新 `PLAN.md`，因为阶段级计划未改变。
- 本轮只修改 Markdown 任务/计划记录，未修改编译产物；按规则不重跑编译、测试或 fixture 套件。`T3-04J` 实现时仍必须执行其任务记录中的完整验证序列。

## 上一轮历史摘录

- 上一轮任务为 `T3-04I`。
- 上一轮已完成主体修改：P6 不再从 reflection source-span bridge 获取类型实参；class ctor readable-path/span-suffix/arg-count fallback 已部分删除；HIR direct-call root scan 已删除；MIR reflection rvalue 携带 `SiteId` 并发布 LIR reflection facts；effect import 缺 stable key 改 fail-fast；MIR backend 普通 source signature 不再本地 `AbiMangler` 合成 ABI。
- 上一轮完成验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（1664 checks）通过。
