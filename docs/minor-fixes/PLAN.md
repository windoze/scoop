# Scoop 小改动分步实施计划

状态：P0–P9 及补充的 consumer、negative、缓存清理均已完成并分批提交；fixture 和 Cone 测试的三个目标验收均已闭合，结果见 [PROGRESS.md](PROGRESS.md)。更新日期：2026-10-22。代码基线：`4de3ae9e8ac3f18bc63eba4947afd8bb87ea34db`。

配套文件：[设计文档](DESIGN.md)。package 前缀、core/std 豁免（D02）、单文件 package 警告后忽略（D03）、始终默认依赖 core/std 且 dep 只追加（D10）、`//! dep:` 和保留可选 `--` 的规则已经确认。其余边界采用原设计的推荐方案，全部选择见设计文档的边界决策表。

按可独立审阅的批次实施。每批先修改对应规范，再修改实现与直接受影响的测试；通过必要检查后进入下一批。测试分组先于减少模式覆盖，目录搬迁放在行为稳定之后，避免把语义变化、覆盖变化和路径变化混在一起。

## 批次和依赖

| 批次 | 交付 | 前置依赖 | 相关决策 |
| --- | --- | --- | --- |
| P0 | 规范变更清单和边界选择 | 无 | 全部 D 项已选定；按功能先修订规范 |
| P1 | 测试职责分类与迁移基线 | 现有测试清单 | 按已选定的语言边界分类 |
| P2 | package 校验及源码迁移 | P0 对应规范，P1 基线 | D01；D02、D03 已确认 |
| P3 | `scoop new` 固定模板 | P2 的 package 语义稳定 | D04 |
| P4 | shebang 与单文件参数 | P0 对应规范 | D05、D08；若测到 package 入口则采用 P2 规则 |
| P5 | 单文件默认与显式依赖闭环 | P4，P2 的单文件规则 | D06、D07；D10 已确认 |
| P6 | 内置库迁入 `scoop.test` | P1，P2 | 无新增产品决策；独立 core 消费测试仍遵守普通 package 规则 |
| P7 | GC stress 构建开关、普通测试 release 化与模式隔离 | P1、P6；构建开关可先行 | D09；stress 支持须显式构建已由用户确认 |
| P8 | fixtures 目录与名称重组 | P2 至 P7 中影响被搬迁用例的行为已稳定 | 无新增产品决策 |
| P9 | 完整验收与交付记录 | 前述批次完成 | 逐项核对全部已选定边界 |

按依赖关系分功能提交。P4 与 P2/P3 在代码上相对独立，P1 可先行推进；这些依赖关系不要求额外创建并行任务。P5 的内部工作可以分提交审阅，但不能发布一个接受依赖注释却仍忽略依赖的中间状态。

## P0 保存设计并确定规范变更

### 工作内容

1. 将设计和计划保存到 docs/minor-fixes，保留用户已确认规则，记录其余边界采用推荐方案的实施选择。每个功能先修订对应权威规范，再修改实现。
2. 修订语言规范中 package 与 Cone group 的关系及 core/std 豁免；规定单文件声明 package 时警告并忽略、生效 package 固定为内置默认值；明确首行 shebang、始终存在的 core/std 默认直接依赖，以及 dep 仅追加依赖的规则。
3. 修订实现规范的单文件构建图、子请求、直接依赖集合、缓存、产物分类、链接及参数传递规则。
4. 更新 fixture runner 的说明和测试组织约定；仅在需要改变产品 `scoop test` 行为时修改该命令合同。

### 规范映射

| 规范位置 | 本批应修改的内容 |
| --- | --- |
| [语言规范第 12.1、12.2、12.3、12.4 节](../specs/SCOOP-SPEC.md) | group 前缀及 core/std 豁免、普通源码省略 package、单文件 package 警告并忽略、默认/显式依赖与可见性、保留 identity |
| [语言规范第 9.7 节](../specs/SCOOP-SPEC.md)及词法说明 | 保留 `//!` 普通注释属性，明确 shebang 不形成文档片段 |
| [实现规范第 2.7 节](../specs/SCOOP-IMPL-SPEC.md) | new/run 输入、头部解析、单文件依赖、子进程协议与缓存读取顺序 |
| [实现规范第 2.6、2.8 节](../specs/SCOOP-IMPL-SPEC.md) | 带依赖的 LocalExecutableRoot、正常 `.slib` 消费与显式链接 |
| [fixture runner 说明](../../tests/fixture_runner/README.md) | release 默认策略、专测模式、GC 环境和功能分组 |

