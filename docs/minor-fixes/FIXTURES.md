# Fixture 分类与迁移记录

基线为 `4de3ae9e8ac3f18bc63eba4947afd8bb87ea34db` 的 4,532 个 fixture。使用正式 discovery 检查载体、输入归属和期望引用；新加的 shebang/runner、GC 开关 fixture 单独计数。逐用例清单（原名称、位置、输入、步骤、变体、断言与分类）保存在本次任务的仓库外工作目录，Git 保留原文件。这里只记录迁移规则和必须保留的覆盖，避免提交原始报告或运行期注册表。

## 职责划分

| 归属 | 基线数量 | 保留的事实 | 模式安排 |
| --- | ---: | --- | --- |
| language | 3,314 | 各语言规则的诊断、求值结果、源码与依赖组合 | release；去掉仅重复运行结果的 debug/stress |
| libraries | 534 | 原基线含 API/consumer；清理后只保留必要外部协议、native 和命令入口边界 | release；普通 API、错误路径和 consumer 断言迁入 Cone 的 `scoop.test` |
| compiler-correctness | 326 | IR 转换、优化、ABI、产物读写、ODR 和必要性能回归 | 按被测转换保留已有 profile/跨产物组合，不把 stress 当通用模式 |
| driver | 294 | CLI、缓存、输入选择、测试命令、文档和安装布局 | release；默认 profile、优化切换/缓存键的专测保留其明确参数 |
| gc-correctness | 51 | 分配、根、屏障、搬迁、回收、safepoint 的真实正确性 | 按场景保留 normal/minor/moving、线程及必要优化组合；显式编入 stress 支持 |
| regressions | 13 | 有明确故障形态的语言/代码生成回归 | release；需要优化或 GC 矩阵的回归分别归入对应专测 |

分类不是根据原有 `gc`、`moving-gc` 标签直接得出。很多普通泛型、重导出、序列化和终端用例都带这些标签，只比较普通输出；它们不会因此成为 GC 专测。共享 `gc.c` helper 的存在也不自动构成 GC 合同，须看被调用的探针和断言。

基线 51 个 GC 专测是模式清理的初始保留集合；P6 逐条检查库断言时补入 hash/linked/queue/tree 的 4 个 gcStats 回收场景，P7 再补入深栈/异常展开的两个 Linux 场景，当前共 57 项。其他用例若发现仍断言实际搬迁、根或回收行为，应先拆分/修订归属，再调整模式，不能改期望来掩盖遗漏。分类阶段不删除源码、步骤、断言或变体。

## GC 覆盖对应关系

| 旧用例组或具体场景 | 功能位置 | 保留原因 |
| --- | --- | --- |
| `m31-gc` | gc-correctness/allocation | nursery 压力、范围复制与写屏障 |
| `m34-allocation`、`m34-parallel`、`m34-poll` | gc-correctness/allocation、parallel、poll | TLAB、并行回收和线程 safepoint |
| `m34-reclamation`、`m34-regions`、`m58-runtime` | gc-correctness/reclamation、regions、nursery | region 回收、pin、nursery 复用、大对象 remembered set、release sweep |
| `m33-atomics/gc`、`m34-containers/storage`、`m34-maybe-uninit/gc`、`m44-gc-writes` | gc-correctness/barriers | 引用写入、atomic/容器/未初始化存储的根与屏障 |
| `m44-moving-references`、`m44-no-move` | gc-correctness/references | 搬迁、pin、GC-free payload 以及不应搬迁的路径 |
| `m32-roots/gc-context`、旧 `m15-moving`、协程跨 suspension、初始化 stress、binding/copy-update 的 suspend-moving 场景 | gc-correctness/roots | 栈、聚合值、handle、上下文与挂起点的 live roots |
| core nominal storage roots、source nominal roots、native runtime GC、Scoop ABI、TLS/callback threads | gc-correctness/native-roots | 跨 Cone 与 native 边界的实际根保活 |
| `m45-file-gc`、`m45-native-buffer-gc`、`m45-thread-gc` | gc-correctness/native-release | 外部资源在 GC/release 下的生命周期 |
| `m56-runtime` 中 cards-moving、gc-update、loops-moving、strings-moving；`m58-poll` | gc-correctness/codegen | 屏障、指针更新及优化后的循环/字符串/poll 路径 |
| statepoint-taildup、constant-dynamic-root、coroutine-unwind-liveness、gc-branch-loads、gc-guarded-unwrap、nonduplicable-safepoint | gc-correctness/regressions | 保留具体曾出错的优化与根存活场景，不按里程碑排列 |

## 编译器矩阵对应关系

优化组包括现有 `m31-optimization`、`m34` 的 inlining/devirtualization/small-values，以及 `m58` 的 optimization/codegen/coroutines。保留能验证不同优化输出或跨优化产物消费的组合；相同程序的普通输出不因此在全部库用例中重复验证。

IR 组包括必要的 HIR publication、MIR production、materialized selections、layout/type foundation、call emission、final MIR、intrinsics、nominal/native result 转换。普通语言用例中的完整阶段快照不因旧目录名称而获得保留资格。

