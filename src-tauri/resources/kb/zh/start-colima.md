# 启动 Colima 实例

## 现象

出现 *Cannot connect to the Docker daemon*、*Colima is not running* 或
*the connection to the server was refused* 之类的错误。工具明明已安装，
但容器、镜像和 Kubernetes 页面都是空的。

## 原因

Colima 已安装，但它的虚拟机处于停止状态。本应用中所有 Docker 和 Kubernetes
命令都与该虚拟机**内部**的守护进程通信，虚拟机停止时就没有任何进程在监听。
重启电脑后这是正常状态 —— Colima 不会自动启动。

## 解决方法

```bash
colima start

# or, for a named profile
colima start --profile dev
```

或者在 Instances 页面点击 **Start**。某个 profile 的首次启动需要一到两分钟，
因为要下载并初始化磁盘镜像；之后的启动只需几秒。

如果启动失败，虚拟机自己的日志会说明原因：

```bash
colima status
colima start --verbose
tail -n 100 ~/.colima/default/daemon/daemon.log
```

有两种值得记住的失败：

- **宿主机磁盘空间不足。** 磁盘镜像是预先分配的。请清理空间，或在
  Settings → Colima Config 中调小磁盘容量。
- **上次启动被中断留下的损坏虚拟机。** 重新创建即可 —— 这会删除虚拟机内的容器
  和镜像，但不会影响你的源代码或宿主机上的挂载目录：

  ```bash
  colima stop
colima delete
colima start
  ```

## 相关

- [常见错误](common-errors)
- [性能调优](performance-tuning)
