## 运行

两个文件保存到同一工作目录。终端一先启动服务并等待 Listening 提示：

```sh
scoop run --release server.scoop
```

终端二执行客户端：

```sh
scoop run --release client.scoop -- http://127.0.0.1:8080/
```

完成后在终端一按 Ctrl+C 停止服务。若 8080 被占用，可将 server 的 HttpServer.listen 调用加上 `port = 9080`，并同步修改客户端 URL。

## 预期结果

程序 stdout：

```text
200
Hello, Scoop!
```

上述为客户端输出。服务端打印 Listening 地址后保持运行；构建诊断不属于程序 stdout。

## 关键说明

默认 URL 指向本机回环服务，没有外网或 TLS 证书依赖。普通 main 仍需显式进入事件循环。withHttpClient 管理 client 生命周期，withResponse 在请求作用域退出时关闭响应。

requireSuccess 检查状态；body.text(1_048_576L) 完整读取正文，并明确设置 1 MiB 上限。修改 URL 或读取上限不应移除响应作用域。正文按 API 合同只消费一次，不能先 text 再期待重新读取同一流。

服务端默认只监听本机回环地址；awaitTermination 保持服务运行。这个演示只展示正常 GET；Ctrl+C 是手工停止方式，不代表应用已经实现自己的优雅关闭协议。

## 常见误用

先等服务开始监听，再运行客户端；不要把连接拒绝误认为 JSON 或 URL 解析问题。HTTP 与 HTTPS 是不同协议，修改为 HTTPS 时要保留目标域名与证书信任的正常校验。
