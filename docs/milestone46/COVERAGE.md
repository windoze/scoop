# M46 离线基线覆盖记录

本记录按 [BASELINES.md](BASELINES.md) 的形状矩阵关联普通 Scoop 用例。它不是完整 API client 或新 manifest 入口。生产库只提供协议、Serialize 合成和 JsonValue；基线 DTO 与业务检查留在测试中。三组 API 尚待实施，不能计为已验收。

## Cone 等价 JSON

用例：[m46-baseline-cone](../../tests/fixtures/m46-baselines/cone/fixture.toml)。完整配置及最小配置位于 [data.scoop](../../tests/fixtures/m46-baselines/cone/program/src/data.scoop)，运行断言位于 [main.scoop](../../tests/fixtures/m46-baselines/cone/program/src/main.scoop)，67 个错误输入及期望路径位于 [errors.scoop](../../tests/fixtures/m46-baselines/cone/program/src/errors.scoop)。

| 基线 | 实现及观察 |
| --- | --- |
| T1 | Manifest／Coordinate 的普通合成读取必需字段；strict wrapper 检查未知字段、null；schema 和 kind 由普通代码验证 |
| T2 | Dependency 的字符串／详细对象分支，Dependencies 的动态字符串 key；version、path／artifact 及互斥关系保留，错误类型和未知成员拒绝 |
| T3 | sources 的 None 与 Some(empty array) 独立保存；Selection 的 path／when 使用同一 typed codec |
| T4 | TestConfig 的 default 与空 sources；test-dependencies 与生产依赖使用相同数据模型，重复 coordinate 在测试字段路径报错 |
| T5 | Values 保存字符串或数组分支；Predicate 的 os／arch／env 按 AND／OR 匹配，空数组不匹配任何值；未知键、值与坏元素均拒绝 |
| T6 | Native 的 sources／include／两组 flags／cxx／libraries 使用普通 default；输出后重新 typed 解码保持结果 |
| T7 | Library 覆盖 default／dynamic／static／framework，name／kind／when 有各自检查；平台条件复用 Predicate |
| T8 | 普通校验函数检查 coordinate／SemVer、相对路径、locator 冲突、重复依赖、库名与 native 参数；保持原始 argv 数据，不访问文件系统或执行 native 编译／链接 |
| T9 | 每个固定表先拒绝已出现的 null，随后复用合成字段 codec；缺失用 default／None，显式空数组保持 Some(empty)，不把 null 当成省略 |

字段包装器先枚举并检查该表的输入，再将同一 decoder 交给普通合成 reader。这样直接验证稳定视图与 default 的组合，无需复制合成实现或使用 JsonValue 保存整份 manifest。文本样例直接表达 spec 的等价 JSON，没有模拟 TOML 文本替换、源码 span 或构建器。
