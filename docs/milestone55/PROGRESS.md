# M55 实施进度

## M55-1：负向接口条件与条件重载

2026-10-19 完成本机功能闭环：函数级 `!Interface<...>`、三态排除关系、约束转发与条件重载、规范化声明身份、完整 bound 的跨 Cone 导出和导入。负向事实不参与猜测类型实参；普通泛型正文保持定义处选择。重复和正负矛盾提供关联位置。

正式 CLI 覆盖 28 个 fixture：两个本地运行组合、25 个错误规则、一组 provider／facade／consumer 的跨 Cone 运行。组合包括静态基类、完整接口实参、kind、局部函数、泛型成员、扩展、默认值、函数引用及同类型递归。没有保存普通程序的 IR 快照。

本机验证通过：workspace 格式化与 all-target Clippy；parser 506 项、identity 351 项、HIR 862 项（含增量新增测试）、HIR lowering 回归 1,479 项与随后新增递归测试、slib 250 项。推断修复后定向重跑新特性 5 项、调用推断 34 项、既有递归检查 3 项；正式 CLI 28 项均通过。

已闭合的问题：空条件字段曾改变无条件声明身份，改为省略空集合；格式版本与固定向量同步升级；同类型递归曾被推断成 `Any`，现复用已物化的相同类型界；跨 Cone 测试的 class 构造器已显式声明 public。identity schema 为 2、metadata outer schema 为 3；runtime ABI 不变。

原始日志及运行目录保存在仓库外。Linux 环境已在 `nuc12:~/repos/scoop-m55-validation` 建立目录，完整验证留在本机实现收尾之后。M55-2／3 与里程碑最终验收尚未完成。

## M55-2：companion 投影与 JSON 默认入口

2026-10-19 完成本机功能闭环。函数自身参数可声明 `T.Companion` 的存在、正向和负向条件；投影用于签名、默认值和正文，并沿既有 bound 成员调用及 singleton 初始化路径具体化。宿主确定后，companion 的正向关系继续提供普通类型推断；投影不反向猜测宿主。JSON 新增分别要求 ValueEncoder／ValueDecoder 的单参数入口，显式 codec 重载保留。

本机格式化与 all-target Clippy 通过；受影响 crate 回归为 parser 506、identity 351、HIR 863、HIR lowering 1,483、slib 250 项。正式 CLI 共 21 个 fixture 已通过：两个本地组合、17 个错误规则、一组 provider／facade／consumer、一个 JSON 实际消费者。JSON 同时覆盖两方向独立实现、派生 DTO、命名 companion、显式替换 codec、泛型转发和强制 GC 移动；跨 Cone 覆盖消费方隐藏宿主、泛型宿主、默认投影值、成员泛型与函数引用。

已闭合的问题：开放投影具体化时保留依赖泛型 singleton 模板；跨 Cone 签名查询按 typed host 查到实际所属模块及必要的隐藏声明；wire 未知 tag 测试避开新增的投影 tag。签名投影、独立 companion binder 与默认 singleton 投影并入 M55 的整体格式迁移，最终格式／兼容性检查在 M55-4 收尾。runtime ABI 不变。

开发 sysroot 的 12 个 Cone 已显式重新准备。清理约 15 GiB 的 debug incremental 缓存，保留可运行工具；日志与工作目录均在仓库外。M55-3、声明重建优化及 Linux 完整验收继续进行。

## 逐 callable 声明重建优化

按本次附加需求，独立优化 codegen 的 LLVM 临时声明。单个 callable module 只声明自身、局部 typed call targets、函数取址目标、callback trampoline，以及 foreign callback 注册所需的 adapter／descriptor；直接借用外层已经选定的函数，删除每个 module 对整个 callable 目录的声明与正文扫描。非 callable metadata 的完整声明表面保持，未改变 LIR、对象划分、ABI 或 wire。

格式化、all-target Clippy 与 codegen 234 项测试通过；新回归检查独立函数、递归、重复调用与取址的精确声明集合。正式 CLI 定向回归 67 个 fixture／71 个变体／104 个进程通过，含全部 M55-1／2、函数指针、managed callback、foreign continuation、初始化与 GC 移动组合。三种规模的新旧 `.slib` 均逐字节一致。

同机单轮前后冷构建测量如下（秒）。编译器自身为 debug 构建，输入为 release library，源码、工具和旧产物均保留，使用独立空缓存／target；测量期间未并行运行其他构建。它不是发布工具的绝对性能承诺，也不是多轮中位数。

| 公开 NoGC 小函数数 | 优化前 | 优化后 | 前／后峰值 RSS MiB |
| --- | ---: | ---: | ---: |
| 100 | 8.78 | 8.26 | 501／514 |
| 1000 | 19.45 | 12.17 | 1230／1294 |
| 5000 | 253.60 | 64.96 | 12745／14103 |

5000 函数耗时降低约 74%，真实缓存命中仍为约 7～12 ms。峰值内存未改善，不把全部 RSS 归因于声明，也不声称解决了其余规模成本。原始数据、工具副本和生成输入保存在仓库外；Linux 完整合同验证随 M55 最终收尾进行。
