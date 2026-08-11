# よくあるエラー

ColimaUI が最も頻繁に表示するメッセージの早見表です。

## Permission denied

**症状。** *permission denied*、*operation not permitted*、あるいはマウントした
ディレクトリがコンテナ内で空に見える。

**原因。** Colima はホストのディレクトリを、あなたのユーザーの所有権で VM に
マウントします。別の UID で動くコンテナは書き込めません。さらに macOS は許可を
与えるまで Documents / Desktop / Downloads を VM に見せません。

**対処。** System Settings → Privacy & Security → Files and Folders で許可を
与え、インスタンスを再起動します。UID の不一致には、自分のユーザーで
コンテナを実行します:

```bash
docker run --user "$(id -u):$(id -g)" -v "$PWD:/work" -w /work alpine sh
```

## Operation timed out

**症状。** *timed out*、*deadline exceeded*、または UI が固まってからタイム
アウトを報告する。

**原因。** VM は生きているが応答できないほど忙しい状態です。大きなイメージの
取得、ビルド、あるいはメモリ不足でスワップに入っているのが典型です。

**対処。** 実行中の処理を待つか、何が消費しているか確認します:

```bash
colima ssh -- top -b -n 1 | head -20
```

アイドル時にも再発するなら VM のサイズ不足です —
[パフォーマンス調整](performance-tuning) を参照してください。

## ネットワークの失敗

**症状。** *connection refused*、*could not resolve host*、*x509 certificate*。

**原因。** VM 内の DNS はホストの DNS とは別です。ホストが経由している社内 VPN
やプロキシは VM からは見えないことが多く、TLS 傍受プロキシの CA も VM 内には
インストールされていません。

**対処。** Settings → Colima Config で DNS サーバーを明示的に指定し
(例: `1.1.1.1`)、インスタンスを再起動します。内部から検証します:

```bash
colima ssh -- nslookup registry-1.docker.io
```

## Port is already allocated

**症状。** コンテナ起動時に *bind: address already in use*。

**原因。** ホスト上の別プロセスが既にそのポートを保持しています。Colima は
公開ポートをホストへ転送するため、ホスト側の衝突がそのまま影響します。

**対処。**

```bash
lsof -nP -iTCP:8080 -sTCP:LISTEN
```

そのプロセスを停止するか、別のホストポートで公開してください。

## No space left on device

**症状。** ホストには空きがあるのに、ビルドや取得が *no space left on device*
で失敗する。

**原因。** VM は固定サイズの独自ディスクを持ちます。VM のディスクが埋まることは
ホストの空き容量とは無関係です。

**対処。** まず VM 内で回収します:

```bash
docker system df
docker system prune -a --volumes
```

それでも埋まったままなら Settings → Colima Config でディスクサイズを増やします。
ディスクは拡大のみ可能で、縮小はできません。

## 関連

- [Colima インスタンスの起動](start-colima)
- [パフォーマンス調整](performance-tuning)
