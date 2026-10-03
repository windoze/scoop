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
