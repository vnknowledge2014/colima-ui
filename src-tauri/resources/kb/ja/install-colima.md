# Colima のインストール

## 症状

ColimaUI が `colima: command not found` を表示する、または Setup ページで
Colima が **Missing** と表示される。Instances ページに何も読み込まれない。

## 原因

Colima は独立したコマンドラインツールです。ColimaUI はそのフロントエンドに
すぎず、VM ランタイムを同梱していません。したがって Colima が `PATH` 上に
存在しなければ何も動作しません。また Finder や Dock からアプリを起動すると、
シェルよりも短い `PATH` が渡されるため、標準外の場所にインストールされた
Colima はターミナルで動いてもアプリからは見えないことがあります。

## 対処

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

バイナリが応答するか確認します:

```bash
colima version
colima status
```

ターミナルで `colima version` が動くのに ColimaUI が Missing と表示する場合、
Colima はアプリが検索するディレクトリの外にあります。`/usr/local/bin` へ
シンボリックリンクを張ってアプリを再起動してください:

```bash
sudo ln -s "$(which colima)" /usr/local/bin/colima
```

## 関連

- [Colima インスタンスの起動](start-colima)
- [Docker CLI のインストール](install-docker-cli)
