# AGENTS.md

## 项目概述

Scoop 是一门静态类型、编译到原生代码的语言：以 Kotlin 核心语法为基础，引入真正的值类型（struct / enum / tuple），泛型采用单态化，后端为 LLVM，runtime 用 C（含 Immix 分代 GC）。

权威文档（先读这些再动手，均位于 `docs/specs/`）：

- `docs/specs/SCOOP-SPEC.md` — 语言规范（语法、类型系统、核心库、FFI）；
- `docs/specs/SCOOP-RUNTIME-SPEC.md` — 运行时规范（对象模型、GC 契约、FFI runtime functions）；
- `docs/specs/SCOOP-IMPL-SPEC.md` — 实现大纲（parser → HIR → MIR → LIR → codegen → linker 的 pipeline 与各 stage 职责）；
- `docs/ROADMAP.md` — 实现路线图（纵向主线策略与里程碑序列）。

三份文档用中文撰写；代码中的标识符与注释用英文。

## 范围纪律（硬性约束）

- **只做正常编译器应有的功能**：严禁自行加入超出正常编译器职责的功能和机制。工作应直接服务语言语义、类型检查、IR 变换、代码生成、模块与链接、诊断、产物读写、构建缓存，以及必要的 runtime / GC / ABI / FFI。新设计必须解决当前里程碑中具体、可验证的问题，不得以“更安全”“更完整”“将来可能需要”为由扩大范围。
- **严禁自行引入授权或防伪体系**：不得为 core、普通库、IR 或编译产物设计额外的来源授权、资格认证、令牌或收据信任链、防伪造或反篡改证明。语言可见性、实体归属和引用合法性应以普通的符号、类型和格式规则表达，不得扩展成安全认证体系。
- **严禁自行引入通用资源预算或计费体系**：不得建立跨阶段的逻辑 CPU / 内存预算，或对查询、遍历、复制、哈希、分配逐项计费；不得用这类任意成本模型决定程序是否合法，或将其固化进产物兼容性规则、runtime ABI。必要的越界、溢出、循环引用和终止性检查应针对实际问题局部实现，不得借此扩展为通用配额框架。
- **验证只承担必要的正确性职责**：保留类型、格式、引用、ABI、内存边界和 GC 契约等必要检查。同一事实在明确边界验证后，若数据未变化，应复用结果；不得只为证明前一步已完成的工作，在多个 stage、发布入口或 runtime 热点反复完整重放语义和布局校验。重型交叉验证应放在确有需要的测试或显式验证模式中。
- **禁止为假想需求搭建框架**：抽象和通用化必须有当前实际调用和必要复用作为依据；不得为仅存在于测试中的假想使用者增加工厂、插件层、凭证状态机或多套平行实现，也不得在 IR / meta crate 中另建一套重复的语言语义检查体系。
- **旧 spec 和 design 不能为过度设计背书**：已有文档、所谓“冻结条款”、既有代码和测试都不是继续保留或扩大上述机制的理由。发现这类问题时，应在当前任务范围内按 spec 先行原则修订对应文档，再简化实现；不得用“spec 已经要求”回避对范围和必要性的判断。
- **以可用的编译器能力完成里程碑**：优先完成当前里程碑要求的源码编译、产物消费、链接与运行闭环，测试围绕真实语言规则、产物正确性和功能组合展开。不得把自行增加的授权、证明、预算或通用框架变成新的验收门槛，持续推迟交付。

## 仓库结构

- `compiler/` — 编译器，Cargo workspace：
  - IR / meta crate（stage 之间的数据通道）：`ast/`（`scoop-ast`）、`hir/`（`scoop-hir`）、`mir/`（`scoop-mir`）、`lir/`（`scoop-lir`）——纯数据结构，含各自 `.slib` meta 格式；
  - stage crate（只做输入 → 输出变换，只依赖输入/输出的 IR crate，不依赖上游 stage 的实现 crate）：`parser/`（`scoop-parser`）、`hir-lower/`（`scoop-hir-lower`）、`mir-lower/`（`scoop-mir-lower`）、`lir-lower/`（`scoop-lir-lower`）、`codegen/`（`scoop-codegen`，依赖 inkwell）；
  - `slib/`（`scoop-slib`）— `.slib` 打包与 reader；
  - `driver/`（`scoopc`）— 编译器 CLI，负责 stage 编排。
