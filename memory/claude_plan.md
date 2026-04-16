# 本轮执行计划

## 约束说明

- 按要求先写入计划文件，再执行其他命令。
- 由于内部推理不适合逐字暴露，这里记录可审计的执行计划、决策依据摘要、关键进展与后续调整。
- 本轮目标：先处理最新提交中提到的既有问题；随后读取 `TODO.md`，定位第一个未完成任务，只完成该任务并停止。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到了尚未解决的已知问题。
2. 读取 `TODO.md`，找出第一个未完成任务。
3. 读取 `PLAN.md`、必要的相关代码与测试，判断该任务是否可在本轮完整完成。
4. 如果任务过大，则把它拆分成更小的子任务，并更新 `PLAN.md` 与 `TODO.md`；本轮只做拆分后排在最前的那个子任务。
5. 实现当前任务，并补充/更新相应测试。
6. 运行必要的验证命令，至少覆盖与改动直接相关的测试；若范围允许，再运行更全面检查。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 提交 Git commit，然后停止，不继续做下一个任务。

## 进度记录

- 已完成：创建本计划文件。
- 已完成：检查最新提交与任务列表。
- 已确认：最新提交 `b30d05eaf6f48d60e54cb091ad4b81edff2c218c` 的提交信息未额外声明新的既有问题；初始定位到的第一个未完成任务是 `T3010b2b0R`（review 任务）。
- 已发现问题 1：`crates/scoopc/src/llvm/codegen/mod.rs` 的 `codegen_object_property_access` 在调用内部 `obj_init` 后没有执行 ordinary-frame 的 TLS active 检查；若 object init 触发 non-resuming effect，当前 helper frame 仍可能继续执行到后续语句。
- 已发现问题 2（更前置 blocker）：在补做 helper 侧检查并构造“ordinary helper -> object property access -> object init Raise”定向复现后，观察到 helper 自身虽已不再继续执行，但外层 `handle/try` 的 caller 仍继续执行 call 后 tail。根因是 unified state-machine 的 `known_fun_effects` 只看显式 effect row，没有把 hidden suspend 来源折叠进 callee metadata，导致 caller 把这类 helper 调用误判为 plain `Call`。
- 已采取措施：
  - 已回退临时生产代码与临时 fixture，避免把未完成方案留在工作树中。
  - 已在 `TODO.md` 中插入新的前置任务 `T3010b2b0a`，把 caller-side hidden suspend call 分类缺口前移到 `T3010b2b0R` 之前。
  - 已同步更新 `PLAN.md`，记录复现场景、根因分析与新的任务顺序。
- 当前结果：`TODO.md` 中新的第一个未完成任务已变为 `T3010b2b0a`。
- 进行中：整理本文件并准备提交本轮“任务重排 / blocker 记录”提交。

## 变更记录

- 初始创建，待根据仓库现状继续补充。
- 已补记本轮目标任务与最新提交检查结果。
- 已补记 review 中发现的具体生产代码缺口。
- 已补记更前置的 unified caller-side hidden suspend 分类 blocker，以及因此产生的 TODO/PLAN 重排。
