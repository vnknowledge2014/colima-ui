# Colima 向け Falco のインストール

Falco は CNCF のランタイムセキュリティエンジンです。eBPF でシステムコールを監視
し、ルールに一致する挙動——コンテナ内でのシェル起動、機密ファイルの読み取り、
想定外の外向き接続など——を検知するとアラートを出します。

ColimaUI 自体は何も検知しません。Falco が報告した内容を読み取り、あなたが普段
使っている compose サービス名の隣に表示するだけです。検知範囲は、あなたの Falco
ルールが対象とする範囲がすべてです。

## 始める前に知っておくべき 2 つの罠

**1. `brew install falco` では別のソフトが入ります。**
`falco` という名前の無関係なツール——Fastly 向けの VCL パーサー兼リンター——が存在
し、Homebrew の `falco` フォーミュラで入るのはそちらです。ランタイムセキュリティ
とは何の関係もありません。ColimaUI はこの状態を検出し、Falco があるふりをせずに
そう表示します。

**2. Falco は macOS では動きません。**
Linux カーネルが必要です。Mac では *Colima の VM 内* で動作し、ホスト上では動き
ません。そのため以下のコマンドはすべて `colima ssh` 経由で実行します。

## インストール

Falco は arm64 と x86_64 のパッケージを公開しています。Apple Silicon の Mac では
VM 内で 10 秒ほどで完了します。

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

ドライバは `modern_ebpf` を選びます。カーネルヘッダーもモジュールのビルドも不要
で、Colima が使うカーネル (6.8) が対応しています。

## 本当に検知しているか確認する

ここは飛ばされがちですが、最も重要な手順です。

Falco は正常にインストールされ、eBPF ドライバを読み込み、systemd サービスとして
健全に動作していても——**まったく検知しない**ことがあります。ルールを 1 つも読み
込んでいない場合です。ステータス表示のどこにもそうとは書かれません。動いている
のに何も見ていないツールは、入っていないツールより危険です。守られていると信じ
てしまうからです。

ルール数を確認します。

```sh
sudo falco -L -o json_output=true | python3 -c \
  'import sys,json; print(len(json.load(sys.stdin)["rules"]), "件のルールを読み込み")'
```

標準構成ではおよそ 25 件が読み込まれます。**0 と表示された場合、Falco は何も検知
していません。** よくある原因は、インストール時にルール取得 (`falcoctl`) が無効化
され、`/etc/falco/falco_rules.yaml` がディスク上にあるにもかかわらずエンジンが空
のルールセットで動いていることです。ファイルを明示的に指定します。

```sh
sudo falco -r /etc/falco/falco_rules.yaml
```

ColimaUI はこの状態を「準備完了」ではなく独立した警告として表示します。Runtime
タブに「ルールなしで動作中」と出たら、この意味です。

## ColimaUI がイベントを読めるようにする

Falco はファイル出力が**無効な状態**で出荷されるため、既定では ColimaUI が読む
ものがありません。JSON でのファイル出力を有効にします。

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

その後 ColimaUI の **セキュリティ → Runtime** を開き、*読み取り開始* を押します。

## 端から端まで動作確認する

既定のルールセットに含まれるルールを発火させます。

```sh
docker run --rm alpine cat /etc/shadow
```

数秒以内に *Read sensitive file untrusted* のイベントが Runtime タブに現れ、発生
元のコンテナ名が付いているはずです。

## Apple Silicon で出る想定内のノイズ

arm64 では起動のたびに次のログが出ます。

```
libbpf: failed to determine tracepoint 'syscalls/sys_enter_open' perf event ID
libpman: failure while attaching TOCTOU mitigation program for 'open'
```

無害です。arm64 には `open` と `creat` がシステムコールとして存在せず `openat` を
使うため、その 2 つに対する TOCTOU 緩和だけが利用できません。検知自体は正常に動作
します。上の `/etc/shadow` のテストで確認できます。

## これで得られないもの

Falco が検知するのは、ルールに記述された挙動だけです。ルートキットスキャナーでは
なく、何かを阻止することもありません。誰も調整していないルールセットは、通常の
作業に対しても誤検知します——パッケージのインストールも機密ファイルを読みます。
イベントは結論ではなく、調査を始める手がかりとして扱ってください。
