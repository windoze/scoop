# M23-11 fixture 迁移记录

本记录按已迁移批次更新。历史单文件覆盖与剩余 driver／stage／program-link 文件测试分别计数，不能用 Rust workspace 总通过数代替 Python 总验收。

## 1. 历史端到端用例

原 `compiler/driver/tests/fixtures.rs` 已在 `c02bd245c` 退役，留下的合并快照此前不再被执行。此次从原已接受快照和 runner 的 moving 清单恢复全部条件，统一调用正式 `scoop build`，从同一次 root 编译取得四个 stage dump，再执行同次产物。

| 原范围 | 源码数 | 当前覆盖 |
| --- | ---: | --- |
| M1–M25 合并 fixture | 480 | 186 个编译成功（含 trap）、294 个 negative |
| M0 smoke | 1 | 正常运行，精确检查原始 stdout |
| M23 source headers | 4 | 1 个正常运行、3 个 negative |
| 合计 | 485 | 178 个正常运行、10 个 SIGABRT、297 个 negative |

逐源码对应见 [LEGACY-COVERAGE.md](LEGACY-COVERAGE.md)。467 份新增声明位于 `tests/fixtures/legacy-acceptance/`；18 份源码复用先前修复中已经建立的 CLI 条件，其中重复声明错误等在同一用例中有独立命名步骤。发现和覆盖检查均来自 TOML 与源码扫描，Python 中没有历史 case 注册表。

新增的 467 个用例在只读验收中全部通过，共 497 个变体、737 次进程执行和 824 次 stage golden 比较。485 份旧合并快照已删除，原源码全部保留。完整声明中保留 649 条 error、2 条 note 和 2 条 warning。

原 35 份 moving-GC 用例全部保留。native companion 的每份 C 源、逻辑库名、SDK、target、deployment、编译和归档参数都在对应 TOML 中明确给出；CLI 仅消费 `--library-path` 下满足真实 typed requirement 的对象／归档。普通运行与 moving 运行使用独立工作目录并比较相同输出。

### 1.1 快照结构

旧合并 `.scoop.snap` 中拼接的 core AST 不再是 single-file 编译输入。当前 AST 只包含原单文件；HIR 分别记录 Export 与 LocalConcrete，MIR／LIR 使用当前完整 typed identity 和独立 core 依赖。四阶段期望分别保存，既不删除类型身份，也不按用例清理差异。运行／诊断期望与可更新的 stage golden 分开，更新 golden 不会接受新的失败类别。

negative 由完整 JSON 数组检查 severity、code、message、canonical cone/source/span 和每条 note；warning 同样精确检查。原以行列呈现的位置对应当前规范的 byte span，显示路径不参与 canonical 期望。两份原 warning（safe-call 的 `inner`、enum 模式的 `Raedy`）保留原消息和位置。

### 1.2 已接受的历史差异

以下差异均逐项对照原快照审核。263 个 negative 的消息、主位置和 note 与旧期望相同；其余 34 个保留原拒绝规则，并按当前共有前端与入口协议记录具体变化。

