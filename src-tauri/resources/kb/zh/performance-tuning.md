# 性能调优

## 现象

构建缓慢、文件修改要过几秒才在容器内出现，或者虚拟机运行时整台机器都变卡。

## 原因

几乎全部原因都落在三个设置上，而默认值偏保守，因为它们必须能在所支持的最低配
机器上启动：

- **VM type。** `qemu` 是模拟；`vz` 使用 Apple 原生的 Virtualization
  框架。在 Apple Silicon 上 `vz` 明显更快。
- **Mount type。** `sshfs` 把每一次文件操作都经由 SSH 隧道传输。
  `virtiofs` 是原生共享文件系统，对 Node、PHP 这类大量使用绑定挂载的工作负载
  来说是收益最大的一项改动。
- **规格。** 2 个 CPU 加 2 GiB 足够启动虚拟机，但不足以在里面做构建。

## 解决方法

在 Apple Silicon 上，切换 VM type 和 mount type。这不会重建任何东西，只是一次
重启：

```bash
colima stop
colima start --vm-type vz --mount-type virtiofs
```

然后给它足够的空间。经验法则是宿主机一半的核心和一半的内存：

```bash
colima stop
colima start --cpu 4 --memory 8 --disk 100
```

两者都可以在 Settings → Colima Config 中修改，那里会写入 `colima.yaml`
并在应用前展示差异。改动在下次重启时生效。

最后，定期回收空间 —— 磁盘写满时的表现和机器变慢一模一样：

```bash
docker system df
docker system prune -a --volumes
```

## 不要这样做

- 不要把所有核心都给虚拟机。它会与宿主机调度器争抢，导致整机更慢。
- 不要在只跑容器的 profile 上开着 Kubernetes；它会长期占用约 1 GiB 内存。
- 不要因为想着以后再改就把磁盘设得过大。磁盘只能扩大，不能缩小。

## 相关

- [常见错误](common-errors)
- [启动 Colima 实例](start-colima)
