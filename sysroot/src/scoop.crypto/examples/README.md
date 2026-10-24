# scoop.crypto 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [digests.scoop](digests.scoop) | 一次性和分块 SHA-256 摘要 |
| [hmac.scoop](hmac.scoop) | 系统随机密钥、HMAC 生成及消息校验 |

```sh
scoop run --release digests.scoop
scoop run --release hmac.scoop
```

摘要示例两次输出 `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`。HMAC 的首行随随机密钥改变，随后输出 `true` 和 `false`，分别表示原消息与修改后的消息是否匹配；SecretKey 在 `use` 结束时释放，示例不保存密钥。