| 原源码 | 原因与保留的断言 |
| --- | --- |
| [m1-hello/errors/no-main.scoop](../../../tests/fixtures/m1-hello/errors/no-main.scoop) | 正式 executable 入口检查要求唯一 ordinary main(): Unit。 |
| [m1-hello/errors/print-arity.scoop](../../../tests/fixtures/m1-hello/errors/print-arity.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m10-coroutines/errors/suspend-main.scoop](../../../tests/fixtures/m10-coroutines/errors/suspend-main.scoop) | 入口错误附独立 source note，说明 main 为 suspend。 |
| [m10-coroutines/suspend-variance-bridge.scoop](../../../tests/fixtures/m10-coroutines/suspend-variance-bridge.scoop) | 保留原不合法赋值，后续未绑定变量诊断归入 core 重载候选。 |
| [m12-ffi/errors/no-gc-operations.scoop](../../../tests/fixtures/m12-ffi/errors/no-gc-operations.scoop) | 真实依赖 receiver 临时值和 managed 依赖调用沿同一 NoGC 检查。 |
| [m12-ffi/errors/sizeof-requires-type.scoop](../../../tests/fixtures/m12-ffi/errors/sizeof-requires-type.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/c-unsafe-signature.scoop](../../../tests/fixtures/m13-callback/errors/c-unsafe-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/closure-signature.scoop](../../../tests/fixtures/m13-callback/errors/closure-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/context-index.scoop](../../../tests/fixtures/m13-callback/errors/context-index.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/generic-signature.scoop](../../../tests/fixtures/m13-callback/errors/generic-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/missing-signature.scoop](../../../tests/fixtures/m13-callback/errors/missing-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/safe-context.scoop](../../../tests/fixtures/m13-callback/errors/safe-context.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m13-callback/errors/suspend-signature.scoop](../../../tests/fixtures/m13-callback/errors/suspend-signature.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m14-generics/errors/intrinsic-types.scoop](../../../tests/fixtures/m14-generics/errors/intrinsic-types.scoop) | core intrinsic 类型在共有模型中没有可访问源码构造器。 |
| [m14-generics/errors/operator-equals.scoop](../../../tests/fixtures/m14-generics/errors/operator-equals.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m2-values/errors/string-plus-int.scoop](../../../tests/fixtures/m2-values/errors/string-plus-int.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m21-delegate-roles/errors/role-contracts.scoop](../../../tests/fixtures/m21-delegate-roles/errors/role-contracts.scoop) | 先拒绝 delegate operator 的非法默认值，不再追加该默认 lambda 的级联类型错误。 |
| [m22-binding-patterns/errors/class-component-ambiguity.scoop](../../../tests/fixtures/m22-binding-patterns/errors/class-component-ambiguity.scoop) | 保留两个歧义候选，使用现行 MSC 判定措辞。 |
| [m22-integers/errors/array-index-requires-long.scoop](../../../tests/fixtures/m22-integers/errors/array-index-requires-long.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/callback-context-index-requires-long.scoop](../../../tests/fixtures/m22-integers/errors/callback-context-index-requires-long.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/mixed-signedness.scoop](../../../tests/fixtures/m22-integers/errors/mixed-signedness.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/mixed-width.scoop](../../../tests/fixtures/m22-integers/errors/mixed-width.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-integers/errors/shift-count-requires-long.scoop](../../../tests/fixtures/m22-integers/errors/shift-count-requires-long.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m22-iteration/errors/protocol-resolution.scoop](../../../tests/fixtures/m22-iteration/errors/protocol-resolution.scoop) | 无适用项报告最近的失败候选；歧义仍列出全部并列候选及 MSC 原因。 |
| [m22-ranges/errors/contracts.scoop](../../../tests/fixtures/m22-ranges/errors/contracts.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m23-source-headers/errors/public-current-unit.scoop](../../../tests/fixtures/m23-source-headers/errors/public-current-unit.scoop) | 诊断定位 current-unit 标记，从整条声明起点收紧到第 15 列。 |
| [m3-generics/errors/conflicting-type-args.scoop](../../../tests/fixtures/m3-generics/errors/conflicting-type-args.scoop) | 共同约束求解顺序调整，冲突为 String/Int，位置为第 18 列。 |
| [m3-generics/errors/none-no-context.scoop](../../../tests/fixtures/m3-generics/errors/none-no-context.scoop) | 共有 Option 变体报告缺少预期 enum 类型，规则不变。 |
| [m3-generics/errors/print-generic.scoop](../../../tests/fixtures/m3-generics/errors/print-generic.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m5-arrays/errors/conversion-method-arity.scoop](../../../tests/fixtures/m5-arrays/errors/conversion-method-arity.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m5-arrays/errors/index-not-int.scoop](../../../tests/fixtures/m5-arrays/errors/index-not-int.scoop) | 独立 core／依赖候选使用共同诊断；保留原失败规则与实际参数类型。 |
| [m6-classes/errors/generic-interface-call.scoop](../../../tests/fixtures/m6-classes/errors/generic-interface-call.scoop) | 不合法泛型接口方法未进入继承表，显式记录后续 override 无对应成员错误。 |
| [m7-overload/errors/duplicate-return-only.scoop](../../../tests/fixtures/m7-overload/errors/duplicate-return-only.scoop) | 保留重复声明错误，删除由 Error 表达式再触发的 println 级联错误。 |
| [m7-overload/errors/duplicate-signature.scoop](../../../tests/fixtures/m7-overload/errors/duplicate-signature.scoop) | 保留重复声明错误，删除由 Error 表达式再触发的 println 级联错误。 |

M0 的源程序使用 `print`，真实 stdout 为 `hello, world`，末尾没有 LF；旧 insta 文件自身的结尾 LF 不等于程序输出。10 个原 trap 仍要求 SIGABRT、空 stdout 和完整 stderr，类型名按现行 canonical TD 名字保存。其中 `m21-delegated-globals/failure`、`m21-initialization/runtime-cycle` 还保留当前 runtime 给出的实际初始化单元上下文。没有用短类型名归一化掩盖 TD 身份，也没有把合法程序失败改成 negative。

### 1.3 迁移发现并修复的编译器缺口

| 能力 | 修复与回归 |
| --- | --- |
| imported enum 派生相等 | 复用定义 Cone 的真实 helper，覆盖本地、依赖与 re-export |
| 拒绝调用的诊断 | Error 表达式不再制造额外 println 级联；ForeignCallback 按真实 typed identity 拒绝源码构造 |
| shared shape | 源码需求先闭合，再生成协议；补泛型 receiver、companion 及实际协程结果类型 |
| SourceLocation | 默认值、泛型、lambda 和协程保留定义／求值位置及 canonical source |
| 整数操作 | 跨 core 调用保留实际失败参数类型与明确转换提示，不重复 lowering |
| 多接口 application | 每张实际接口表独立保存 slot selection 与选中 receiver，继承默认实现不丢失 |
| raw global／TLS | GC-free 存储不进入 managed registration，正确发布 Mach-O TLS descriptor／初值及 bootstrap 引用 |
| Scoop ABI native roots | C 使用真实源码类型描述符；正常 GC 与强制移动分别验证可观察契约 |

## 2. 后续批次

parser／HIR／MIR／LIR 的文件加载与 golden 编排、driver end-to-end、program-link/native 以及需要外部构建的 runtime 测试仍按原断言逐批迁移。直接构造内存 typed IR 的内部单元测试保留。完成每批后补充对应关系、删除的 helper 与实际验证结果；全部完成前不将 M23-11 标记完成。
