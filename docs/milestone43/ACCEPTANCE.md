# M43 验收核对

状态：Darwin 与 Linux GNU 验收完成，Linux musl 验收进行中；尚未判定 M43 完成。日期：2026-10-14。

范围依据 [设计第 9～10 章](DESIGN.md#9-实施批次)，实施与失败修复历史见 [PROGRESS](PROGRESS.md)。M43 仅交付同 Cone 单元测试；integration-test 等待 multi-cone workspace 定型。首个实际应用为 scoop.std。

## 1. 实施批次与实际实现

| 批次 | 已有实现与直接证据 | 剩余验收 |
| --- | --- | --- |
| M43-1 测试库与断言 | [普通 scoop.test Cone](../../sysroot/lib/scoop.test/)、语言规范第 16 章；8 个公开断言函数及 AssertionFailure，10 个 assertions fixture 覆盖真实库消费、错误约束与失败位置 | Linux 对应 fixture |
| M43-2 测试输入与依赖 | 语言规范 12.2.4、实现规范 2.7；manifest test.sources/test-dependencies、前端源码与直接依赖角色、测试构建键和独立输出；manifest、boundaries、source-selection fixture | Linux 对应 fixture |
| M43-3 发现与生成入口 | [声明检查](../../compiler/hir-lower/src/test_annotations.rs)、[typed 目录](../../compiler/hir/src/test_suite.rs)、[入口合成](../../compiler/hir-lower/src/test_entry.rs)；21 项发现／入口 Rust 测试，生成入口的正式 CLI fixture | Linux 生成程序的执行与产物组合 |
| M43-4 CLI 与报告 | [选择](../../compiler/scoop/src/cli/test_selection.rs)、[进程运行](../../compiler/scoop/src/cli/test_run.rs)、[事件读取](../../compiler/scoop/src/cli/test_run/events.rs)、[报告](../../compiler/scoop/src/cli/presentation/test_report.rs)；list、selection、report、process、shutdown fixture | Linux 对应 fixture；GNU process 已通过，musl process 待完成 |
| M43-5 实际库、产物与平台 | [scoop.std 同 Cone 测试](../../sysroot/lib/scoop.std/tests/)、std-application/std-selection、无源码依赖与 artifact-only 链接 fixture | GNU／musl 整组结果与失败项复验 |

普通生产构建不包含测试源码或测试依赖；测试构建复用普通编译、链接、ABI、异常与 GC 路径。测试目录中的显示名称只用于展示和过滤，实际调用使用 typed class/callable。生成入口具有独立 generated identity，未改写用户 main。未新增 runtime 公共 ABI 或通用测试引擎。

## 2. 设计矩阵与对应证据

下表的 fixture 均已在 Darwin 完成实际 CLI 验收，运行型正例按各自声明覆盖 debug/release、moving GC 或两者。声明类负例检查完整诊断与实际源码位置。Linux 状态统一见第 4 节，不能用 Darwin 结果代替。

| 设计要求 | 直接验收证据 |
| --- | --- |
| 首个实际应用 | [std-application](../../tests/fixtures/m43-std-application/fixture.toml) 验证生产隔离、只构建、运行与 moving GC；[std-selection](../../tests/fixtures/m43-std-selection/fixture.toml) 验证 3 个容器、10 个用例的列表、整容器和单方法过滤及缓存复用 |
| 容器、发现、原名称与 helper | [test-list](../../tests/fixtures/m43-test-list/fixture.toml)、[source-selection](../../tests/fixtures/m43-source-selection/fixture.toml) 覆盖多文件 private 容器、Unicode／反引号、空容器、alias、同名普通注解及泛型 helper；[发现 Rust 测试](../../compiler/hir-lower/src/tests/m43_test_annotations/discovery.rs) 检查 typed 目录、直接声明方法及普通构造决议 |
| 容器、方法、标记与构造错误 | [49 项 declarations](../../tests/fixtures/m43-declarations/) 覆盖非法容器形态、方法归属／签名／安全性、重复标记、角色冲突、hook 数量、Ignore 及空／全部忽略容器的非法构造；[生产标记负例](../../tests/fixtures/m43-markers/) 覆盖真实标记身份与 re-export |
| 实例、生命周期、串行与共享状态 | [lifecycle](../../tests/fixtures/m43-lifecycle/) 验证新实例、初始化／准备／正文／清理顺序、finally 和两次失败保留；[generated](../../tests/fixtures/m43-test-purpose/generated/fixture.toml)、[failures](../../tests/fixtures/m43-test-purpose/failures/fixture.toml) 验证生成入口的真实调用与输出 |
| 过滤、忽略和列表无实例副作用 | [selection](../../tests/fixtures/m43-test-selection/fixture.toml) 覆盖默认、filter、ignored、空匹配、构造失败与直接启动；test-list 验证列表没有 initializer／hook／正文输出 |
| 源码选择与可见性 | [boundaries](../../tests/fixtures/m43-boundaries/) 检查 private、生产引用测试声明、路径重叠／缺失／类型／编码、空生产源码；source-selection 检查平台条件、显式清单、空清单以及生产正文／默认实参／泛型正文的重载隔离 |
| 测试依赖 | [manifest](../../tests/fixtures/m43-manifest/)、boundaries 检查 exact 配置、重复／覆盖、环和版本冲突；[artifact-dependencies](../../tests/fixtures/m43-test-artifact-dependencies/fixture.toml) 验证依赖自身的无效 tests 不被启用，并消费无源码生产依赖 |
| 断言、求值和异常 | [assertions](../../tests/fixtures/m43-assertions/) 覆盖 8 个 API 成功／失败、参数求值顺序与次数、预期异常返回／子类／错类型／未抛、AssertionFailure 的继承边界和 finally |
| 类型与渲染组合 | assertions 的 equality、identity、rendering 及 bound 负例覆盖 Equals、结构比较、引用身份、Float／Double NaN 与正负零、缺少 ToString、格式化抛出和非法 Throwable 类型参数 |
| 来源位置、分层报告与输出 | assertions/location 覆盖跨 Cone 默认位置；[report](../../tests/fixtures/m43-test-report/fixture.toml) 验证各阶段、目标／容器／用例归属、计数、耗时、非 UTF-8 输出、nocapture、渲染错误及 0 用例；CLI Rust 测试补充事件解码与人类可读报告 |
| 入口、进程退出与中断 | artifact-dependencies 验证生产 main 不自动执行但可显式调用；[process](../../tests/fixtures/m43-test-process/fixture.toml) 验证 exit(0)、runtime panic、SIGSEGV、父命令收到 SIGINT 后的真实子进程结果；report/shutdown 验证 abort、eager 初始化及正常用例完成后的 shutdown 失败 |
| CLI 选项与失败后的输出 | scoop CLI Rust 测试检查参数与互斥选项；[selection-invalid](../../tests/fixtures/m43-test-selection-invalid/fixture.toml) 验证 filter 不能隐藏编译错误，失败后旧 binary 保持但不运行；test-list、selection、generated 覆盖 list／ignored／no-run 与 profile |
| 产物与缓存 | generated、failures 验证生产／测试隔离、热缓存、测试变化后的新指纹与实际执行输出；artifact-dependencies 删除依赖源码后构建，再删除根源码执行 artifact-only 链接；共享入口回归覆盖四种生产 main 与无源码产物消费 |
| GC、Context 与平台 | [retention](../../tests/fixtures/m43-lifecycle/retention/program/src/main.scoop) 在 hooks 分配／GC 后检查实例、首次失败、清理失败和 Context 恢复；constant-dynamic-root 覆盖修复后的 LLVM 根合并；三目标的 CLI 执行状态见第 4 节 |

不能运行外来 target 时，当前宿主的真实请求均在目标工具链解析处失败，没有宿主回退，见 [Darwin→GNU](validation/foreign-target-darwin.json.gz) 和 [Linux→Darwin](validation/foreign-target-linux.json.gz)。本环境没有对应交叉工具链，这两项不声称覆盖已链接外来程序的执行；执行阶段沿既有 OS spawn 错误路径返回工具诊断。

## 3. Darwin 已完成证据

当前发现的 M43 fixture 为 105 项。按名称合并以下报告，用后续成功复验替换对应早期结果，得到 105 项通过、127 个变体、288 个进程步骤；没有忽略早期失败或将失败步骤计为通过。

| 报告 | 使用规则 |
| --- | --- |
| [initial](validation/acceptance-initial-darwin.json.gz) | 基础 43 项；两项热缓存断言失败由 warm-cache 替换，selection-invalid 的扩展也由该复验替换 |
| [warm-cache](validation/warm-cache-darwin.json.gz) | 3 项、5 个变体、47 个步骤，替换上述 3 项 |
| [declarations](validation/declarations-darwin.json.gz) | 49 项、49 个步骤 |
| [artifact-dependencies](validation/artifact-dependencies-darwin.json.gz) | 1 项、2 个变体、12 个步骤 |
| [boundaries](validation/boundaries-darwin.json.gz) | 11 项、12 个变体、16 个步骤 |
| [panic/process](validation/panic-darwin.json.gz) | 1 项、2 个变体、10 个步骤；替换增加 panic 前的 process 报告 |

共享生产功能另有 62 项、71 个变体、293 个步骤通过，取 [source-roles](validation/source-roles-darwin.json)、[dependency-roles](validation/dependency-roles-darwin.json)、[entry](validation/entry-darwin.json)、[discovery](validation/discovery-darwin.json) 中非 M43 fixture 的去重集合。它们覆盖源码选择、名称与依赖查找、构造决议、生产入口、退出、链接与产物消费。阶段数据与身份编码的 Rust 验证见 PROGRESS 中各功能提交的实际日志。

报告中的 complete=false 表示使用定向选择而非全仓 --all，不表示所选 fixture 尚未结束；每项是否通过以 results 的状态及进程结果为准。本轮没有重跑与改动无关的全仓 4050 项 fixture。

## 4. Linux 状态与收尾条件

环境为 nuc12，LLVM 22.1.2、Rust 1.99.0，GNU 与 musl 原生工具链。所有工具临时目录使用仓库 tmp/m43，验收输出使用 target/m43。

- 格式检查、workspace clippy、配套工具构建与 49 项 runner 测试已通过，日志见 PROGRESS 的 Linux 验证记录。
- 受影响 Rust 组共 1193 项通过，包含 385 项 Linux codegen、571 项 slib、21 项测试发现／入口，以及源码／依赖角色和 driver 输入检查。
- [初始 CLI 选择清单](validation/linux-selection.json)定义每目标 167 项范围，即 105 项 M43 加 62 项共享回归；166 项首批和 process 分开启动。两个平台首轮各有 imported-constructor-private-constructor 因并行 Cargo 重建正在使用的 scoopc 而失败。
- 固定 release 工具的 [GNU 验证报告](validation/release-check-gnu.json.gz)共 4 项、6 个变体、22 个步骤通过，包括该构造回归、scoop.std 选择与 process。同一冷缓存构造负例由 debug 工具的 259.62 秒降至 33.07 秒；据此结束较慢的首轮工具进程，保留已完成结果，将剩余项切换到独立的固定 release 工具。
- 首轮提前结束，没有生成整组 report.json；[GNU 原始日志](validation/debug-initial-gnu.log.gz)、[musl 原始日志](validation/debug-initial-musl.log.gz)分别保存 11 个完整 fixture 的 passed 记录。[GNU 摘录](validation/debug-observed-gnu.json.gz)和 [musl 摘录](validation/debug-observed-musl.json.gz)只提取这些已完成状态，不补写未完成项的通过结果。
- [剩余选择清单](validation/linux-remaining-selection.json.gz)已核对集合：GNU 的 14 项已完成证据与 153 项续验没有交集，并集为完整的 167 项；musl 的 11 项已完成证据与 156 项续验同样覆盖完整范围。
- GNU 153 项续验全部通过，共 180 个变体、491 个进程步骤，见 [报告](validation/remaining-gnu.json.gz) 和 [日志](validation/remaining-gnu.log.gz)。与首轮已完成日志和四项 release 验证按名称合并后，GNU 的全部 167 项均已验证，包括 105 项 M43 及 62 项共享回归；首轮受工具重建影响的构造回归已成功复验。
- musl 的 156 项续验仍在运行，待整组完成后再作平台结论。
- 必须取得两种 libc 剩余组的完成报告和所有失败项的成功复验，再与已完成日志／报告逐项合并，核对完整选择集合，才能把本文件、设计状态及路线图更新为完成。M43 完成后再推进 M44。
