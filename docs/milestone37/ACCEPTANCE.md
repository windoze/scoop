# M37 验收记录

> 原始日志、逐次 JSON 报告和压缩归档已移出版本库。本页保留验收范围、结果与限制；文中的历史报告名称仅标识当时的验证批次。

M37 已完成并验收。21 项功能分别实现并提交，Darwin、GNU、musl 三个目标的全部 680 个 M37 fixture 均通过普通比较；Linux 完整 Rust、GNU 正式 CLI 全量覆盖与 musl 受影响旧用例复验全部闭合。

基线为 `ec9d8849d`（M36），工作分支为 `codex/m37`。完整设计见 [DESIGN.md](DESIGN.md)，规范修订、实现路径及每项验证见 [PROGRESS.md](PROGRESS.md)。实现使用既有类型化身份、单态化、闭包、对象、GC、异常与链接机制，没有引入授权、防伪、通用资源预算或仅供假想需求使用的框架。

## 交付与提交

| 功能 | 主提交 |
| --- | --- |
| 五个完全忽略的兼容修饰符 | `a97511447` |
| 单语句控制体与 else-if | `b6470dff8` |
| Unicode 与反引号标识符 | `2c7df0635` |
| 循环标签和 break/continue 表达式 | `8818d0799` |
| do/while 与确定初始化 | `7dc13b0f9` |
| callable 局部返回标签 | `2d01c8b2f` |
| 具名表达式体函数的结果推导 | `48d5e12f1` |
| 非局部属性的类型推导 | `30e24f4b2` |
| 构造器与未绑定成员引用 | `31c8a9389` |
| receiver 函数类型、隐式 receiver 与 this 标签 | `18c0fa0d1` |
| 泛型、嵌套与局部 typealias | `4603e5395` |
| sealed 继承与 when 穷尽性 | `c9f3e9bc7` |
| interface by 实现委托 | `3d6720a30` |
| inner class 与外层实例捕获 | `208de05b0` |
| 局部类型、匿名 object 与 val 捕获 | `f5448131b` |
| 正常控制流出口的 smart cast | `7865b9941` |
| 整型枚举、整数父类型与标量 ABI | `b871c92c1` |
| 整型枚举、嵌套注解等静态参数值 | `5844d173a` |
| 注解声明目标与 use-site 选择 | `d124a4cf3` |
| Target 与 Repeatable 规则 | `dfef93683` |
| type-use 注解、alias 展开与产物传递 | `a99cfd2fd` |

五个兼容修饰符只解析后丢弃，不改变闭包、控制流或优化提示。interface 方法自身泛型、泛型虚方法和外层词法局部 `var` 捕获继续明确拒绝；可以通过捕获的 `val` 引用对象修改其普通成员。alias 不创建新类型身份，类型注解不影响类型相等、重载、布局或 ABI。non-local control flow 不在本次范围内。

新增依赖仅为 Unicode XID 识别所需的 `unicode-ident 1.0.26`；标识符按实际码点比较，不做 Unicode 规范化。已有静态注解规则使用真实 core 声明身份，普通同名声明不会取得特殊行为。

## do/while 的初始化规则

body 最外层局部绑定可以在 condition 中使用，但所有真正到达 condition 的路径都必须先完成被读取绑定的初始化。分析合并正常落空与匹配目标的 `continue`，并先应用 `finally` 的实际效果；已经转向 `break`、`return`、`throw` 的路径不参与条件入口合流。循环的上一轮值不能替代本轮初始化。

因此，在 `val value = ...` 之前执行 `continue`，而条件读取 `value`，会在条件的读取位置报编译错误。初始化表达式内部的 `?: continue`、外层标签跳转、函数值调用及捕获值同样受检查，不生成默认零值或未初始化读取。规则见[语言规范 8.7](../specs/SCOOP-SPEC.md)，实现位于 `compiler/hir-lower/src/stmt/flow/initialization.rs`，独立负例位于 `tests/fixtures/m37-do-while/`。

## 组合与验收回归

680 个新增 fixture 覆盖独立正例、精确诊断位置和信息的负例，以及 HIR/MIR/LIR golden。设计第 14 节的六条完整组合链由五个额外 fixture 闭合：

- 泛型 sealed 根、匿名开放子家族、when guard/smart cast 与 receiver lambda。
- do/while、标签和 Elvis continue、finally、Context 与实际挂起恢复。
- 泛型 alias、推导签名、构造器/成员引用，以及 provider → bridge → consumer、重导出、再次发布和移除源码后的独立链接。
- inner/局部委托类、按值捕获、老对象持有新对象与 moving/minor GC。
- Unicode/反引号名字、命名参数/尾随 lambda、整型枚举、嵌套/可重复注解和跨 Cone 类型使用位置。
- 整型枚举的父类型参数/返回、函数值适配、直接与上转后的不同装箱身份，以及独立 C compiler 编译的 Int16 callback。

