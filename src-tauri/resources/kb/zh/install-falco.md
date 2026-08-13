# 为 Colima 安装 Falco

Falco 是 CNCF 的运行时安全引擎。它用 eBPF 监控系统调用，当某个行为匹配它的规则时
发出告警——比如容器内打开了 shell、敏感文件被读取、出现了意料之外的外连。

ColimaUI 本身不做任何检测。它读取 Falco 报告的内容，并显示在你熟悉的 compose 服务
名旁边。你的检测覆盖范围，完全等于你的 Falco 规则所覆盖的范围。

## 开始前的两个坑

**1. `brew install falco` 装的是另一个软件。**
有一个同名的无关工具 `falco`——Fastly 的 VCL 解析器和检查器，而 Homebrew 的 `falco`
formula 给你的正是它。它与运行时安全毫无关系。ColimaUI 会识别出这种情况并明确说明，
而不是假装 Falco 已经就位。

**2. Falco 不能在 macOS 上运行。**
它需要 Linux 内核。在 Mac 上，它运行在 *Colima 的虚拟机内部*，而不是宿主机上，所以
下面所有命令都通过 `colima ssh` 执行。

## 安装

Falco 同时发布 arm64 和 x86_64 的软件包。在 Apple Silicon 的 Mac 上，虚拟机内大约
十秒即可完成。

```sh
colima ssh
sudo bash
curl -fsSL https://falco.org/repo/falcosecurity-packages.asc \
  | gpg --dearmor -o /usr/share/keyrings/falco-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/falco-archive-keyring.gpg] https://download.falco.org/packages/deb stable main" \
  > /etc/apt/sources.list.d/falcosecurity.list
apt-get update -y
FALCO_FRONTEND=noninteractive FALCO_DRIVER_CHOICE=modern_ebpf apt-get install -y falco
```

驱动选 `modern_ebpf`：不需要内核头文件，不需要编译模块，而 Colima 所用的内核（6.8）
原生支持它。

## 验证它真的在检测

这一步常被跳过，但它才是最关键的。

Falco 可能安装干净、eBPF 驱动加载成功、systemd 服务运行健康——**却什么都没检测到**，
因为它一条规则都没加载。它的任何状态输出都不会告诉你这一点。一个正在运行却什么也
看不见的安全工具，比一个根本没装的更危险，因为你以为自己受到了保护。

检查规则数量：

```sh
sudo falco -L -o json_output=true | python3 -c \
  'import sys,json; print(len(json.load(sys.stdin)["rules"]), "条规则已加载")'
```

标准安装大约加载 25 条规则。**如果输出为 0，说明 Falco 什么都没在检测。** 常见原因
是安装时禁用了规则拉取（`falcoctl`），导致引擎以空规则集运行，尽管
`/etc/falco/falco_rules.yaml` 就在磁盘上。显式指定该文件：

```sh
sudo falco -r /etc/falco/falco_rules.yaml
```

ColimaUI 会把这种状态显示为独立的警告，而不是「就绪」。所以如果 Runtime 标签页提示
Falco 正在运行但没有规则，指的就是这件事。

## 让 ColimaUI 能读到事件

Falco 出厂时文件输出是**关闭的**，因此默认情况下 ColimaUI 无内容可读。启用 JSON
文件输出：

```sh
sudo tee /etc/falco/config.d/colimaui.yaml <<'YAML'
json_output: true
json_include_output_property: true
file_output:
  enabled: true
  keep_alive: false
  filename: /var/log/falco/events.json
YAML
sudo mkdir -p /var/log/falco
sudo systemctl restart falco-modern-bpf.service
```

然后在 ColimaUI 中打开 **安全 → Runtime**，点击 *开始读取*。

## 端到端确认

触发一条默认规则集中的规则：

```sh
docker run --rm alpine cat /etc/shadow
```

几秒内，Runtime 标签页应出现一条 *Read sensitive file untrusted* 事件，并标注它来自
哪个容器。

## Apple Silicon 上的正常噪声

在 arm64 上，Falco 每次启动都会打印：

```
libbpf: failed to determine tracepoint 'syscalls/sys_enter_open' perf event ID
libpman: failure while attaching TOCTOU mitigation program for 'open'
```

这是无害的。arm64 上不存在 `open` 和 `creat` 这两个系统调用，它使用 `openat`，因此
只有针对这两者的 TOCTOU 缓解不可用。检测本身工作正常，可用上面的 `/etc/shadow`
测试验证。

## 它不能给你什么

Falco 检测的是其规则所描述的行为。它不是 rootkit 扫描器，它不会阻断任何东西，而且
一套没人调优过的规则会对正常工作误报——安装软件包同样会读取敏感文件。请把事件当作
调查的起点，而不是结论。
