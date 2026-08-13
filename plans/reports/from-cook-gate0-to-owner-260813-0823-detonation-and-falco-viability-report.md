# Gate 0 — Detonation sandbox & Falco viability

**Ngày:** 2026-08-13 · **Phase:** security-posture 6 + 7 · **Máy đo:** macOS ARM (aarch64), Colima 0.10.3, vz driver

Hai phase đều mở đầu bằng một bước đo bắt buộc. Báo cáo này thay phần suy đoán
trong `phase-06` bước 1 và `phase-07` bước 1 bằng số thật. Mọi phép đo chạy trong
profile tạm `detonate-probe`, đã xoá sạch sau khi đo; profile `default` của người
dùng không bị chạm.

---

## Gate A — chi phí tạo/huỷ instance Colima (phase 6, bước 1)

| Phép đo | Kết quả |
|---|---|
| Tạo nguội (`colima start -p`, 2 CPU / 2 GiB / 10 GiB) | **24 giây** |
| Docker sẵn sàng sau khi start trả về | **0 giây** (đã sẵn) |
| Dừng (`colima stop -p`) | **2 giây** |
| Khởi động lại instance đã tồn tại | 30 giây |
| Dung lượng metadata trên host | 12 KB (`~/.colima/<profile>`) |

**Quyết định kiến trúc:** ngưỡng trong plan là 3 phút. Thực đo 24 giây, tức
**1/7 ngưỡng**. Bỏ hoàn toàn phương án "chuẩn bị trước một instance chờ sẵn" —
tạo theo yêu cầu, huỷ sau khi xong.

Đáng chú ý: khởi động lại (30 giây) **chậm hơn** tạo mới (24 giây). Nghĩa là giữ
lại instance giữa các phiên không mang lợi gì cả về tốc độ lẫn RAM. Tạo mới mỗi
phiên vừa nhanh hơn vừa sạch hơn — hai điều thường phải đánh đổi thì ở đây trùng
nhau.

### Phát hiện ngoài dự kiến — cướp docker context (chặn, không phải ghi chú)

`colima start -p <name>` **đổi docker context toàn cục** sang instance mới:

```
$ docker context show
colima-detonate-probe        # ← sau khi tạo instance detonation
```

Plan không nhắc điều này. Hệ quả nếu bỏ qua: mọi lệnh `docker` sau đó — **kể cả
của chính ColimaUI** — trỏ vào VM detonation. Danh sách container của người dùng
sẽ hiện container của mẫu đang bị kích nổ. Đây là lỗi đúng nghĩa, không phải
phiền toái thẩm mỹ.

**Ràng buộc bắt buộc cho phase 6:**
1. Lưu `docker context show` **trước** khi tạo, khôi phục **ngay sau** khi
   `colima start` trả về — không đợi tới teardown.
2. Mọi lệnh docker của detonation dùng `--context colima-<name>` tường minh,
   không bao giờ dựa vào context môi trường.
3. `sweep_orphans` cũng phải khôi phục context, vì nhánh crash không chạy bước 1.
4. Test khẳng định: context môi trường không đổi qua trọn một phiên, **kể cả
   đường lỗi và đường timeout**.

---

## Gate B — Falco có chạy trong Lima VM trên macOS ARM không? (phase 7, bước 1)

Plan ghi rõ: *"Nếu không chạy được, phase này dừng và ta báo lại chủ dự án."*

**Kết luận: CHẠY ĐƯỢC. Phase 7 không bị chặn.**

| Hạng mục | Kết quả |
|---|---|
| Guest kernel | Linux 6.8.0-117-generic, aarch64, Ubuntu 24.04 |
| Gói Falco arm64 | Có, repo chính thức `download.falco.org/packages/deb` |
| Thời gian cài | **9 giây** |
| Phiên bản | Falco 0.44.1 (aarch64), engine 0.62.0 |
| Driver modern_ebpf | **Nạp và gắn được** — `Opening 'syscall' source with modern BPF probe` |
| Systemd unit | `falco-modern-bpf.service` bật và chạy |
| Event thật | **Có**, kèm đầy đủ trường tương quan container |

Event thật thu được, đúng shape phase 7 cần:

```json
{"rule":"Read sensitive file untrusted","priority":"Warning","source":"syscall",
 "output_fields":{"container.id":"01cdef08d09b","container.image.repository":"alpine",
   "container.name":"reverent_mahavira","proc.cmdline":"cat /etc/shadow",
   "proc.exepath":"/bin/busybox","user.uid":0},
 "tags":["T1555","container","filesystem","mitre_credential_access"]}
```