除已确认的 GC stress 构建能力开关外，这批改动不改变普通 GC 语义、runtime ABI 或 FFI 合同。stress 的编译与运行选择同步写入 runtime 规范，不因模板或文件注释功能扩大范围。

### 完成门

- 每个规范变更能对应一个实际实现入口和验收场景。
- 用户已确认规则与实施选择均有记录；旧的 core-only 叙述在相关现行规范中一致更新。
- 没有为了脚本引入新的身份、授权、资源预算、来源审计或缓存恢复体系。

## P1 建立测试分类和迁移基线

### 工作内容

1. 使用现有 discovery 校验 fixture 的载体、输入所有权和期望引用，保存一份仓库外的迁移基线。
2. 对有多 profile、GC stress、跨产物或完整结构断言的用例逐组确认实际测试目的。
3. 用现有 tags 和迁移说明区分普通功能、编译器正确性、GC 正确性、库单元测试、外部消费者及回归记录。
4. 为即将合并或搬迁的用例记录“旧名称与位置 → 新位置或测试方法 → 保留的断言 → 保留的模式理由”。无需新增运行期注册表。
5. 先选出需要保留 debug/release、混合优化、ODR、ABI、异常清理、relocation 等矩阵的代表用例；尚未确认归属的用例暂不减少覆盖。

### 完成门

- 4,532 个基线 fixture 的分类可以追溯；迁移后的用例数不要求机械地保持相等。
- 拟移除的每个普通变体都有理由；专测中的每个保留变体有具体被测事实。
- 不能仅按 `mNN`、`moving` 或旧 `gc` 标签批量删除用例。
- 此批只建立分类，不改变测试运行语义，现有发现与引用检查通过。

## P2 实现 package 规则并迁移受影响源码

### 工作内容

1. 在掌握 manifest 和 AST 的前端入口加入逐段前缀校验，覆盖 build/test/doc；不让 parser 依赖 manifest，也不在多个 IR 阶段重验。
2. 实现 core/std 的明确豁免；单文件使用现有内置默认 package，显式声明只产生警告并在语义处理时忽略，保留原文和 span。普通 manifest 源码省略 package 的行为依 D01 实施。
3. 迁移主要内置库、测试辅助 Cone、示例和 fixtures 中受影响的声明及导入。单文件中真正验证命名空间的用例迁入 manifest 载体；无关的 package 声明可移除。保留原负例的被测错误，不机械删除全部声明。
4. 同步调整诊断位置、测试显示名称和直接依赖名称相关期望。对 package 变化影响的库重新准备对应 sysroot 产物。

### 验收场景

- group 本身和多个层级的子包通过；相似字符串但不同名称段的 package 被拒绝。
- 连字符 group 的转义标识符通过；单个反引号名称中的点不能冒充多个 package 段。
- 生产源码、测试源码和文档入口给出相同的 package 规则及准确错误位置。
- core/std 的根包或其他 package 不被前缀检查拒绝；其他内置库、同名不同 group 的 Cone 和独立测试辅助 Cone 不继承例外。普通源码省略 package 覆盖 D01 选定行为。
- 单文件省略 package 时使用 RootPackage；显式合法声明产生定位准确的警告，编译继续且名称解析仍使用 RootPackage。与省略声明的等价程序对照，确认声明没有残留命名空间效果。
- package 原文与行号保留，语法错误仍被诊断；普通 manifest 的合法声明不产生单文件警告。警告展示遵循既有缓存诊断约定，不在多层处理时重复发出。
- 合法 split package、跨 Cone import/re-export 及 typed identity 行为保持正确。

### 完成门

