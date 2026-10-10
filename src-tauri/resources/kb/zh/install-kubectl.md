# 安装 kubectl 并指向 Colima

## 现象

Kubernetes 页面为空，或提示 `kubectl: command not found`，或
*The connection to the server localhost:8080 was refused*。

## 原因

需要两样彼此独立的东西，缺一不可：

1. 宿主机上的 `kubectl` 二进制文件。
2. Colima 虚拟机内部的 Kubernetes 集群。Colima 默认**不会**启动它 ——
   必须按 profile 单独启用。

其中 `localhost:8080` 这条消息说明 kubectl 本身运行正常，只是没有配置任何
context，于是回退到了内置的默认地址。

## 解决方法

安装客户端：

```bash
brew install kubectl
kubectl version --client
```

然后在虚拟机中启用 Kubernetes 并选择它的 context：

```bash
colima start --kubernetes
kubectl config get-contexts
kubectl config use-context colima
kubectl get nodes
```

你也可以在 Settings → Colima Config 中启用 Kubernetes。该操作会把设置写入
`colima.yaml`，在实例下次重启时生效。

启用 Kubernetes 大约会多占用 1 GiB 内存并使启动多花约一分钟，所以只跑容器的
profile 建议保持关闭。

## 相关

- [启动 Colima 实例](start-colima)
- [性能调优](performance-tuning)
