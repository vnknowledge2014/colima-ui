# Docker CLI のインストール

## 症状

Colima は起動しているのに ColimaUI が `docker: command not found` を表示する、
または Compose 機能が *docker compose is unavailable* とともに無効化される。

## 原因

Colima が提供するのは VM 内部の Docker **デーモン**です。それと通信する
`docker` **クライアント**は提供しません — 別パッケージです。さらに Docker
Compose v2 は別のプラグインなので、素の `docker` は動くのに Compose だけが
無いという状態が起こります。

## 対処

```bash
brew install docker docker-compose
docker version
docker compose version
```

Docker Desktop は**不要**です。むしろ Colima と併せて Docker Desktop を入れる
ことが、ここで最も多い混乱の原因になります。独自のデーモンと独自のコンテキスト
を登録してしまうためです。

`docker` が入っているのにコマンドが失敗する場合、クライアントの向き先が違い
ます。Colima は起動時に `colima` という名前のコンテキストを登録します:

```bash
docker context ls
docker context use colima
docker ps
```

## 関連

- [Colima インスタンスの起動](start-colima)
- [よくあるエラー](common-errors)
