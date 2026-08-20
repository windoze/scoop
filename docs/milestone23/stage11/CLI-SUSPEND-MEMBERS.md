# M23-11 suspend 成员的发布与消费

正式 CLI 迁移暴露了两个实际缺口：自动物化把 suspend effect 当作跳过 owner 和接口槽的条件；隐式协程失败构造器收集没有覆盖 MethodCall／DirectSuperMethodCall。闭合 suspend 成员现在沿普通声明、物化和 callable 发布路径进入产物，成员调用也在原 executable 遍历中选择 IllegalStateException 初始化入口。未绑定泛型仍等待实际 application，没有修改协程 ABI、产物格式或增加平行导出表。

| 用例 | 实际覆盖 | 只读进程／golden／固定产物指纹 |
| --- | --- | --- |
| `suspend-members-cross-cone` | 接口默认 suspend 方法、protected suspend 成员、虚调用覆盖、下游 SuspendTask／Continuation，结果分别为 20 和 23 | 6／9／3 |
| `source-dispatch-callables` | 原 callable 源码完整发布，unsafe／infix／operator／protected 成员组合，实际跨 Cone 协程返回 29L | 7／13／4 |
| `source-dispatch-protected-callables` | 原 protected 签名完整发布，泛型／默认参数／访问器／abstract 实现与 suspend 调用组合，实际协程返回 1L | 7／13／4 |

三个用例均先独立编译 core 和 provider，再移走源码供下游消费；保留 AST／HIR／MIR／LIR、完整链接计划及固定产物指纹。独立链接使用没有相邻 scoopc 的 scoop 副本，PATH 与 LLVM locator 不可用于编译；普通 GC 和 moving GC 均运行成功。组合用例先编译原始源码，再在临时工作副本内加入合法 owner 观察入口，原源码不变。

已有 source-only 六例只读复验为 18 个进程、48 份完整 golden。shape-demand 新增 DeferredSuspend；initialization-demand 新增 DeferredRegistry 及其原有初始化路径。后者的 consumer HIR 只改变导入 arena ID。原初始化身份关系、完整根集合以及 projected／decoded／LocalConcrete 一致性断言保留；删除两份只用于摘要文本比较的旧快照。实际发布产物的 shape support closure 计数见 [source-only 记录](CLI-SOURCE-ONLY.md)。

内部 AST builder 单测的 11 份 MIR 快照经逐函数审阅后同步：统一函数 ID 后，全部原有函数正文保持不变，只新增完整 suspend 协议所需的 startCoroutine helper 及对应槽记录。这些内部单元测试不属于文件 fixture runner，继续保留。HIR 1294 项、MIR 114 项、slib 587 项单元测试及 workspace lint 通过。

首轮无筛选 CLI 回归发现 43 项既有用例仍记录修复前的 suspend 发布结果，现同步 148 份阶段与链接计划预期。56 份 HIR 的 Export 部分全部相同；50 个 nominal 记录只在 methods 列表补入同一模块中已有的 suspend 函数，原成员全部保留且没有重复。原 LocalConcrete 记录不变，新增记录来自闭合物化。MIR／LIR 逐个按持久身份及符号匹配，原函数均保留且正文相同；LoopId、导入身份、字符串池、闭包和函数类型 arena 的重排按实际身份、池内容或一致双射对应，包含控制流引用。

`core-layouts-shared-units-standalone` 新增 SharedUnitDeferred 的初始化与 ensure 路径，三个源码初始化单元现在均有实际产物；原 284 个 MIR、285 个 LIR 函数不变，分别新增 16 个函数，相应更新 core 的完整产物指纹。七个 `coroutines-*` provider 的原 73 个 MIR／LIR 函数不变，分别新增 26 个函数，覆盖 DerivedTask、DeferredDerivedTask、Gate／Registration 的 DerivedResult 实例及协程 helper。十二份链接计划只把 provider 的对象数从 74 改为 100，其他内容保持相同。

七个协程用例仍逐行比较完整 `symbols.txt` 和 `strong_symbols.txt`：每份保留全部原符号，分别增加 1005 个符号和 426 个强符号，对应实际发布的机器定义。源码、运行结果、诊断和 native 输入保持不变，未移除符号检查或放宽产物指纹比较。受控观察用于审阅预期；验收继续使用无更新开关的正式 runner。

上述 43 项正式只读回归全部通过：61 个变体、220 个进程、321 次阶段与链接计划 golden 比较。报告位于 `/tmp/scoop-m23-11-final-repairs/suspend/verified/report.json`；这是专项复验，全仓完成状态以最终无筛选验收为准。
