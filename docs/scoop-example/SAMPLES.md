# 三个主题的实际输出入口

日期：2026-10-27。原编辑样稿已落实到发布内容；此处保留三个代表主题的入口，不再维护第二份源码与正文。

| 主题 | 说明与源码 | 查看完整输出 |
| --- | --- | --- |
| language/option | [正文](../../sysroot/share/scoop/examples/topics/language/option.md)、[程序](../../sysroot/share/scoop/examples/programs/language/option/main.scoop) | `scoop example language/option` |
| json/typed | [正文](../../sysroot/share/scoop/examples/topics/json/typed.md)、[程序](../../sysroot/src/scoop.json/examples/typed.scoop) | `scoop example json/typed` |
| http/get | [正文](../../sysroot/share/scoop/examples/topics/http/get.md)、[客户端](../../sysroot/src/scoop.http.client/examples/get.scoop)、[服务端](../../sysroot/src/scoop.http.server/examples/hello.scoop) | `scoop example http/get` |

默认输出依次包括标题、配套版本、摘要、运行条件、全部文件、运行方式、预期结果、关键说明、常见误用和相关主题。HTTP 页面包含两个完整文件与两个终端的操作顺序。

## 原始源码

```sh
scoop example language/option --format source
scoop example http/get --format source --file client.scoop
scoop example http/get --format source --file server.scoop
```

单文件直接输出原文，多文件显式选择显示路径。输出没有 Markdown 围栏或附加说明。

## 结构化内容

```sh
scoop example json/typed --format json
```

主题 JSON 包含 `schema = 1`、`kind = "topic"`、`version`、`id/title/summary`、`requirements`、`files`、`body_markdown` 和 `related`。每个 file 给出显示路径 path、安装根内的 source、language 和完整 text。相关主题包含 ID、标题和摘要。搜索和目录同样支持 JSON。

三种形式均读取同一份 catalog、正文与源码，独立测试核对每一个发布文件的原文一致性。实际程序验证见 [VALIDATION.md](VALIDATION.md)。
