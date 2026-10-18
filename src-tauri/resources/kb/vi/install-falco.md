# Cài Falco cho Colima

Falco là engine bảo mật thời gian chạy của CNCF. Nó theo dõi system call bằng
eBPF và báo động khi có thứ khớp với rule của nó — một shell mở trong container,
một tệp nhạy cảm bị đọc, một kết nối ra ngoài bất thường.

ColimaUI không tự phát hiện gì cả. Nó đọc lại những gì Falco báo và hiển thị
cạnh tên compose service bạn quen dùng. Phạm vi phát hiện đúng bằng rule Falco
của bạn, không hơn.

## Hai cái bẫy trước khi bắt đầu

**1. `brew install falco` cài nhầm phần mềm.**
Có một công cụ khác cũng tên `falco` — trình phân tích và lint VCL cho Fastly.
Đó chính là thứ formula `falco` của Homebrew đưa cho bạn. Nó không liên quan gì
tới bảo mật thời gian chạy. ColimaUI nhận ra trường hợp này và nói thẳng, thay
vì giả vờ rằng Falco đã có.

**2. Falco không chạy trên macOS.**
Nó cần kernel Linux. Trên máy Mac, nó chạy *bên trong VM của Colima*, không phải
trên host — nên mọi lệnh dưới đây đều đi qua `colima ssh`.

## Cài đặt

Falco phát hành gói cho cả arm64 và x86_64. Trên máy Apple Silicon, bước này mất
khoảng mười giây bên trong VM.

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

Chọn driver `modern_ebpf`: nó không cần kernel header, không cần build module,
và kernel Colima đang dùng (6.8) hỗ trợ sẵn.

## Kiểm chứng rằng nó thật sự đang phát hiện

Đây là bước hay bị bỏ qua, và là bước quan trọng nhất.

Falco có thể cài sạch sẽ, nạp được driver eBPF, chạy như một dịch vụ systemd
khoẻ mạnh — **và không phát hiện gì cả**, vì nó nạp không rule nào. Không dòng
trạng thái nào nói ra điều đó. Một công cụ đang chạy mà mù còn tệ hơn một công
cụ không có, vì bạn tin rằng mình đang được bảo vệ.

Đếm số rule:

```sh
sudo falco -L -o json_output=true | python3 -c \
  'import sys,json; print(len(json.load(sys.stdin)["rules"]), "rule đã nạp")'
```

Bản cài mặc định nạp khoảng 25 rule. **Nếu kết quả là 0, Falco không phát hiện
gì cả.** Nguyên nhân thường gặp: phần tải rule (`falcoctl`) bị tắt lúc cài, khiến
engine chạy với tập rule rỗng dù tệp `/etc/falco/falco_rules.yaml` vẫn nằm trên
đĩa. Hãy chỉ thẳng cho Falco:

```sh
sudo falco -r /etc/falco/falco_rules.yaml
```

ColimaUI hiển thị trạng thái này như một cảnh báo riêng chứ không phải "sẵn
sàng" — nên nếu tab Runtime báo Falco đang chạy mà không có rule, đó chính là ý
này.

## Cho ColimaUI đọc được sự kiện

Falco xuất xưởng với ghi tệp **tắt sẵn**, nên mặc định ColimaUI không có gì để
đọc. Bật ghi JSON ra tệp:

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

Rồi mở **Security → Runtime** trong ColimaUI và bấm *Bắt đầu đọc*.

## Kiểm tra từ đầu đến cuối

Kích một rule có sẵn trong tập mặc định:

```sh
docker run --rm alpine cat /etc/shadow
```

Sau vài giây, một sự kiện *Read sensitive file untrusted* sẽ xuất hiện ở tab
Runtime, kèm tên container sinh ra nó.

## Nhiễu bình thường trên Apple Silicon

Trên arm64, Falco in những dòng này mỗi lần khởi động:

```
libbpf: failed to determine tracepoint 'syscalls/sys_enter_open' perf event ID
libpman: failure while attaching TOCTOU mitigation program for 'open'
```

Vô hại. `open` và `creat` không tồn tại như syscall trên arm64 — nó dùng `openat`
— nên chỉ phần giảm thiểu TOCTOU cho hai lệnh đó là không có. Việc phát hiện vẫn
chạy bình thường, kiểm bằng bài thử `/etc/shadow` ở trên.

## Những gì nó KHÔNG cho bạn

Falco phát hiện hành vi mà rule của nó mô tả. Nó không phải trình quét rootkit,
nó không chặn gì cả, và một tập rule chưa ai tinh chỉnh sẽ báo nhầm với công việc
bình thường — cài gói phần mềm cũng đọc tệp nhạy cảm. Hãy coi sự kiện là điểm
khởi đầu của một câu hỏi, không phải một kết luận.
