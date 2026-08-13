# OpenCanary によるおとりサービス

## これは何か

OpenCanary は、FTP、HTTP、SSH バナーなど一連の偽サービスを動かします。それらは
触れてきた相手を記録する以外に何もしません。Cowrie が攻撃者を招き入れて観察する
のに対し、OpenCanary は応答して「誰かがノックした」と書き留めるだけです。

この違いは重要です。実際に検知目的で配置するなら OpenCanary の方です。静かで、
軽く、実際に触れられるまでほとんど出力を出さないからです。

## 実行する前に

まず **Colima 上のハニーポット** を読んでください。企業ネットワークに関する警告
はここで最も強く当てはまります。社内ネットワーク上の偽 FTP サーバーは、その
ネットワークの管理者から見れば、許可されていないサービスそのものに見えます。

下の compose ファイルは `127.0.0.1` にバインドしています。実運用では変更する
ことになりますが、それこそが概要記事が「意識的に行ってください」と求めている
決断です。

## 実行する

OpenCanary は、どのサービスを有効にするかを指定する設定ファイルを必要とします。
`opencanary.conf` として保存します。

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

`opencanary-compose.yml` として保存します。

```yaml
services:
  opencanary:
    image: thinkst/opencanary:latest
    restart: unless-stopped
    ports:
      # ホスト側の 127.0.0.1 バインドが安全性そのものです。
      - "127.0.0.1:2121:21"
      - "127.0.0.1:8080:80"
    volumes:
      - ./opencanary.conf:/root/.opencanary.conf:ro
```

両方のファイルは同じディレクトリに置き、そのディレクトリは Colima が VM と共有
しているものである必要があります。ホームディレクトリは共有されますが、ホスト側の
`/tmp` は共有されません。VM から `opencanary.conf` が見えない場合、Docker は
ファイルをマウントする代わりに同名の*ディレクトリ*を作ります。すると OpenCanary
は設定を見つけられず、サービスを一つも起動しないまま、`restart: unless-stopped`
によって静かに再起動を繰り返します。症状は「正常に見えるのに何もログを出さない
コンテナ」です。

起動します。

```bash
docker compose -f opencanary-compose.yml up -d
```

## 自分でトラフィックを発生させる

```bash
curl -s http://127.0.0.1:8080/ >/dev/null
```

このリクエスト一つが「ヒット」です。他の何もヒットを生むべきではありません。

## 何が見えるか

```bash
docker compose -f opencanary-compose.yml logs -f
```

静かな状態が続き、ヒットごとに送信元アドレス、触れられたサービス、タイムスタンプ
を含む JSON が一行記録されます。この静けさこそが機能です。このログに現れるもの
は何であれ注意に値します。あなたが持つ他のログのほとんどには当てはまらない性質
です。

上の設定は二つのサービスによる出発点です。OpenCanary はさらに多くをサポートし
ます。基本の流れが動くのを確認してから追加してください。

## 停止と後片付け

```bash
docker compose -f opencanary-compose.yml down
```

## 関連

Colima 上のハニーポット · Cowrie による SSH ハニーポット · ハニーポットのログを読む
