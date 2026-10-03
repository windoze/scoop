# M23-11 类型、形状与 runtime 布局的 CLI 验收

原源码保持不变，以原 coordinate 和 logical source path 通过公开 CLI 生成完整 AST／HIR／MIR／LIR，再移走源码供下游消费。需要访问 internal 构造器或字段时，先完成原始发布，再仅在临时工作副本加入同 Cone 的公开工厂或读取函数。完整阶段 golden、产物指纹、独立产物链接计划以及普通／moving GC 运行共同锁定行为。

| 功能 | 用例 | 只读进程／golden／指纹 | 覆盖和旧 infra 清理 |
| --- | --- | --- | --- |
| 隐式异常布局 | `runtime-layout-gates` 六例 | 36／54／18 | 成功／失败转换、实际 ClassCastException、除零 ArithmeticException、默认实参启用与显式实参抑制；六份原 MIR 逐字相同，删除旧文件编排模块 |
| 名义形状物化 | `shape-materialization` 两例 | 13／22／7 | 值、class、enum、接口默认属性、alias、装箱；保留实际产物的 1／4 个 root、1／2 个 box 及六种支持角色断言，三阶段重复编译收敛为一次生产读取 |

形状物化快照审阅：两例原 Export HIR 全文相同，新 HIR 同时显示 LocalConcrete 与 CrossCone。原 31 个 MIR 函数正文在统一函数 ID 后相同，新增 Payload／Combined／Choice 的既有派生 equality；LIR 只增加对应函数与更新函数引用编号。原逐 root 产物断言继续验证 layout、scan、descriptor、registration、coroutine step／slot，并将目标缺失改为明确失败。
| 有限 MIR helper | `finite-mir-types` 两例 | 11／22／5 | 原 core identity 与 src/finite-types.scoop 路径，真实 core 重建与单文件下游；9／10 个指定形状 helper、字段／变体身份、GC、私有 box 排除和 wire 拒绝断言保留 |