`container.id` có sẵn → `correlate()` của phase 7 nối được sang tên compose
service. Không phải tự chế schema.

### Cảnh báo tracepoint trên arm64 — vô hại, nhưng sẽ bị người dùng báo lỗi

```
libbpf: failed to determine tracepoint 'syscalls/sys_enter_open' perf event ID
libpman: failure while attaching TOCTOU mitigation program for 'open'
         Detection will continue to work, but TOCTOU mitigation may not properly work
```

`open` và `creat` **không tồn tại như syscall trên arm64** — arm64 chỉ có
`openat`. Chỉ phần giảm thiểu TOCTOU mất, phát hiện vẫn chạy (event `/etc/shadow`
ở trên chính là `evt_type=openat`, bắt được bình thường).

Hệ quả cho phase 7: các dòng này xuất hiện **mỗi lần khởi động trên mọi máy Mac
ARM**. Nếu UI hiển thị log Falco thô, đây là thứ đầu tiên người dùng thấy và báo
là hỏng. Phải lọc hoặc chú thích rõ.

### Phát hiện nghiêm trọng nhất — Falco chạy với **không rule nào**

Lần chạy đầu qua systemd unit của gói: driver gắn thành công, tiến trình sống,
**không một event nào**. Log:

```
Loading rules from:            ← danh sách rỗng
Loaded event sources: syscall
```

Nguyên nhân: `falcoctl` (dịch vụ tải rule) bị tắt → không có rule nào được nạp,
dù `/etc/falco/falco_rules.yaml` **có sẵn trên đĩa**. Chỉ khi truyền
`-r /etc/falco/falco_rules.yaml` tường minh thì event mới xuất hiện.

Đây là trạng thái nguy hiểm nhất một công cụ bảo mật có thể ở: **đang chạy,
trông khoẻ mạnh, và không phát hiện gì cả.** Người dùng cài Falco theo hướng dẫn
mặc định hoàn toàn có thể rơi vào đây.

**Ràng buộc cho phase 7:**
1. Detect Falco **không được dừng ở "tiến trình có chạy không"**. Phải trả lời
   được "có bao nhiêu rule đang nạp". Không rule = trạng thái riêng, hiển thị
   như cảnh báo, không phải như "đã sẵn sàng".
2. Bài KB phải ghi bước xác minh rule đã nạp, không chỉ bước cài.

### Va tên `falco` trên Homebrew — cạm bẫy trong install hint

```
$ brew info falco
==> falco: stable 2.3.0
VCL parser and linter optimized for Fastly
https://github.com/ysugimoto/falco       ← KHÔNG phải Falco của CNCF
```

`brew install falco` cài nhầm một trình lint VCL của Fastly. Thêm nữa, Falco của
CNCF **không có bản chạy trên host macOS** — nó phải chạy trong VM Linux.

**Ràng buộc cho phase 7:**
1. Detect **không được** dùng `which falco` trên host. Trên macOS thì luôn phải
   dò **bên trong VM**.
2. `install_hint` cho `falco` tuyệt đối không được là `brew install falco`.
3. Phải phân biệt được hai nhị phân: kiểm chuỗi version (`Falco version: X` so
   với bản Fastly), không chỉ kiểm file có tồn tại.

---

## Việc gate 0 đổi trong plan

| Chỗ | Trước | Sau khi đo |
|---|---|---|
| phase 6 kiến trúc | mở: tạo theo yêu cầu **hoặc** instance chờ sẵn | **Tạo theo yêu cầu.** 24s, dưới ngưỡng 3 phút |
| phase 6 | — | **Mới:** khôi phục docker context là ràng buộc chặn |
| phase 7 khả thi | chưa biết, là cổng chặn | **Chạy được.** Không chặn |
| phase 7 detect | "detect qua system_capabilities" | Phải dò trong VM + đếm rule + phân biệt với falco của Fastly |
| plan.md Unresolved #3 | bundle Falco hay không? | Nghiêng hẳn về "người dùng tự cài": cài mất 9 giây, ta bundle không thêm giá trị |

## Chưa giải quyết

1. **Sinkhole chưa đo.** Chủ dự án đã chốt ship `--network none` ở v1, nên đây
   chưa chặn. Khi làm sinkhole phải có test khẳng định không gói nào rời VM.
2. **Chưa đo dưới tải.** Event flood (ngưỡng priority, ảo hoá danh sách) vẫn là
   rủi ro đã ghi trong phase 7, gate này không chạm tới.
3. **gRPC output chưa kiểm.** Không chặn — phase 7 chốt làm tail file trước.
