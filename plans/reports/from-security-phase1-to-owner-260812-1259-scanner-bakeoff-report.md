---
from: security-posture phase 1, bước 0
to: owner
date: 2026-08-12
subject: Bakeoff Trivy vs Grype — chốt scanner mặc định và trả lời cổng go/no-go
verdict: GO. Scanner mặc định = Trivy
---

# Bakeoff scanner — Trivy 0.73.0 vs Grype 0.117.0

**Corpus:** 23 image thật trên máy dev, 6MB → 4.86GB. Alpine, Debian, distroless;
image hạ tầng kéo từ registry lẫn image tự build tại chỗ. Không pull thêm gì —
đây đúng là những gì một người dùng ColimaUI thật sự có trong máy.

**Môi trường:** macOS aarch64, Colima 0.10.3, Docker 29.x, docker context `colima`
(socket `unix:///Users/longnd/.colima/default/docker.sock`), `DOCKER_HOST` **không**
được set — đây là cấu hình mặc định của Colima và nó hoá ra quyết định kết quả.

Script: `scripts/security-scanner-bakeoff.py`. Dữ liệu thô: `bakeoff.json` / `bakeoff.tsv`.

## Phán quyết

**GO. Dùng Trivy làm scanner mặc định.** Không hỗ trợ đa scanner ở phase 1.

## Phát hiện quan trọng nhất — Grype rò tên image ra Docker Hub

Đây là thứ không có trong danh sách sáu câu hỏi, và là lý do đáng giá nhất của
việc chạy bakeoff trên máy thật thay vì tin tài liệu.

Grype **không đọc docker context, chỉ đọc biến môi trường `DOCKER_HOST`.** Colima
dựng một docker *context* và để `DOCKER_HOST` trống. Hệ quả: Grype không thấy
daemon, rồi **im lặng rơi xuống phương án kế tiếp là kéo image từ registry công
cộng**:

```
oci-registry: failed to get image descriptor from registry:
GET https://index.docker.io/v2/library/recipe-nutrition-platform-api/manifests/latest:
UNAUTHORIZED: authentication required
```

Tên image riêng tư của người dùng vừa được gửi tới Docker Hub. Với image nội bộ
thì nó thất bại và lộ tên; với image công khai thì nó **thành công bằng cách tải
lại image từ mạng** thay vì đọc bản có sẵn dưới đĩa.

Hai hệ quả:

1. **Toàn bộ số đo thời gian của Grype trong báo cáo này là thời gian tải mạng,
   không phải thời gian scan.** Không so trực tiếp với Trivy được.
2. Nếu ColimaUI gọi `grype` một cách ngây thơ, nó sẽ làm rò tên image riêng tư
   của người dùng — vi phạm thẳng cam kết "không gửi danh sách image đi đâu" ở
   `plan.md`.

**Khắc phục được, và công cụ đã có sẵn trong repo:** export `DOCKER_HOST` trỏ vào
socket Colima thì Grype chạy đúng trên image local (đã kiểm chứng). `path_util.rs`
đã có `detect_docker_host()`. Nhưng đây là cái bẫy phải nhớ mãi mãi, còn Trivy
không có bẫy đó.

## Sáu câu hỏi của cổng

### 1. Chi phí DB lạnh — xác nhận KHÔNG THỂ bundle

| | Tải lần đầu | Cache trên đĩa |
|---|---|---|
| Trivy | 41.6s | **1229 MB** |
| Grype | 59.0s | **1996 MB** |

Plan ước tính Trivy ~700MB. Thực tế **1.2GB**, Grype **2.0GB**. Quyết định
"detect, không bundle" không những đúng mà còn đúng hơn ta tưởng.

### 2. Độ trễ scan ấm — CỔNG QUA

Ngưỡng plan đặt: 60s trên image ~200MB.