- `runtime/` — C runtime（GC、FFI runtime functions、核心类型后备实现）；
- `tests/fixtures/` — 测试 fixture（见"编码准则"）。

## 构建与测试

- 编译器实现语言：**Rust**（edition 2024），LLVM 绑定用 **inkwell 0.10（feature `llvm22-1-prefer-dynamic`，包含 `llvm22-1`）**，Scoop 后端使用本机外部 **LLVM 22.1**，不要求与 Rust 工具链自带的 LLVM 版本相同。
- 依赖本机 LLVM 22.1（`llvm-config` 在 `PATH` 中，或设置 `LLVM_SYS_221_PREFIX`）；`scoop-codegen` 与 `scoopc` 之外的其他 crate 无 LLVM 依赖。
- 常用命令：
  - `cargo build` / `cargo test --workspace` — 构建 / Rust 单元与集成测试；
  - `cargo build -p scoop -p scoopc -p scoop-linker --bins` — 构建文件 fixture 使用的三个配套命令；
  - `python3 -m unittest discover -s tests/fixture_runner/tests` + `python3 tests/run_fixtures.py --all` — 公共规则测试与完整文件 fixture 验收；`cargo test` 不能替代这组正式 CLI 覆盖。声明规则与工具位置配置见 [fixture schema 1](tests/fixture_runner/README.md)；
  - `cargo fmt --all` + `cargo clippy --workspace --all-targets` — Rust 格式化 / lint（变更后、测试前先执行，见"编码准则"）；
  - Python 变更先执行 `uv tool run --from ruff==0.16.10 ruff format tests/fixture_runner tests/run_fixtures.py` 与同版本 `ruff check tests/fixture_runner tests/run_fixtures.py`，再运行测试。

## 编码准则

- **不造轮子**：实现时尽量选择已有的、成熟且近期有更新的库；引入新依赖前确认其维护状态。
- **充分测试**：每个特性都要有相应的独立 fixture，以及一组与其他特性相结合的组合 fixture。此外：
  - 大量语言规则是编译错误（见 spec），每个"编译错误"规则都要有对应的 negative fixture（断言报错位置与信息）；
  - pipeline 各 stage（HIR / MIR / LIR）的输出用 golden dump 测试锁定结构。
- **文件长度**：保证代码质量，单个文件控制长度；超长的文件用合理的方式切分成多个子模块，但不做机械的硬性切割。
- **先格式化再验证**：每次完成一批代码变更后，先格式化 + lint，再开始测试、验证或下一步工作。

## Scoop 实现准则

- **输出结构完备**：HIR / MIR / LIR 每个阶段的输出必须从结构上完备——必要的内容由数据结构保证不可能缺失（例如不能用 `Option<TypeInfo>` 保存表达式的类型信息），而不是靠后续代码 `unwrap` / `is_some()` 之类的检查来兜底。
- **不留占位符**：所有条件和分支必须有相应的处理或出口，不允许留下任何"尚未支持"的占位符（`TODO` / `unimplemented` / 空分支）。发现 spec 未覆盖的情况，先回到 spec 讨论，再实现。
- **全局唯一、类型化的实体 id**：全局信息（如类型）必须有全局唯一 id，不允许用 FQN 做回退。不同的实体必须有不同类型的 id，从类型上杜绝混用——例如 generic function、generic function with resolved type param、monomorphized generic function 是三种不同的实体，必须有不同的类型，它们的 id 也必须是不同类型的 id。
- **stage 只经 IR crate 通信**：每个 stage crate 只负责把输入变成输出，只依赖输入/输出的 IR/meta crate，不了解、不依赖上游 stage 的实现；stage 之间的编排由 driver 负责。

## 工作流约定

- **spec 先行**：任何语言行为、runtime 契约、pipeline 职责的变更，先改对应的 spec 文档，再改实现；实现与 spec 冲突时以 spec 为准，发现 spec 本身矛盾时先提出并修订 spec。
- **跨文档一致性**：三份 spec 互相引用（章节号），修改时注意同步引用方。

## 待决策

- （已解决）编译器实现语言：Rust + inkwell，LLVM 22.1。
