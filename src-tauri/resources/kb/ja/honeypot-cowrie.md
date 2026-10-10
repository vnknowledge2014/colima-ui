# Cowrie による SSH ハニーポット

## これは何か

Cowrie は、弱い認証情報を持つ SSH サーバーを装います。誰かがログインすると、
偽のシェルを差し出します。コマンドは動いているように見え、ファイルシステムは
本物らしく見えますが、入力された内容はあなたのマシンには一切触れません。そして
セッションを記録します。試された認証情報、実行されたコマンド、取得されたファイル
のすべてを記録します。

Telnet も提供できますが、同梱の設定では無効になっているため、この記事では SSH
ポートだけを公開します。

ログがツールなしで読めるため、最初の一歩として最適です。

## 実行する前に

まだであれば **Colima 上のハニーポット** を先に読んでください。下の compose
ファイルは `127.0.0.1` にバインドしているため、到達できるのは自分のマシンだけ
です。ここを変更すると、意図的に脆弱な SSH サービスを、接続先のネットワークに
公開することになります。

Cowrie は root で動作せず、特権モードも必要としません。`privileged: true` を
追加するよう指示する手順書を読んでいるなら、それは別の何かの手順書です。

## 実行する

`cowrie-compose.yml` として保存します。

```yaml
services:
  cowrie:
    image: cowrie/cowrie:latest
    restart: unless-stopped
    ports:
      # ホスト側の 127.0.0.1 バインドが安全性そのものです。概要記事の警告を
      # 読まずに外さないでください。
      - "127.0.0.1:2222:2222"
    volumes:
      # Cowrie の作業ディレクトリは /cowrie ではなく /cowrie/cowrie-git です。
      # /cowrie/var にマウントすると、ハニーポットが書き込まない空のディレクトリ
      # ができるだけで、記録したセッションは次の `down -v` で消えます。
      - cowrie-var:/cowrie/cowrie-git/var
      - cowrie-etc:/cowrie/cowrie-git/etc

volumes:
  cowrie-var:
  cowrie-etc:
```

起動します。

```bash
docker compose -f cowrie-compose.yml up -d
```

## 自分でトラフィックを発生させる

自分で接続してみます。パスワードは何でも通ります。それが要点です。

```bash
ssh -p 2222 root@127.0.0.1
```

ホスト鍵を受け入れ、任意のパスワードを入力すると偽のシェルに入ります。`ls`、
`cat /etc/passwd`、`wget http://example.com/x` を試してください。どれもあなた
のシステムには触れません。終わったら `exit` と入力します。

## 何が見えるか

```bash
docker compose -f cowrie-compose.yml logs -f
```

各行が構造化されたイベントです。接続の開始、認証の試行、コマンドの実行、
セッションの終了。同じイベントはコンテナ内の
`/cowrie/cowrie-git/var/log/cowrie/cowrie.json` に JSON として記録されます。
運用を続けるなら解析対象にすべきはこちらです。

活用方法は **ハニーポットのログを読む** を参照してください。

## 停止と後片付け

```bash
docker compose -f cowrie-compose.yml down -v
```

`-v` は記録されたセッションも削除します。残したい場合は付けないでください。

## 関連

Colima 上のハニーポット · ハニーポットのログを読む
