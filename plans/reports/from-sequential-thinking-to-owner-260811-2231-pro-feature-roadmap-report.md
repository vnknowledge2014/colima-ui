# Phân tích 7 tính năng Pro — hướng xây dựng & roadmap

Ngày: 2026-08-11 · Branch: `dev` · Nguồn đối chiếu: `docs/pricing-rationale.md`, `src-tauri/src/`, `src/pages/`

---

## 0. Ràng buộc phải tôn trọng (từ pricing-rationale.md)

Hai lời hứa đã công bố, mọi quyết định gate phải đi qua:

1. **Không tính tiền thứ Colima/Docker CLI đã làm miễn phí.** Paywall một wrapper quanh lệnh free = mời người ta fork.
2. **Core app MIT, không có bản bị cắt xén.** Free = "toàn bộ app như hôm nay, kể cả dùng thương mại".
3. Entitlement là **boolean** (`proState.paid`), không có thang bậc tính năng giữa Pro và Pro Teams.
4. AI là **BYOK ở mọi tier**. Pro mở *tính năng* và gỡ *giới hạn*, không bán token.

→ Hệ quả trực tiếp: trong 7 mục đề xuất, **không phải mục nào cũng được phép là Pro**. Gate sai chỗ sẽ phá lời hứa đã viết ra và làm yếu chính lập luận định giá.

---

## 1. Phân loại 7 tính năng

Ba câu hỏi cho mỗi mục: (a) CLI có làm sẵn không? (b) Nỗi đau thật hay đồ trang trí? (c) Chi phí xây so với chỗ đã có trong repo.