必要 Rust 检查、package 专测及受影响的名称解析/可见性/测试目录/doc 用例通过。迁移引起的 ID 或诊断变化有具体源码原因；不通过全量重录 snapshot 隐藏变化。

## P3 增加 scoop new

### 工作内容

1. 增加独立 New 参数和执行入口，不混入 build 的 native、cache 或 runtime 选项。
2. 复用 coordinate、SemVer 和 package 校验，实现 D04 选定的模板参数。
3. 生成 executable/library 的固定文件集合及可选 `scoop.test` 用例。普通 manifest 模板沿用现有默认依赖，不套用单文件 core/std 默认值。
4. 写文件前检查输入和目标冲突；按普通 I/O 规则报告失败，不覆盖原文件。
5. 增加命令 help 和简短使用示例。

### 验收场景

- 默认 executable 模板 build/run 成功，生成的测试在 release 通过。
- library 模板不带 main，构建和测试成功。
- 显式 group/name/version/package 正确写入；连字符与关键字等情况使用现有名称转义。
- 非法 coordinate、越界 package、目标冲突和实际 I/O 失败给出有效诊断。
- 同一路径再次 new 不覆盖已有内容；已有空目录和非空目录均拒绝。

### 完成门

全部交付模板无需用户手工修补即可使用；模板测试与命令自身参数测试通过。没有加入外部模板下载或模板框架。

## P4 支持 shebang 和可选分隔符参数

### 工作内容

1. 依据 D05 在 lexer 中将允许位置的 shebang 作为 trivia 消费，保持原始源码位置。
2. 在 run 的单文件输入中开放普通位置参数；保留显式 `--`、已知 runner 选项和转发开始后的原样参数。
3. 依据 D08 处理转发前未知的连字符参数。
4. 保留 `scoop run -- args` 的默认 Cone 语义，保留普通目录/Cone.toml 的现有参数规则。
5. 更新 help 与脚本示例，明确程序自身首个 `--help` 应写在分隔符之后。

### 验收场景

| 方面 | 必需覆盖 |
| --- | --- |
| 词法 | 有/无 shebang、LF/CRLF、只有 shebang、行尾 EOF、非首行 `#!`、D05 的 BOM 与前导空白规则 |
| 位置 | 第二行及之后的错误行列/字节位置保持准确，注释不形成文档片段 |
| 参数 | 设计文档全部行为表，特别是已知选项冲突、首个位置参数后的 `--release`、字面 `--` |
| 兼容 | 无 root 的 `run -- args`、普通 Cone、runner 选项值中包含 `.scoop` 的路径 |
| 字节 | 空参数、含空格参数、适用平台上的非 UTF-8 参数；不经 shell 重解释 |
| 实际执行 | 从可执行 `.scoop` 脚本直接启动，无/有参数及显式 `--` 均正确；退出状态及 stdio 传递不回归 |

### 完成门

解析层行为测试和正式 CLI/shebang 用例通过。不能只以 Clap 配置编译成功认定完成，也不能以直接 `scoop run` 成功替代操作系统脚本执行测试。

## P5 贯通单文件依赖

### 内部实施顺序

1. **头部与生效依赖**：实现 D06 的扫描和诊断；统一生成 `{core, std} ∪ 显式 dep` 的直接依赖集合。默认与显式依赖合并时不重复建立同一坐标的边，源码内重复声明及冲突版本按 D06 和现有规则处理。区分省略与非法指令，不因解析失败回退默认值；复用坐标校验并保留原文和 span。普通 Cone 不解释依赖内容。
2. **更新记录**：以输入状态和解析规则版本复用头部结果，先判断更新再读取源码；更新默认策略对应的缓存版本，防止复用旧 core-only 图。实际 scoopc 工作复用同一份已读文本。
3. **图发现**：开放 `--cone-path`，按 D07 加入生效直接依赖及传递闭包，core/std 始终保留，沿用已存在的定位、版本冲突、环和 library 检查。
4. **编译输入**：贯通 child plan、protocol、scoopc shape 和生效/实际 direct set 校验；直接调用 scoopc 沿用现有 core 处理，由调用者提供 std 及其余所需产物，无论是否存在 dep 声明。
5. **产物与链接**：解除 `.slib` 生产和链接的 core-only 条件，保留 LocalExecutableRoot、单 source 和禁止作为依赖的条件。
6. **缓存与兼容**：验证头部、源码和依赖变化导致正确更新；按实际请求或编码变化处理已有版本协商。

