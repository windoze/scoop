# M23-11 类型、形状与 runtime 布局的 CLI 验收

原源码保持不变，以原 coordinate 和 logical source path 通过公开 CLI 生成完整 AST／HIR／MIR／LIR，再移走源码供下游消费。需要访问 internal 构造器或字段时，先完成原始发布，再仅在临时工作副本加入同 Cone 的公开工厂或读取函数。完整阶段 golden、产物指纹、独立产物链接计划以及普通／moving GC 运行共同锁定行为。

| 功能 | 用例 | 只读进程／golden／指纹 | 覆盖和旧 infra 清理 |
| --- | --- | --- | --- |
| 隐式异常布局 | `runtime-layout-gates` 六例 | 36／54／18 | 成功／失败转换、实际 ClassCastException、除零 ArithmeticException、默认实参启用与显式实参抑制；六份原 MIR 逐字相同，删除旧文件编排模块 |
