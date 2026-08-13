# 使用 Cowrie 的 SSH 蜜罐

## 这是什么

Cowrie 伪装成一台使用弱口令的 SSH 服务器。当有人登录后，它会给对方一个假的
shell：命令看起来能执行，文件系统看起来很真实，而对方输入的任何内容都不会碰到你
的机器。它会记录整个会话——尝试过的每一组凭据、执行过的每一条命令、下载过的每一
个文件。

它也能提供 Telnet，但镜像自带的配置里是关闭的，所以本文只发布 SSH 端口。

它是最好的起点，因为它的日志不需要任何工具就能读懂。

## 运行之前

如果还没读过，请先读 **Colima 上的蜜罐**。下面的 compose 文件绑定到
`127.0.0.1`，因此只有你自己的机器能访问到它。改动这一点，就等于把一个刻意做得
脆弱的 SSH 服务暴露给你所接入的网络。

Cowrie 不以 root 运行，也不需要特权模式。如果某份指南让你加上
`privileged: true`，那你读的是另一个东西的指南。

## 运行它

保存为 `cowrie-compose.yml`：

```yaml
services:
  cowrie:
    image: cowrie/cowrie:latest
    restart: unless-stopped
    ports:
      # 主机侧绑定 127.0.0.1 正是这里的安全属性。在读完概述文章的警告之前，
      # 不要去掉它。
      - "127.0.0.1:2222:2222"
    volumes:
      # Cowrie 的工作目录是 /cowrie/cowrie-git，不是 /cowrie。挂载到 /cowrie/var
      # 只会创建一个蜜罐从不写入的空目录，已记录的会话会在下一次 `down -v` 时
      # 全部消失。
      - cowrie-var:/cowrie/cowrie-git/var
      - cowrie-etc:/cowrie/cowrie-git/etc

volumes:
  cowrie-var:
  cowrie-etc:
```

启动：

```bash
docker compose -f cowrie-compose.yml up -d
```

## 自己制造一些流量

自己连上去。任何密码都能通过——这正是重点：

```bash
ssh -p 2222 root@127.0.0.1
```

接受主机密钥，输入任意密码，你就进入了那个假 shell。试试 `ls`、
`cat /etc/passwd`、`wget http://example.com/x`。这些都碰不到你的系统。完成后输入
`exit`。

## 你会看到什么

```bash
docker compose -f cowrie-compose.yml logs -f
```

每一行都是一条结构化事件：建立连接、尝试凭据、执行命令、会话结束。同样的事件会
以 JSON 形式写入容器内的 `/cowrie/cowrie-git/var/log/cowrie/cowrie.json`。如果
你打算长期运行，那才是值得解析的格式。

拿到日志之后该做什么，见 **读懂蜜罐日志**。

## 停止与清理

```bash
docker compose -f cowrie-compose.yml down -v
```

`-v` 会连同已记录的会话一起删除。想保留就不要加它。

## 相关

Colima 上的蜜罐 · 读懂蜜罐日志