### 验收矩阵

| 组别 | 必需覆盖 |
| --- | --- |
| 正常输入 | 无 dep、有一条或多条 dep 时均保留 core/std；有/无 shebang、D06 允许的注释与空白组合；显式写出默认 core/std 坐标时不重复建立依赖边 |
| 真实库消费 | 无 dep 时可显式 import std 并运行；只声明 CLI 等额外库时，仍可直接 import std；至少消费一个有传递依赖的库 |
| 定位 | sysroot 默认库、`--cone-path`、缺失/歧义候选、目标或坐标不匹配 |
| 错误 | 非法三段坐标、非法版本、源码内重复声明、图内版本冲突、executable 依赖、与默认 core/std 冲突的版本 |
| 模式隔离 | 同一份合法/非法 dep 文本在普通 Cone 中只是注释；相邻 manifest 与其他源码不改变单文件输入集合 |
| 名称可见性 | 生效直接依赖之外的库不可直接导入，传递依赖不自动变成 direct import；默认和显式依赖都不自动 import |
| 编译入口 | `scoop build/run` 与直接 `scoopc build --direct-slib ... --support-slib ...` 使用同一依赖合同；有/无 dep 时缺失 std 均失败，不退回 core-only |
| 产物 | 带依赖的单文件构建和显式 link/run 成功；该单文件产物仍不能作为另一个 Cone 的依赖 |
| 更新 | 冷构建、无变化命中、只改正文、增加首条/删除最后一条 dep 后 core/std 仍保留、默认或显式依赖产物变化、策略版本变化、失败不发布且不运行旧文件 |
| 组合 | shebang、默认/显式依赖、程序参数与真实库共同工作；加入 package 声明后警告但仍使用默认 package，dep 位置按 D06 处理 |

### 完成门

以上端到端链路闭合后再交付。旧“single-file 拒绝所有依赖”和“无 dep 的直接依赖仅一个 core”的测试按新规则替换；同步更新受影响的依赖集合、初始化顺序、链接输入和缓存期望。只应删除已改变的约束，保留输入隔离、产物角色和依赖合法性测试。

不得为了缓存验收删除、搬走或隐藏源码、工具、原产物；不添加来源认证或重复产物内容复核。

## P6 迁移内置库测试

### 工作内容

1. 按 P1 分类，从 `scoop.cli`、`scoop.json` 和 `scoop.std` 的纯库 API 行为开始迁移。
2. 在对应 `tests/` 下按功能组织 `@Test` 类和方法，复用 `scoop.test` 断言；必要时保留已有 native 测试辅助 Cone。
3. 为 scoop.test 添加自测，不引入对自身的依赖；为 core 的公开行为建立独立消费者测试 Cone，避免 core/test 依赖环。
4. 将混合 fixture 中的库行为与 CLI、链接、缓存、生产消费者、外部对端等边界分开。
5. 把测试框架自身的用例统计、过滤、忽略和生命周期检查放到小型固定测试 Cone，避免绑死整个标准库的测试数量。
6. 新测试在 release 下通过且职责对应关系确认后，再删除重复 fixture。

### 完成门

- 每组被删除 fixture 的断言在库测试或保留的边界用例中有对应位置。
- 同 Cone 测试迁移没有替代生产产物和跨 Cone 可见性验证。
- 依赖库按 Production 构建，不递归运行依赖的测试；正常生产构建不引入测试依赖。
- 受影响内置 Cone 的 release 测试及保留的消费者/CLI 用例通过。

## P7 调整 profile 与 GC 运行模式

### 先实现 GC stress 构建开关

