# 常见错误

ColimaUI 最常显示的那些消息的速查表。

## Permission denied

**现象。** *permission denied*、*operation not permitted*，或挂载的目录在容器
内看起来是空的。

**原因。** Colima 以你当前用户的所有权把宿主机目录挂载进虚拟机。以不同 UID
运行的容器无法写入；此外 macOS 在你授权之前不会把 Documents、Desktop 和
Downloads 暴露给虚拟机。

**解决。** 在 System Settings → Privacy & Security → Files and Folders 中授予
访问权限，然后重启实例。若是 UID 不匹配，用你自己的用户运行容器：

```bash
docker run --user "$(id -u):$(id -g)" -v "$PWD:/work" -w /work alpine sh
```

## Operation timed out

**现象。** *timed out*、*deadline exceeded*，或界面卡住后报超时。

**原因。** 虚拟机还活着，只是忙到无法应答 —— 通常是在拉取大镜像、构建，
或内存不足导致虚拟机开始使用交换分区。

**解决。** 等待当前操作完成，或查看是什么在消耗资源：

```bash
colima ssh -- top -b -n 1 | head -20
```

如果空闲时仍反复出现，说明虚拟机资源给少了 —— 参见
[性能调优](performance-tuning)。

## 网络故障

**现象。** *connection refused*、*could not resolve host*、*x509 certificate*。

**原因。** 虚拟机内的 DNS 与宿主机是分开的。宿主机所经过的企业 VPN 和代理
通常在虚拟机里看不到，拦截 TLS 的代理的 CA 也没有装进虚拟机。

**解决。** 在 Settings → Colima Config 中显式设置 DNS 服务器（例如
`1.1.1.1`），然后重启实例。从内部验证：

```bash
colima ssh -- nslookup registry-1.docker.io
```

## Port is already allocated

**现象。** 启动容器时出现 *bind: address already in use*。

**原因。** 宿主机上已有另一个进程占用该端口。Colima 会把发布的端口转发到宿主机，
因此宿主机侧的冲突同样生效。

**解决。**

```bash
lsof -nP -iTCP:8080 -sTCP:LISTEN
```

停掉那个进程，或改用其他宿主机端口发布。

## No space left on device

**现象。** 宿主机明明还有空间，构建和拉取却报 *no space left on device*。

**原因。** 虚拟机有自己固定大小的磁盘。它被填满与宿主机是否满无关。

**解决。** 先在虚拟机内部回收：

```bash
docker system df
docker system prune -a --volumes
```

如果仍然是满的，就在 Settings → Colima Config 中调大磁盘容量。磁盘只能变大，
不能缩小。

## 相关

- [启动 Colima 实例](start-colima)
- [性能调优](performance-tuning)
