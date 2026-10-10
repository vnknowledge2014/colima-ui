# 使用 OpenCanary 的诱饵服务

## 这是什么

OpenCanary 运行一组假服务——FTP、HTTP、SSH banner 等等——它们除了记录下谁碰过它
们之外什么也不做。Cowrie 是把攻击者请进来然后观察，而 OpenCanary 只是应个门，然
后记下有人敲过。

这个区别很重要：真要部署来做检测，用的是 OpenCanary，因为它安静、开销小，而且在
真正被触碰之前几乎不产生任何输出。

## 运行之前

先读 **Colima 上的蜜罐**。关于企业网络的警告在这里最为适用——公司网络上的一台假
FTP 服务器，在网络管理员眼里，看起来就是一个未经授权的服务。

下面的 compose 文件绑定到 `127.0.0.1`。真实部署时你会改掉它，而这正是概述文章要
求你有意识做出的那个决定。

## 运行它

OpenCanary 需要一个配置文件来说明启用哪些服务。保存为 `opencanary.conf`：

```json
{
  "device.node_id": "colima-canary",
  "ftp.enabled": true,
  "ftp.port": 21,
  "ftp.banner": "FTP server ready",
  "http.enabled": true,
  "http.port": 80,
  "http.banner": "Apache/2.2.22 (Ubuntu)",
  "http.skin": "nasLogin",
  "logger": {
    "class": "PyLogger",
    "kwargs": {
      "handlers": {
        "console": { "class": "logging.StreamHandler", "stream": "ext://sys.stdout" }
      }
    }
  }
}
```

保存为 `opencanary-compose.yml`：

```yaml
services:
  opencanary:
    image: thinkst/opencanary:latest
    restart: unless-stopped
    ports:
      # 主机侧绑定 127.0.0.1 正是这里的安全属性。
      - "127.0.0.1:2121:21"
      - "127.0.0.1:8080:80"
    volumes:
      - ./opencanary.conf:/root/.opencanary.conf:ro
```

两个文件必须放在同一目录下，而且该目录必须是 Colima 共享给虚拟机的目录——你的
home 目录是，宿主机的 `/tmp` 不是。如果虚拟机看不到 `opencanary.conf`，Docker 会
创建一个同名的*目录*，而不是挂载你的文件。此时 OpenCanary 找不到配置，一个服务
也不会启动，而 `restart: unless-stopped` 会悄悄地让它无限重启。症状是：容器看起
来很健康，却从不输出任何日志。

启动：

```bash
docker compose -f opencanary-compose.yml up -d
```

## 自己制造一些流量

```bash
curl -s http://127.0.0.1:8080/ >/dev/null
```

这一个请求就是一次"命中"。除此之外不该有任何东西产生命中。

## 你会看到什么

```bash
docker compose -f opencanary-compose.yml logs -f
```

先是安静，然后每次命中输出一行 JSON，包含来源地址、被触碰的服务和时间戳。这份安
静就是它的功能——这个日志里出现的任何东西都值得你注意，而这一点对你手上大多数日
志都不成立。

上面的配置是一个只启用两个服务的起点。OpenCanary 支持的远不止这些；等你看到基本
流程跑通之后再添加。

## 停止与清理

```bash
docker compose -f opencanary-compose.yml down
```

## 相关

Colima 上的蜜罐 · 使用 Cowrie 的 SSH 蜜罐 · 读懂蜜罐日志