1. 先修订 runtime 和实现规范：默认不编入 stress 支持，`--enable-gc-stress` 仅编入能力，再由 `SCOOP_GC_STRESS_MOVE` / `SCOOP_GC_STRESS_MINOR` 控制运行时开关。
2. 贯通 scoop 的全局参数与 build/run/test/link、runtime 构建配置和预编译选择；提供两类 runtime 资源，不改普通 GC 语义和库身份。
3. 在 C 编译时排除默认版本的 stress 专用代码与环境读取；更新 runtime 配置编码、准备脚本和实际受影响的缓存键，拒绝显式 runtime 能力不匹配。
4. 先检查默认程序在 stress 环境下仍无效，再验证启用支持后环境值 `1`、`0`、未设置的行为，以及开关切换与重复构建的链接缓存行为。
5. 更新 runtime 定向 C 测试与 GC fixtures，使其显式构建 stress 版本；普通 fixtures 不传该开关。该功能完成后独立提交，再继续模式清理。

### 工作内容

1. 按 D09 统一普通库测试和普通 fixtures 的 Scoop 编译 profile 为 release，包括原来只有 normal 的默认 debug 调用。
2. 对直接 `scoopc build` 使用明确的 release profile；调整产物路径和相关 assertions。
3. 移除普通用例重复的 debug/release 变体和独立 moving 步骤。
4. 保留 P1 确定的 compiler-correctness 与 GC 专测模式；每个额外模式保留说明。
5. runner 的子进程环境清除继承的 `SCOOP_GC_STRESS_MOVE` 与 `SCOOP_GC_STRESS_MINOR`，再允许专测显式设置。保留其他用户环境，不新增通用过滤机制。
6. 更新 runner 单元测试、说明和公共样例，确保新增普通 fixture 不再依赖隐含 debug 默认值。

### 验收场景

- 检查普通命令确实选择 release；不能仅以 variant 名称是 release 判断。
- 测试 runner 的普通子进程在父进程设置 stress 时仍默认关闭；使用小型受控单测验证该合并规则，不以全量 stress 验收代替。
- GC 专测的变体和步骤级显式设置仍可生效。
- 编译器专测保留必要模式差异；产品默认 profile 的专测仍验证原合同。
- 既有共享 cache/sysroot 与变体工作目录行为不回归，不无故为每个 fixture 重建相同依赖。

### 完成门

普通组没有无理由的 debug/release 双跑或 stress；专测覆盖没有因清理消失。受影响 runner 单测和所迁移组通过后，不再反复跑无变化的完整矩阵。

## P8 按功能和回归原因重组目录

### 工作内容

1. 按设计中的 language、driver、artifacts、native、compiler-correctness、gc、regressions 分类逐组搬迁。
2. fixture 的 name/tags 使用功能名称；回归用例记录缺陷行为及已有 issue/commit 信息。
3. 更新相对输入、support、root、snapshot/诊断路径、活动脚本和 `--suite`/`--filter` 使用方式。
4. 核对所有 `.scoop` 输入仍归属于 fixture inputs/support 或正式 Cone 测试集合。
5. 更新当前文档入口。历史里程碑验收保留原始结果，必要时增加现有位置说明。

### 完成门

发现与 schema 校验无重复、遗漏、悬空引用。纯移动部分没有无意修改源代码、期望或模式。每组移动后只运行受影响的必要用例；最终完整验收在 P9 统一进行。

## P9 最终验收和交付记录

### 验证顺序

1. 核对已确认需求与设计完成条件全部有实现及覆盖；D02、D03、D10 已落实，其余 D01、D04 至 D09 均有明确选择并落实。
2. 按变更涉及的语言运行格式化和 lint，再运行测试。
3. 运行 Rust workspace 与 Python runner 测试，按脚本说明显式准备开发 sysroot。
4. 在各适用宿主运行内置 Cone release 测试、完整 fixtures，以及已选定的编译器/GC 正确性矩阵。
5. 验证 new 模板、shebang、dep、被忽略的 package 和参数转发的组合；旧无 dep 单文件按新 core/std 默认值运行，默认 Cone 和显式 `--` 的兼容路径保持正确。
6. 记录结果与实际限制，修复失败后只补验受影响范围；除非新变化引入理由，不重复已通过的完整检查。

