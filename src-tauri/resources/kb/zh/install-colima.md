# 安装 Colima

## 现象

ColimaUI 提示 `colima: command not found`，或者 Setup 页面将 Colima 显示为
**Missing**。Instances 页面没有任何内容。

## 原因

Colima 是一个独立的命令行工具。ColimaUI 只是它的前端，并不内置 VM 运行时，
所以必须先让 Colima 出现在 `PATH` 中。此外，从 Finder 或 Dock 启动应用时，
应用拿到的 `PATH` 比 shell 中的更短，因此安装在非标准目录下的 Colima 即使在
终端里可用，对应用来说仍然是不可见的。

## 解决方法

**macOS (Homebrew):**

```bash
brew install colima docker docker-compose
```

**Linux:**

```bash
# Debian/Ubuntu
sudo apt install colima docker.io docker-compose-v2

# Arch
sudo pacman -S colima docker docker-compose
```

然后确认二进制文件可以响应：

```bash
colima version
colima status
```

如果在终端里 `colima version` 正常，而 ColimaUI 仍显示 Missing，说明 Colima
不在应用搜索的目录中。将它软链接到 `/usr/local/bin` 并重启应用：

```bash
sudo ln -s "$(which colima)" /usr/local/bin/colima
```

## 相关

- [启动 Colima 实例](start-colima)
- [安装 Docker CLI](install-docker-cli)
