# パフォーマンス調整

## 症状

ビルドが遅い、ファイルの変更がコンテナ内に反映されるまで数秒かかる、または
VM 実行中はマシン全体が重くなる。

## 原因

ほぼすべてが 3 つの設定に起因します。既定値は、サポート対象の最も小さい
マシンでも起動できる必要があるため保守的になっています:

- **VM type。** `qemu` はエミュレーション、`vz` は Apple のネイティブ
  Virtualization フレームワークを使います。Apple Silicon では `vz` が大幅に
  高速です。
- **Mount type。** `sshfs` はすべてのファイル操作を SSH 経由で行います。
  `virtiofs` はネイティブの共有ファイルシステムで、Node や PHP のような
  バインドマウントを多用するワークロードでは最大の効果があります。
- **サイズ。** CPU 2 個と 2 GiB は VM の起動には足りますが、その中でビルドする
  には足りません。

## 対処

Apple Silicon では VM type と mount type を切り替えます。作り直しではなく
再起動です:

```bash
colima stop
colima start --vm-type vz --mount-type virtiofs
```

次に余裕を与えます。目安はホストのコア数の半分、RAM の半分です:

```bash
colima stop
colima start --cpu 4 --memory 8 --disk 100
```

どちらも Settings → Colima Config で編集できます。そこでは `colima.yaml` に
書き込み、適用前に差分を表示します。変更は次回の再起動で有効になります。

最後に、定期的に空き容量を回収してください。ディスクが満杯の状態は、遅い状態と
見分けがつきません:

```bash
docker system df
docker system prune -a --volumes
```

## やってはいけないこと

- VM に全コアを割り当てないでください。ホストのスケジューラと競合し、マシン
  全体が遅くなります。
- コンテナしか動かさないプロファイルで Kubernetes を有効のままにしないで
  ください。常時約 1 GiB のメモリを消費します。
- 後で変えられると考えてディスクを過大にしないでください。ディスクは拡大のみ
  可能で縮小はできません。

## 関連

- [よくあるエラー](common-errors)
- [Colima インスタンスの起動](start-colima)