ABI/产物组保留 native C/C++、native link、ODR、generic publication/reader、runtime image/layout 和 program read 的实质断言。缓存与工具选择归 driver；其中显式观察 profile 改变、缓存失效或默认 profile 的步骤不批量改成 release。

## 内置库迁移顺序

1. `scoop.cli`：参数、选项、默认值与诊断 API 的纯行为；保留进程参数转发和正式 CLI 边界。
2. `scoop.json`：值模型、解析/输出、编解码组合；把 annotation/静态形状错误归入语言规则；只保留必要的产物与 native 边界。
3. `scoop.std`：可直接迁移的断言加入既有测试，普通消费者一并迁移；测试框架计数/过滤用例改用小型固定 Cone，避免绑定整个 std 的测试总数。
4. `scoop.test`：框架自测不添加自身依赖。core 的公开行为用独立消费者 Cone 测试，避免 core/test 依赖环。
5. 其他内置库已有 `scoop.test` 时沿用其组织；仅在实际覆盖对应后删除重复 fixture。网络、PTY、进程和文件系统用例也先判断 scoop.test 能否完成，仅保留确实依赖外部控制的边界。

每组迁移记录原 fixture → 新测试方法/保留边界，先通过 release 测试再删除重复载体。package 迁移引起的位置和类型名称变化记录为源码变化，不全量重录快照。

## 目录搬迁规则

P8 使用 `language/`、`libraries/`、`compiler-correctness/`、`driver/`、`gc-correctness/`、`regressions/` 表达职责，下面按功能组织。共享源码与 companion 跟随功能归属；混合目录按 fixture 职责拆分。

旧 `mNN` 前缀从活动用例名称、tags 与目录中移除。同名功能来自不同里程碑时按具体场景合并或细分，不用里程碑号消歧。更新实际相对路径、file 操作、source fixture 引用、活动脚本和文档入口；历史设计文档的里程碑标题不改写。

搬迁后运行正式 discovery 和 runner 单测，核对每个原用例仍有迁移去向、每个 `.scoop` 仍有输入归属。数量变化应来自明确的库测试合并或变体清理，不能把找不到文件当成测试减少。

P6 已迁移的 111 个场景与保留的边界见 [内置库测试迁移](LIBRARY-TESTS.md)。

## P8 当前目录

4,435 个现有用例已迁入六类功能目录：language 3,303、libraries 423、driver 297、compiler-correctness 341、gc-correctness 58、regressions 13。基线 4,532 项加 14 项新覆盖、减去已迁入库测试的 111 项，数量闭合；模式去重只减少步骤/变体，不减少场景。完整验收另将未优化返回路径 MIR 和跨 profile 缓存记录两个用例补入编译器正确性组，原共享输入继续保留于原功能目录。

同名的 annotations、floating、modifiers 和 poll 分别按 scalar/array、arithmetic/IEEE、declaration/inline 和 thread/codegen 细分；泛型、函数、构造器等按具体语义与产物消费区分。共享源码只保留一份，跨目录用例更新显式引用。测试资料中的 Cone 坐标和 native 符号是被测输入，保持原值，不因去掉里程碑目录而改变实体身份。

P8 的目录搬迁提交为 `5e5f5c414`，其前置提交为 `e5a397876`，Git 可追溯每个旧文件；本次任务的仓库外移动清单逐项对应新旧名称和路径。该搬迁提交保持源码正文的字节不变，并同步活动 Rust/Python 引用、runner 说明和 fixtures 首页；历史里程碑设计记录保留当时路径。P9 发现的 package/import、期望身份和平台路径修复分别提交，见 [验收记录](PROGRESS.md)。4,435 项 discovery 和输入归属检查通过；目录入口见 [fixtures 首页](../../tests/fixtures/README.md)。

## 补充清理后的当前集合

当前 4,200 项：language 3,415、libraries 71、driver 297、compiler-correctness 345、gc-correctness 59、regressions 13。相对于 P8 的 4,435 项，合并 77 个重复普通编译错误，迁出 154 个 consumer 场景，并删除 4 个已有 Cone 测试覆盖的终端重复用例；目录重新分类不改变总数。113 个内建诊断归入语言/编译器专测，3 个 native/ABI 场景归入编译器专测，1 个列表回收场景归入 GC 专测。具体覆盖见 [库测试迁移](LIBRARY-TESTS.md)。

fixture 默认直接使用准备好的 sysroot；共享 cache 独立于本次 work-dir。累计去掉 334 个 fixture 中的 369 次重复系统库构建和 132 次无关 warm-build 步骤，并移除与库行为无关的冷构建次数断言。仅改变输入、构建/发布、缓存行为等专测有理由隔离对应资源。21 个修改 core 的编译器专测必须重新构建依赖它的 std，声明中写明原因；它们仍使用共享缓存。原始工作目录、日志和报告不进入仓库。

缓存复查还将两个 HTTP/2 边界和七个 GC/回归用例改为共享 cache，删除其中五次无关的 warm build 和冷编译计数断言。九项本机验证全部通过（13 个变体、38 个进程步骤）；原有 ABI/GC 模式及运行断言不变。
