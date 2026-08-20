# M23-11 真实泛型版本冲突

conflict 与 strings-conflict 均使用原始 provider-left、provider-right 和 consumer 源码。两套 provider 使用同一 Cone coordinate，各自发布完整产物并由自己的 consumer 与 program 消费：整型版本分别得到 41、42，字符串版本分别得到 left value、next value；四个程序均在普通 GC 和 moving GC 下通过。

每套图在移走源码后由独立 scoop 重链，保留 core、provider、consumer、program 的产物指纹和所有 AST/HIR/MIR/LIR、link plan golden。随后将右侧 provider 的已发布产物复制到左侧可写 locator，左侧 consumer 的原依赖记录保持不变。正式 link 在 manifest:direct_dependencies 给出精确 StaleDependency，完整诊断固定 code、phase、severity、provider、记录／实际 HIR/MIR/LIR 指纹和产物位置；不会生成目标程序。

这里实际触发的是已有依赖语义指纹检查。底层相同 ODR group/member/ABI 但正文不同的冲突，以及字符串 immortal object/registration 冲突，继续由原 Rust typed 测试验证 Conflict 与 DuplicateArtifact；不把高层 StaleDependency 误写成已穿过该边界后的 ODR 合并错误。

只读验收：2 个用例、28 个进程、52 份 golden、14 个产物指纹，全部通过。原 actual_duplicate_generic 两项 typed 测试、workspace Clippy、格式检查通过。记录位于 `/tmp/scoop-m23-11-final-graphs/verified-odr-locator/report.json`。这批只增加 schema 数据和配套程序，没有修改 runner 或产物兼容规则。