| Image | Kích thước | Trivy |
|---|---|---|
| `vector:0.53.0-alpine` | 196 MB | **2.6s** |
| `kong:2.8.1` | 212 MB | **2.5s** |
| `pgvector:pg16` | 952 MB | 8.6s |
| `postgres:17.6.1.158` | 1.76 GB | 17.4s |
| `recipe-…-web` | 4.86 GB | **49.5s** |

Ở mốc 200MB, Trivy nhanh hơn ngưỡng **~23 lần**. UI phase 3 **được phép** là
"bấm và chờ" kèm progress, không bắt buộc phải "chạy nền + thông báo".

**Nhưng** image 4.86GB mất 49.5s — sát ngưỡng. Khuyến nghị: ngưỡng theo kích
thước, >2GB thì chuyển sang chế độ nền. Hạ tầng đã có (`run_cmd_streaming` +
`cancel_stream`), nên đây là quyết định UI chứ không phải công việc thêm.

> **Cảnh báo về số liệu:** Trivy cache theo layer. Vài image dùng chung base cho
> ra thời gian 0.3–0.7s ở lần thứ hai (`recipe-…-api` 0.7s so với `recipe-…-web`
> 49.5s, dù cả hai đều 3.22GB+). Các số nhanh đó là **cache hit, không phải scan
> thật**. Con số dùng để quyết định là con số chậm trong mỗi cặp.

### 3. Chạy offline — CÓ, với một điều kiện

`trivy image --skip-db-update` chạy rc=0 trong 0.2s với DB có sẵn. Grype mặc định
cũng không cập nhật DB mỗi lần.

**Chưa kiểm thật bằng cách ngắt mạng.** Mới chỉ chứng minh cờ hoạt động. Với
Grype, "offline" còn giả nữa vì như trên nó đang tải image từ registry — không có
mạng thì nó hỏng với đa số image. Với Trivy, đường đi là daemon local nên nhiều
khả năng offline thật. **Cần kiểm bằng airplane mode trước khi quảng cáo tính
năng offline** — ghi vào Unresolved.

### 4. SBOM — Trivy thắng dứt khoát, và điều này CẮT được phạm vi

| | Sinh SBOM |
|---|---|
| Trivy | ✅ CycloneDX **và** SPDX-JSON, native, rc=0 |
| Grype | ❌ chỉ **tiêu thụ** SBOM. Sinh là việc của `syft` |

Chọn Trivy nghĩa là **phase 1 không cần detect `syft`** — bớt một binary phải dò,
một install hint, một bài KB, một nhánh lỗi. Phase 1 gọn đi thấy rõ.

### 5. Độ ổn định JSON — parser khoan dung là bắt buộc

Điểm tốt: **không công cụ nào phát ra nhãn severity ngoài tập dự kiến** trên cả
23 image. Enum đóng ở `Finding::severity` là an toàn.

Điểm phải xử lý: **hai công cụ viết hoa khác nhau.** Trivy dùng `CRITICAL`/`HIGH`,
Grype dùng `Critical`/`High`/`Negligible`. Chuẩn hoá tại biên parse, đúng như
`phase-01` đã đặc tả. Grype còn có `Negligible` mà Trivy không có.

Cả hai đều xuất JSON hợp lệ ở mọi lần chạy thành công — không có lỗi parse nào.

### 6. Chênh lệch số finding — LỚN, và nó xác nhận một quy tắc của plan

| Image | Trivy | Grype | Lệch |
|---|---|---|---|
| `postgres:17.6.1.158` | 28 | 373 | **13.3×** |
| `kong:2.8.1` | 90 | 282 | 3.1× |
| `storage-api:v1.68.10` | 37 | 83 | 2.2× |
| `postgrest:v14.16` | 789 | 301 | 2.6× (**ngược chiều**) |
| `nginx:alpine` | 0 | 8 | — |
| `busybox:latest` | 0 | 3 | — |
| `realtime:v2.124.2` | 382 | 374 | 1.02× |

