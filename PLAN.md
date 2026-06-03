# Scoop 编译期结构化重构计划（Fact → Tree/Handle）

> 设计依据：[`FACT_REFACTOR.md`](./FACT_REFACTOR.md)（完整概念模型、各阶段设计、DefId/句柄、LIR 草案、handoff 契约）。
> 归档：上一代「Fact 体系统一」计划（手工删 fallback 路线，FG-01–18 / 批 1–4）见 [`docs/archive/fact-unify/`](./docs/archive/fact-unify/)。

## 0. 为什么换路线

上一代计划试图在**扁平 fact 表示内**靠「多发布 fact + verifier + grep 黑名单」逐个堵 fallback。实测（FACT_REFACTOR §3 更新）：`T3-04` 已 13 轮 review 仍未收口、`tools/dependency_gate.py` 已 2460 行黑名单、`lir_facts/verify.rs` 涨到 2007 行——**缺口无固定底，且会因 helper 改名回潮**。根因：fallback 是一条条运行期路径，**没有任何结构禁止它存在**。

本计划改为**结构性修补**：让「缺 fact / fallback」**不可表示**——producer 构造引用闭合的结构 + 定型句柄；consumer 只 walk/deref，没有字符串 key 可回退。届时黑名单 gate 不再需要，**类型系统即 gate**。

## 1. 原则（详见 FACT_REFACTOR）

- 每阶段 = 纯函数；输出是**阶段专属、引用闭合、所有权即树、每节点强类型**的单一结构（§1–§2）。
- **统一身份模式**（§2.7）：每个句柄层一律「**密集下标（live 引用）+ 紧凑 hash（跨 cone/序列化/map key）+ String（仅调试）**」；禁止字符串当 live key。
- **责任划分**（§1.7）：HIR 是唯一前端拒绝边界；MIR 起只做变换、不做类型检查；下游任一处出诊断 = bug。
- side table 仅装全局环境信息（§2.2/§2.7 三分法）。

## 2. 策略：strangler + 后→前

并行另开一条路，**从管线后段往前推进**（FACT_REFACTOR §11）：先建下游、回头建上游时照「已被测试钉死的具体需求」生产；最高风险的 HIR 留到最后。两端是固定锚点（前=源码/AST，后=LLVM IR/object）。每打通一段用现有 fixture 端到端验证；前沿同一时刻一个**升级 shim（旧→新）**，写一个、推进、退役。

## 3. 阶段划分（后→前）

| 阶段 | 范围 | 产出 / 契约 | 状态 |
|---|---|---|---|
| **P1** | **LIR↔codegen handoff** | 抽出独立 LIR 阶段；定义自包含 `LirArtifact` + `CodegenInput`；codegen 只 walk LIR；删去 codegen 对 HIR/MIR/Index/TypeEnv 的直接消费；主/依赖 cone 对称（FACT_REFACTOR §14） | **进行中**（见 `TODO.md`） |
| P2 | LIR 内部重设计 | `LirProgram` arena + 定型句柄 + 自包含指令（lift MIR、消除 overlay）；折叠 `LirFacts` 平表；身份用 §2.7 模式（FACT_REFACTOR §13） | 待 P1 |
| P3 | effect-facts / effect-lowering（P4/P5） | 阶段专属树 + 引用闭合；删 `MirFactIndex` 式 join | 待 P2 |
| P4 | MIR / materialize | MIR 树 + 解析句柄；monomorphization 实例由消费 cone 拥有 | 待 P3 |
| P5 | HIR / typecheck | 三相（检查树→totality gate→hole-free HIR）；DefId 句柄；desugar 边界（FACT_REFACTOR §4–§9） | 待 P4 |
| P6 | 跨 cone 接口面 | 每阶段一等 interface artifact（FACT_REFACTOR §2.4） | 贯穿各阶段 |

> 每阶段完成条件统一：升级 shim 接旧管线 → 全套 fixture 端到端绿 → 该段消费侧无字符串 live key / 无缺-fact fallback → 提交。

## 4. 验证基线（每个任务收尾跑）

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all --all-targets
cargo build -p scoop -p scoopc
python3 tools/dependency_gate.py
python3 tools/spec_fixtures.py check
python3 tools/run_fixtures.py
```

## 5. 当前阶段

P1 = LIR↔codegen handoff。详细任务见 [`TODO.md`](./TODO.md)。
