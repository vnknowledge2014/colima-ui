# kubectl のインストールと Colima への接続

## 症状

Kubernetes ページが空になる、`kubectl: command not found` が出る、または
*The connection to the server localhost:8080 was refused* が表示される。

## 原因

必要なものが 2 つあり、どちらが欠けても失敗します:

1. ホスト上の `kubectl` バイナリ。
2. Colima VM 内の Kubernetes クラスタ。Colima は既定では起動**しません** —
   プロファイルごとに有効化する必要があります。

`localhost:8080` というメッセージは、kubectl 自体は動いたがコンテキストが未設定
のため組み込みの既定アドレスにフォールバックしたことを意味します。

## 対処

クライアントをインストールします:

```bash
brew install kubectl
kubectl version --client
```

次に VM 内の Kubernetes を有効化し、そのコンテキストを選択します:

```bash
colima start --kubernetes
kubectl config get-contexts
kubectl config use-context colima
kubectl get nodes
```

Settings → Colima Config から有効化することもできます。設定は
`colima.yaml` に書き込まれ、インスタンスの次回再起動時に反映されます。

Kubernetes を有効にするとメモリ使用量が約 1 GiB、起動時間が約 1 分増えます。
コンテナだけを動かすプロファイルでは無効のままにしてください。

## 関連

- [Colima インスタンスの起動](start-colima)
- [パフォーマンス調整](performance-tuning)