### 常用检查命令

以下命令从 Scoop 仓库根执行。Python 使用符合 runner 要求的 3.11+ 环境；编译与链接工具按仓库说明配置 LLVM 22.1。

```sh
cargo fmt --all
cargo clippy --workspace --all-targets

uv tool run --from ruff==0.16.10 ruff format tests/fixture_runner tests/run_fixtures.py
uv tool run --from ruff==0.16.10 ruff check tests/fixture_runner tests/run_fixtures.py

cargo test --workspace
python3 -m unittest discover -s tests/fixture_runner/tests

cargo build -p scoop -p scoopc -p scoop-linker --bins
# Explicitly prepare the development sysroot per scripts/README.md first.
SCOOP_SYSROOT="$PWD/sysroot" python3 tests/run_fixtures.py --all

git diff --check
```

脚本准备和 native 资源步骤以届时的 [scripts 说明](../../scripts/README.md)为准。不要让普通 fixture runner 自动执行工具链打包或 sysroot 准备。

目标平台为 `aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。各用例只在其适用 target 运行；跨平台行为或 ABI 专测保留必要差异，不能将不适用项计为通过。某宿主未执行的范围在交付记录中明确保留，不能以本机通过替代。

### 完成记录

记录基线与最终 revision、改动范围、各目标与测试组的结果概要、失败闭合情况和实际限制。fixture 数量可因合并与迁移下降，必须通过职责对应解释覆盖变化，不能单纯以通过数量增减判断完成。

原始日志、JSON 报告和临时产物放仓库外。仓库文档只保存精简结论，不保存逐次运行的大型日志或压缩归档。

## 实施中的控制点

- **fixture 作为普通测试手段**：补充逐组审查库 consumer/artifacts；能用 scoop.test 完成的 API、错误路径和组合行为全部迁入所属 Cone，重复已有行为的直接合并。只有不能在测试进程中完成的断言才保留 fixture，不以生产库消费或附带缓存检查作为例外。
- **按库复制编译错误**：将普通 negative 按编译器规则与既有 expected-error 覆盖对应，去掉跨库重复项；独有的内建语义错误归入语言规则组。记录覆盖对应，迁移后重新执行 discovery，并验证受影响的 Cone 和新增/修改的 expected-error。
- **正在运行的完整验收**：先在本地验证迁移；保留正在执行用例的真实输入，只有确认新提交不涉及活动选择的 inputs/support 时才同步。最终报告剔除已退役用例并补入新测试结果，不因整理目录重跑无关测试。

- **package 提前遮蔽负例错误**：在 P2 迁移其合法外围源码，保留原测试目的。
- **例外范围扩大**：core/std 的豁免不扩展到整个 scoop group 或独立辅助 Cone，不为前缀约束迁移这两个例外的 package。
- **单文件 package 只警告却仍生效**：P2 验证声明归属和名称解析真正使用内置默认 package，并保留原语法位置；有 namespace 目的的旧用例迁入 manifest 载体。
- **新默认依赖停留在文档或 wrapper**：P5 统一 `{core, std} ∪ 显式 dep` 集合，覆盖直接 scoopc、旧断言和缓存更新；新增 dep 不移除默认依赖，缺失 std 不静默回退。
- **只开放 parser 而依赖链仍受限**：P5 同时覆盖 graph、protocol、driver、slib 和 link，禁止以单个 import 成功替代闭环。
- **省略 root 的参数回归**：P4 固定覆盖 `scoop run -- args`，不全局机械替换 `last = true`。
- **缓存命中仍读文件头**：P5 将解析记录复用放在源码读取之前，验证实际调用与更新结果。
- **迁移后覆盖变少却无对应解释**：P1 先建立对应关系，P6/P7 再移除重复项。
- **目录变化掩盖行为变化**：P8 独立进行，复核活动入口和期望路径，保持移动 diff 可审阅。

本计划完成的标准是设计合同和必要验证全部满足，不以文件搬完、模式数量下降或编译命令退出为零单独作为验收依据。