Không có hướng nhất quán — mỗi công cụ nhiều hơn ở những image khác nhau. Điều
này **xác nhận bằng số liệu** quy tắc plan đã đặt: điểm phải gắn cứng với
`(scanner, scanner_version, db_snapshot_date, pack_version)` và **tuyệt đối không
so điểm giữa hai scanner**. Một người dùng đổi scanner sẽ thấy điểm nhảy 13 lần
mà image không đổi.

Cũng nghĩa là: nếu sau này hỗ trợ scanner thứ hai, lịch sử điểm ở phase 5 phải
**đứt đoạn** khi đổi scanner, y như khi đổi rule pack.

## Cả hai đều thất bại, ở những image khác nhau

Đây là điều phase 1 phải thiết kế cho, không phải coi là ngoại lệ.

| | Số image hỏng | Nguyên nhân |
|---|---|---|
| Trivy | **2 / 23** | `failed to initialize the struct from the temporary file: file blobs/sha256/… not found in tar` — lỗi đọc layer. Trên `edge-runtime`, `studio` |
| Grype | **5 / 23** | Toàn bộ image build tại chỗ. Do `DOCKER_HOST` như mục trên |

Grype scan được 2 image Trivy chịu thua (`studio`: 896 finding). Nhưng đó là lý
do để **báo lỗi tử tế**, không phải lý do để ship hai scanner. YAGNI.

**Yêu cầu rút ra cho phase 1:** scan thất bại là kết quả bình thường của một
image cụ thể, phải hiển thị được lý do, và **không được làm hỏng cả danh sách**.

## Vì sao Trivy, tóm tắt

| Tiêu chí | Trivy | Grype |
|---|---|---|
| Đọc được daemon Colima mặc định | ✅ | ❌ cần `DOCKER_HOST` |
| Rò tên image ra registry công cộng | không | **có, mặc định** |
| Sinh SBOM | ✅ CycloneDX + SPDX | ❌ cần syft |
| DB trên đĩa | 1.2 GB | 2.0 GB |
| Scan 200MB | 2.5s | không đo được sạch |
| Image hỏng | 2/23 | 5/23 |

## Ảnh hưởng tới `phase-01`

1. `ScannerKind` **chỉ có `Trivy`** ở phase 1. Không dựng trừu tượng cho công cụ
   thứ hai chưa cần.
2. **Bỏ việc detect `syft`** khỏi phase 1 — Trivy làm luôn SBOM.
3. Thêm yêu cầu: **`ScanResult` phải biểu diễn được thất bại từng image** kèm lý
   do đọc được, không chỉ đường thành công.
4. Ngưỡng UI theo kích thước image (>2GB → chạy nền), không phải một hằng số.
5. Nếu sau này thêm Grype: **bắt buộc set `DOCKER_HOST`** qua
   `path_util.rs::detect_docker_host()`. Ghi vào phase file để không ai vấp lại.

## Unresolved

1. **Chưa kiểm offline thật.** Mới chứng minh cờ `--skip-db-update` chạy được,
   chưa ngắt mạng. Phải kiểm trước khi UI hứa "hoạt động offline".
2. **Hai image Trivy không đọc được** — chưa rõ là bug Trivy, bug containerd, hay
   đặc thù image. Nếu tỉ lệ này đúng với người dùng thật (~9%) thì đường báo lỗi
   quan trọng hơn ta tưởng.
3. **Trivy bật secret scanning mặc định** (thấy trong log). Chưa đo phần này tốn
   bao nhiêu thời gian, và nó có nghĩa Trivy đang đọc nội dung file trong image.
   Cần quyết định có tắt bằng `--scanners vuln` không — ảnh hưởng cả tốc độ lẫn
   phạm vi dữ liệu chạm vào.
4. **Ngưỡng kích thước chuyển chế độ nền** tạm đề xuất 2GB, dựa trên 5 điểm đo.
   Nên chỉnh lại khi có dữ liệu thật.
