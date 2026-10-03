# 重建 core 的名义声明与参数协议

三个原 fixture 由 `m23-cli-core-nominals` 消费。各用例复制完整标准 core 和
原源码，先生产未改动的声明，再在明确的工作副本中加入合法的观察函数，保存
两次 core 的 AST／HIR／MIR／LIR 与指纹。下游 single-file 仅通过私有 sysroot
消费已发布 core，移走源码后使用正式 artifact-only link，并运行普通和移动 GC。

| 用例 | 原源码 | 运行覆盖 |
| --- | --- | --- |
| `nominal-varargs` | `m23-type-source-nominals/parameter-varargs.scoop` | 泛型类构造、私有方法、泛型方法、默认数组、空／逐元素／spread／命名整数组参数、enum vararg payload |
| `dispatch-varargs` | `m23-type-source-dispatch/parameter-varargs.scoop` | protected 普通与泛型 vararg 方法、默认参数、公开构造和数组实参协议 |
| `storage-roots` | `m23-type-source-nominals/storage-roots.scoop` | 私有 enum、Option、compound storage 声明闭包、单例身份与状态 |

三项只读通过，共 18 次进程、39 份阶段／链接 golden、9 项产物指纹。每项保存
原 core 的独立产物副本，避免第二次构建覆盖其指纹证据。已有的 32 份阶段输出
在 core 默认 object 值修复前后逐字相同，新增部分补齐实际程序输出。

原 Rust 内部测试保留：两种 vararg 表面的实际 Array application／元素类型
关系和 6／4 个参数计数；storage 的完整七个源码声明集合、排除无关私有类型
与生成 backing class。原源码全部保留。历史内存测试将追加 AST 统一放在
`src/core.scoop`，当前 CLI 保留真实 `src/nominals.scoop` 来源；不要求两者的
源码位置身份字节相同。

单例默认导入缺口见 [core 值修复](CLI-CORE-VALUES.md)。本记录不代替 M23-11
完整仓库验收。
