# Scoop LLM client 规范

本库通过普通 `scoop.llm.client` Cone 实现。总体职责、协议覆盖和实施顺序见 [架构设计](../llm-client/DESIGN.md)与[实施记录](../llm-client/PROGRESS.md)。本规范随功能闭环补充正式数据与 API 合同；尚未落地的调用示例仍以[使用草案](../llm-client/USAGE.md)标明状态。

## 1. 公共 token 用量

`TokenUsage` 是不可变的归一化计数快照。以下构造参数均为 `Long?`，缺省 None：

| 字段 | 含义 |
| --- | --- |
| inputTokens | 输入总量，包含已明确归入输入的缓存读写部分 |
| nonCachedInputTokens | 已明确报告或按完整、不重叠口径得到的非缓存输入量 |
| outputTokens | 输出总量，包含已归入输出的 reasoning 部分 |
| totalTokens | 明确报告或按已知口径得到的输入与输出总量 |
| cacheReadTokens | 输入缓存读取细项 |
| cacheWriteTokens | 输入缓存写入细项 |
| reasoningTokens | 输出中的 reasoning 细项 |

None 表示没有该计数，Some(0L) 表示已知为零；构造和序列化往返不能把两者混同。出现的计数必须非负，负值在构造时抛 IllegalArgumentException；使用 Long 保存完整整数，不经 Double 转换。

`modality: Array<ModalityTokenUsage> = []` 保存有明确归属的模态细项。每项包含 direction（TokenDirection.Input／Output）、modality（TokenModality.Text／Image／Audio／Video／Document／Other(name)）及非负 Long tokens。细项不是额外总量，不自动与 inputTokens／outputTokens 相加。媒体秒数、搜索次数和金额不放进 token 计数。

TokenUsage 不根据字段名猜测供应商原生口径，不自动求和、补齐总量或校验未经声明的包含关系。原生字段到这些计数的映射由相应 dialect 完成；重试与流式快照的归属由调用和协议 reducer 管理。此处的数据构造器不进行跨尝试汇总，也不实现价格／预算框架。

上述类型的 companion 显式声明 ValueEncoder／ValueDecoder，方法使用语言现有的缺省实现合成。TokenUsage 与 ModalityTokenUsage 按普通记录编码，enum 使用普通 enum codec 表示。缺省为 None 的字段按语言规则省略，解码时缺失的计数字段恢复为 None，缺失的 modality 恢复为空数组；因此空对象也是有效的未知用量。Some(0L) 必须保留。格式库负责数字及形状检查，普通构造器负责其计数范围；不新增编译器 intrinsic 或另建序列化层。