| # | Tính năng | CLI có sẵn? | Đã có gì trong repo | Gate đề xuất | Chi phí |
|---|---|---|---|---|---|
| 7 | **Auto-fix compose error → recommendation** | Không (`compose config` chỉ báo lỗi, không sửa) | `commands/compose_diagnose.rs`, `knowledge_bank.rs`, `components/compose/DiagnosePanel.svelte` — đã ~70% | **Pro** (apply patch + không giới hạn) | Thấp |
| 6 | **Self-healing** | Không (`restart: always` chỉ restart mù, không chẩn đoán) | `docker_state.rs`, `sse.rs`, KB signature matching | **Pro** | Trung bình |
| 4 | **Cluster + transfer giữa các cụm** | Không | `commands/kind.rs`, `k8s_cluster.rs`, `adapters/kubectl.rs` | **Pro** (neo giá trị cho Teams) | Cao |
| 2 | **Activity Monitor** | Một phần (`docker stats`, `top`) | `/api/containers/stats/all`, `/api/system/engine-resources` | **Tách**: live view Free, *lịch sử + cảnh báo* Pro | Trung bình |
| 1 | **Graph view** | Không | `pages/ClusterTopology.svelte`, `pages/XRay.svelte` | Free (hoặc Pro nếu là graph *liên cụm*, xem #4) | Trung bình |
| 3 | **File transfer + export image TAR** | **Có** (`docker cp`, `docker save`) | routes containers/images | **Free — bắt buộc** | Thấp |
| 5 | **Diagnostic log / report bug** | Không, nhưng đây là công cụ hỗ trợ | `crash.rs`, `redact.rs`, `telemetry/` | **Free** | Thấp |

### Lý do cho ba quyết định gây tranh cãi nhất

**#3 File transfer + export TAR phải là Free.** `docker cp` và `docker save` là lệnh miễn phí; bán GUI bọc quanh chúng vi phạm trực tiếp quy tắc số 1. Giá trị thật của mục này là *thu hút người dùng*, không phải doanh thu — nó là lý do người ta mở app thay vì mở terminal. Đặt nó ở Free và dùng nó làm phễu.

**#2 Activity Monitor phải tách đôi.** `docker stats` đã cho số liệu tức thời. Cái CLI *không* có là: giữ lịch sử qua thời gian, so sánh trước/sau, và bắn cảnh báo khi vượt ngưỡng. Gate đúng ranh giới đó: xem hiện tại = Free, *nhớ và cảnh báo* = Pro. Ranh giới này cũng trùng khít với hạ tầng #6 cần.

**#1 Graph view khó bán một mình.** Nó là tính năng người ta khen chứ không phải trả tiền. Đề xuất: để Free như một phần của app, và dồn sức cho phiên bản *graph liên cụm* ở #4 — nơi nó thực sự là thứ duy nhất trả lời được câu "cụm nào đang nói chuyện với cụm nào".

---

## 2. Xâu chuỗi phụ thuộc

Bốn tính năng Pro không độc lập — chúng dùng chung ba nền tảng:

```
              ┌─ Signature engine (KB matching, redact) ──┬─> #7 Auto-fix
              │                                            └─> #5 Bug report
Nền tảng ─────┼─ Time-series store (metrics + events) ────┬─> #2 History/Alerts
              │                                            └─> #6 Self-healing
              └─ Multi-context registry (cluster/instance) ┬─> #4 Cluster transfer
                                                            └─> #1 Graph liên cụm
```

Xây nền tảng trước, tính năng sau. Xây ngược lại sẽ có ba bản sao của cùng một store.

---

## 3. Roadmap — 5 giai đoạn

Thứ tự chọn theo: nỗi đau lớn nhất trước, tận dụng code đã có nhiều nhất trước, rủi ro cao nhất sau cùng.

### Giai đoạn 1 — Chốt Auto-fix Compose (Pro đầu tiên có thể bán)
**Vì sao trước tiên:** đã có 70% code, và đây là "nỗi đau" bạn tự nhận diện. Nó biến `proState.paid` từ boolean chưa dùng thành boolean có ý nghĩa.

- Free: chẩn đoán + tra Knowledge Bank + xem trước payload (đã có hôm nay).
- Pro: sinh **patch YAML** cụ thể, diff trước/sau, một nút Apply có thể hoàn tác; không giới hạn số lần chẩn đoán bằng LLM.
- Việc cần làm: bộ sinh patch (deterministic cho các category đã phân loại trong `compose_diagnose.rs`, LLM chỉ cho phần còn lại), UI diff, ghi lại lịch sử fix, gate ở `ProGate.svelte`.
- Rủi ro: patch sai làm hỏng file người dùng → bắt buộc backup + hoàn tác một chạm, không bao giờ ghi đè im lặng.
- Xong khi: 10 file compose hỏng thật (dùng `scripts/compose-diagnose-benchmark.sh`) được sửa đúng ≥7, không file nào bị hỏng thêm.

### Giai đoạn 2 — Diagnostic bundle + Report bug (Free, hạ tầng cho phần sau)
**Vì sao thứ hai:** rẻ, dùng lại `redact.rs` + `crash.rs`, và cho bạn dữ liệu lỗi thật để nuôi Knowledge Bank ở #7. Đây là giai đoạn *tự phục vụ chính bạn*.

- Gom: log container liên quan, phiên bản colima/docker, host specs, crash gần nhất → 1 file zip đã redact.
- Người dùng xem toàn bộ nội dung trước khi gửi; gửi là hành động thủ công, có xác nhận.
- Signature hoá lỗi tái dùng chính hàm `categorize()` đã có.
- Xong khi: bundle không chứa secret nào trên bộ test redact; issue GitHub tạo được từ trong app.

### Giai đoạn 3 — Time-series store + Activity Monitor
**Vì sao thứ ba:** là nền tảng bắt buộc của #6, và tự nó đã bán được phần lịch sử/cảnh báo.

- Store: SQLite cuộn vòng (ring buffer theo thời gian), lấy mẫu từ `/api/containers/stats/all` — cần chống việc poll làm nặng máy.
- Free: bảng live + sắp xếp + lọc (thay thế `docker stats` bằng UI tử tế).
- Pro: biểu đồ lịch sử, so sánh khoảng thời gian, quy tắc cảnh báo (CPU/mem/restart-loop), thông báo hệ thống.
- Xong khi: app chạy nền 24h dưới 2% CPU trung bình và store không vượt ngưỡng dung lượng đã đặt.

### Giai đoạn 4 — Self-healing (Pro)
**Vì sao thứ tư:** phụ thuộc GĐ3 (phát hiện) và GĐ1 (biết cách sửa).

- Luật rõ ràng, người dùng thấy được: điều kiện → hành động → giới hạn số lần. Không có "ma thuật".
- Hành động khởi đầu: restart container unhealthy, gỡ crash-loop, cảnh báo đầy disk + gợi ý prune, VM treo → restart colima.
- Bắt buộc: chế độ **dry-run** mặc định, nhật ký mọi hành động tự động, công tắc tắt toàn cục.
- Rủi ro lớn nhất của cả roadmap: một hành động tự động sai trên máy production của người dùng. Vì vậy mặc định là *đề xuất*, tự động chỉ khi người dùng bật từng luật.
- Xong khi: mô phỏng được 5 kịch bản hỏng và mỗi kịch bản đều phục hồi hoặc báo đúng lý do bỏ cuộc.

### Giai đoạn 5 — Cluster + transfer liên cụm (Pro, neo cho Teams)
**Vì sao cuối:** rủi ro và chi phí cao nhất, và là thứ duy nhất cần thiết kế mới hoàn toàn.

- Registry đa ngữ cảnh: liệt kê colima instances + kind clusters như các endpoint đồng hạng.
- Transfer: đẩy image giữa các cụm (qua registry cục bộ hoặc `save`/`load` có luồng tiến độ), copy config/secret có xác nhận rõ ràng.
- Graph liên cụm: cụm nào, service nào, đang nói chuyện với ai — đây là chỗ #1 trở nên đáng tiền.
- Rủi ro: copy secret giữa các môi trường là đường ngắn nhất tới rò rỉ dữ liệu → mọi thao tác chạm secret phải hiển thị đúng cái gì sẽ đi đâu, và mặc định là từ chối.
- Xong khi: chuyển được một image giữa 2 kind cluster và một service chạy được ở đích.

---

## 4. Bảng Free / Pro sau khi xong roadmap

| Free | Pro |
|---|---|
| Toàn bộ app hôm nay | Auto-fix compose: patch + apply + hoàn tác |
| File transfer, export image TAR | Lịch sử metrics + cảnh báo ngưỡng |
| Activity Monitor (live) | Self-healing (luật + tự động) |
| Graph view trong một cụm | Transfer & graph liên cụm |
| Diagnostic bundle + report bug | Không giới hạn số lần chẩn đoán LLM |
| Chẩn đoán compose + Knowledge Bank | |

Entitlement vẫn là một boolean. Pro Teams vẫn là "Pro cho N người" — không mục nào ở trên chỉ dành cho Teams.

---

## 5. Câu hỏi còn để ngỏ

1. **Activity Monitor tách đôi có làm bạn khó chịu không?** Đây là quyết định gate duy nhất tôi đề xuất đi ngược lại cách liệt kê ban đầu của bạn (bạn xếp toàn bộ vào Pro). Lý do là quy tắc "không bán wrapper", nhưng đây là quyết định sản phẩm của bạn.
2. **Graph view: để Free hay giữ Pro?** Tôi nghiêng về Free vì nó khó bán một mình, nhưng nếu bạn coi nó là màn hình đại diện của sản phẩm thì lập luận có thể đổi.
3. **Self-healing có nên tự động thật sự, hay chỉ đề xuất?** Tự động bán tốt hơn, đề xuất an toàn hơn. Tôi đề xuất mặc định là đề xuất, và cần bạn chốt.
4. **Cluster transfer nhắm colima instances hay kind clusters trước?** Hai đối tượng khác nhau về kỹ thuật; làm cả hai cùng lúc sẽ kéo GĐ5 dài gấp đôi.
5. Chưa có ước lượng thời gian — cần biết bạn làm một mình hay có người khác trước khi đưa số.
