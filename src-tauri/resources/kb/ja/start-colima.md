# Colima インスタンスの起動

## 症状

*Cannot connect to the Docker daemon*、*Colima is not running*、
*the connection to the server was refused* といったエラーが出る。ツールは
インストール済みなのに Container / Image / Kubernetes ページが空になる。

## 原因

Colima はインストールされているが、その仮想マシンが停止しています。本アプリの
Docker および Kubernetes コマンドはすべて VM の**内部**にあるデーモンと通信
するため、VM が停止していれば応答するものがありません。再起動後はこれが通常の
状態です — Colima は自動起動しません。

## 対処

```bash
colima start

# or, for a named profile
colima start --profile dev
```

または Instances ページの **Start** を押します。プロファイルの初回起動は
ディスクイメージのダウンロードとプロビジョニングのため 1〜2 分かかります。
2 回目以降は数秒です。

起動に失敗した場合、VM 自身のログが理由を示します:

```bash
colima status
colima start --verbose
tail -n 100 ~/.colima/default/daemon/daemon.log
```

覚えておくべき失敗は 2 つです:

- **ホストのディスク容量不足。** ディスクイメージは事前に確保されます。空き容量
  を作るか、Settings → Colima Config でディスクサイズを下げてください。
- **中断された起動による壊れた VM。** 作り直します。VM 内のコンテナとイメージ
  は消えますが、ソースコードやホスト側のマウント先は消えません:

  ```bash
  colima stop
colima delete
colima start
  ```

## 関連

- [よくあるエラー](common-errors)
- [パフォーマンス調整](performance-tuning)