这五个用例均执行 debug/release 与 normal/moving/minor 模式。连同既有整型枚举产物及默认参数回归，每个平台的 7 项普通比较均通过，12 个配置、73 个进程、71 个 golden，见 Darwin、GNU、musl。

验收中发现并单独修复了四类实际问题：完整泛型 companion 的绑定引用；捕获值及构造器参数经 subject 快照传播的 smart cast；函数 smart cast 的显式判型、转换和引用相等读取原存储值；导入整型枚举时沿整数父类型继续查找接口。函数视图的修复恢复了旧协程用例原有检查顺序，没有通过交换检查次序规避问题。相关新增 fixture 和定向 Rust 测试均已完成，详细来源见实施记录。

## 已完成的验证

每批 Rust 修改先 fmt/workspace clippy，再执行相关测试并构建三个配套 CLI；本机验证后在 `nuc12:~/repos/scoop` 验证 Linux。公共 fixture runner 两机各 44 项通过。临时脚本、测试日志和工作目录都保存在仓库的 `target` 下，构建和测试使用 `target/m37-tmp` 作为 TMPDIR。

Linux 完整 Rust 覆盖为 43 个测试目标、5,553 项不同测试通过，0 failed、0 ignored。初跑的八条旧断言在按现行语言和 wire 规则迁移后完成定向补验；随后回归修复继续使用相关 Rust 测试和正式 CLI，没有重复耗时的 driver 82 项完整组合。记录、原始日志和逐项来源见 Linux Rust 验证。

最终普通 CLI 覆盖如下。统计按 fixture 的最后一次普通比较去重，快照生成不计为普通通过；Darwin 使用各功能开发验收和受影响旧用例选集，未声称在最后一次提交上重跑整个工作区。

| 目标及范围 | 通过／不适用 | 配置 | 进程 | 阶段／计划快照 |
| --- | --- | --- | --- | --- |
| Darwin 开发与受影响回归 | 1,233／2 | 1,486 | 5,329 | 5,481 |
| GNU 正式 CLI 全量覆盖 | 3,741／30 | 4,203 | 16,425 | 13,747 |
| musl 全部 M37 与受影响回归 | 1,227／0 | 1,481 | 5,342 | 5,485 |

M37 的 GNU/musl 逐项记录见 M37 覆盖。GNU 使用正式 `--all` 选择开始，接续未完成项并定向复验失败项，当前 3,771 个 fixture 均有最终普通结果，0 失败、0 遗漏，见 GNU 全量覆盖。Darwin 覆盖 保留逐功能与补验来源。musl 针对实际受影响的阶段输出、ABI、链接和运行路径选择；最后一组 253 个旧用例全部普通比较通过，见 musl 覆盖、232 项主体回归与21 项原生链接回归。

## 快照与产物兼容性

旧快照按真实变化迁移：类型化 loop/default target、receiver 形状、整型枚举表示、注解值/规则/类型使用位置，以及新增 core 声明。ImportedIdentityId 和局部表索引变化按定义与全部引用核对；静态形状以完整块比较，保留字段、参数、构造器与接口信息。总是终止的函数推导为 `Nothing` 时，新增的普通函数值适配和捕获扫描按对应 HIR/MIR/LIR 核对。

存在真实 ABI 差异的 LIR 按目标保存。Darwin/GNU 的比较只在审阅时投影目标规定的寄存器拆分，仓库内保留实际 AArch64/x86-64 输出。公共 core 符号迁移只用于 GNU 对应文件恰好增加已检查 core 符号多重集的情况；各平台使用独立观测的新增集合，保留所有原有平台符号和重复次数。额外局部符号或函数结果变化仍检查实际目标输出，符号迁移本身不计为运行通过。

cross-cone-interface 从 72 升为 76，旧产物需要重建；MIR/LIR 与 runtime ABI 保持不变。没有为了验收在生产路径添加重复语义重放、权限状态或资源计费。

## 文件长度与工作目录

新增 182 个 Rust 文件，最长 289 行；新增生产文件最长 269 行。已有超过 500 行文件的最大净增长为 55 行，具体记录见 文件长度检查。模块按职责切分，新增编译器/runtime/core 代码未引入 TODO 或 unimplemented 占位。

测试过程中持续清理通过用例的 cases、过期上传副本与编译中间文件，保留报告、当前工具及可复用缓存，见清理记录。Linux 实际验证源码与本机的 5,507 个编译器、runtime、sysroot 及 Cargo 文件逐字节一致，见源码一致性记录。用户原有 M38 路线图增补和 `docs/milestone38/` 草稿保留在本地工作区中。
