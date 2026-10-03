# M23-11 原源码边界、构造器和装箱

这批用例保持原源文件不变，通过标准 Python 声明执行正式 CLI。四个负例继续要求原来接受的 HIR 错误，完整 JSON 额外固定 code、severity、Cone、logical source path、byte span 和 notes；空数组与 addressOf 保留精确表达式区间，两个 C ABI 负例保留原行列。没有把编译成功、其他错误或工具失败视作通过。

成功用例发布完整产物和四阶段 golden，移走源码后以独立 scoop 重链，在普通 GC 与 moving GC 下运行。program-read 还分别发布未修改的原程序和加结果断言的工作副本。read-basic 使用实际 core 源码加入已有 stage3_test.scoop 后正式生产，不通过 runner 拼接 AST 或伪造 core。

| 功能 | 用例／进程／golden／指纹 | 保留的行为与清理 |
| --- | --- | --- |
| empty array rejection | 1／2／0／1 | 空数组无法推导元素类型；原六个正例已在 CLI-NOMINAL-RUNTIME.md 验收。删去 Rust 中重复的文件诊断断言，保留 imported Array 身份测试。 |
| stored property address rejection | 1／2／0／1 | 普通存储属性不能因生成 accessor 变成 addressable global；保留 addressOf 原错误文字与 answer 的精确位置。 |
| native source boundary rejections | 2／4／0／2 | 普通同形 struct 不能冒充 handle，GcHandle? 不具备 C nullable-pointer ABI；删除重复的两行 Rust 诊断快照。 |
