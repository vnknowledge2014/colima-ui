# 安装 Docker CLI

## 现象

Colima 正在运行，但 ColimaUI 提示 `docker: command not found`，或者 Compose
功能变灰并显示 *docker compose is unavailable*。

## 原因

Colima 在虚拟机内部提供 Docker **守护进程**，但不提供与之通信的 `docker`
**客户端** —— 那是一个独立的软件包。Docker Compose v2 又是另一个独立插件，
所以会出现普通 `docker` 可用而 Compose 缺失的情况。

## 解决方法

```bash
brew install docker docker-compose
docker version
docker compose version
```

你**不需要** Docker Desktop。事实上，把它和 Colima 一起装才是这里最常见的困惑
来源，因为它会注册自己的守护进程和自己的 context。

如果 `docker` 已安装但命令仍然失败，说明客户端指向了错误的守护进程。Colima
启动时会注册一个名为 `colima` 的 context：

```bash
docker context ls
docker context use colima
docker ps
```

## 相关

- [启动 Colima 实例](start-colima)
- [常见错误](common-errors)
