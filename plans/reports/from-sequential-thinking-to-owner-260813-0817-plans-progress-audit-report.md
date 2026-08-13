---
from: sequential-thinking
to: owner
date: 2026-08-13 08:17
subject: Kiểm tra tiến độ toàn bộ plan — đo bằng code, không tin phần text tự khai
method: đối chiếu từng tuyên bố trong plan với artifact thật + chạy các cổng chất lượng
---

# Tiến độ plan — đo lúc 2026-08-13 08:17

**Cách đo:** trạng thái tự khai trong `plan.md` đã sai ít nhất hai lần trong 24h
qua (bảng phụ thuộc của security-posture khai P7 chưa xong khi nó đã xong). Nên
mọi dòng dưới đây đối chiếu với file/symbol thật trong repo, và ghi rõ tuyên bố
nào khớp, tuyên bố nào không.

## Thought 1/7 — Bức tranh tổng: tiến độ rất lớn, chất lượng cũng giữ được

| Cổng | Kết quả |
|---|---|
| `cargo test --lib` | ✅ **452 pass, 0 fail**, 5 ignored |
| `svelte-check` | ✅ 407 file, 0 lỗi, 0 cảnh báo |
| `check:tokens` | ✅ 86 defined / 78 referenced |
| `pnpm build` | ✅ 0 lỗi |
| `check:i18n` | ❌ **fail** |
| `vitest` (frontend) | ❌ **không chạy được** |

452 test Rust là con số đáng kể — hôm qua là 211. Không phải làm ẩu.

## Thought 2/7 — Bảng trạng thái thật của 5 plan đang mở

| Plan | Tự khai | Đo được | Khớp? |
|---|---|---|---|
| `260812-1013-security-posture` | phase 1,2,4,5,8 xong; 3 xong phần UI | đúng như vậy | ✅ |
| `260811-2245-pro-tier-features` | P1 xong, P2 còn soak, P3 code xong | `compose_autofix*.rs`, `metrics_store.rs`+`alerts.rs`, `self_heal.rs` đều có | ✅ |
| `260813-0142-activity-history-local` | phase 1,2 xong; 3,4 chưa | `commands/activity.rs`, `routes/activity.rs` có | ✅ |
| `260811-0919-terminal-integration` | 6/7 xong, phase 6 chờ mắt người | — | ✅ |
| `260811-1930-subscription-teams` | P3 huỷ, P7 partial | — | ✅ |
| `260810-2258-commercial-foundation` | phase 1 in-progress (website/MoR chưa) | — | ✅ |

**Đã archive thêm 4 plan** kể từ lần nhìn trước: free-tier-foundation,
platform-prerequisites, notification-center-background-transfers,
cloud-broadcast-announcements, security-ui-posture-redesign.

Lần này **không tìm thấy tuyên bố sai nào.** Đây là cải thiện thật so với hôm qua.

## Thought 3/7 [VERIFICATION] — Báo cáo bakeoff đã thực sự được dùng

Đây là thứ đáng kiểm nhất, vì một report không ai đọc thì bằng không. Đọc
`commands/security_scan.rs`:

| Khuyến nghị trong report | Trong code |
|---|---|
| Chỉ Trivy, không dựng trừu tượng cho công cụ thứ hai | `pub enum ScannerKind { Trivy }` — một biến thể |
| Bỏ detect `syft` | `grep syft src-tauri/src/commands/system_capabilities.rs` → **không có** |
| `ScanResult` biểu diễn được thất bại từng image | doc ghi rõ *"One image the scanner cannot read must produce one readable error, never a broken list"* |
| Severity là enum đóng, chuẩn hoá tại biên | doc ghi *"Trivy writes CRITICAL, Grype writes Critical and adds Negligible"* |
| Cảnh báo Grype rò tên image qua `DOCKER_HOST` | doc header chép lại nguyên phát hiện |

Module doc trích cả con số "2 of 23 images" và lý do Grype bị loại. Report được
đọc và dùng đúng, không phải làm cho có.

## Thought 4/7 — Hai phase chưa động tới, và đó là đúng thứ tự

`commands/detonation.rs`, `falco_bridge.rs`, `falco_triage.rs`,
`routes/detonation.rs` — **không tồn tại**.

Đó là phase 6 (detonation sandbox) và phase 7 (mặt tiền Falco), tức Lớp 3. Plan
ghi rõ *"Đường tới hạn: 1 → 2 → 3 là thứ khiến người dùng chọn sản phẩm. Không
được hoãn để làm phase 6/7 trước, dù chúng thú vị hơn."*

Thứ tự đã được tôn trọng. Lớp 1 và Lớp 2 xong trước, Lớp 3 chưa bắt đầu.

## Thought 5/7 — Phase 3 xong một nửa, và nửa còn lại bị chặn bởi một quyết định

`commands/security_catalog.rs` tồn tại, nhưng:

- `grep 'ed25519\|minisign\|ring' src-tauri/Cargo.toml` → **không có crate ký số nào**
- Module doc tự thú: *"Phase 3 of the plan pairs this with a signed update channel (`catalog-vN.json` + `.sig`, verified before parsing). That needs a private [key]…"*

Nghĩa là: catalog **nhúng trong binary** đang chạy, **kênh cập nhật ký số thì chưa
có**. Plan tự khai đúng ("xong phần UI, kênh cập nhật ký số chờ Unresolved #1").

Đây không phải nợ kỹ thuật — nó bị chặn bởi **Unresolved #1: chưa ai quyết ai
giữ khoá riêng và ký ở đâu**. Đó là quyết định của anh, không phải việc code.

## Thought 6/7 — Ba vấn đề thật, xếp theo mức nghiêm trọng

### 1. 346 đường dẫn chưa commit — và một commit đã bốc hơi

```
147 modified   169 untracked   25 added   3 AM   2 deleted
```

Phân bố: `src-tauri/src` 85, `src/lib` 68, `src/components` 34,
`plans/reports` 32, `src-tauri/resources` 26, `src/pages` 23.

Đây là **toàn bộ Lớp 1 + Lớp 2 của security, activity history, self-healing,
notification centre** nằm ngoài git. Cộng thêm:

- `archive/260812-2318-security-ui-posture-redesign/plan.md` tự ghi *"phase 1, 2, 3 xong 2026-08-13; **chưa commit**"*
- Commit `dc1a315` (honeypot KB) của tôi hôm qua **đã bị reset khỏi history** — object còn dangling, file còn trong working tree, nhưng không còn trong nhánh

Rủi ro không phải giả định: một commit **đã** mất một lần rồi. Với 346 file chưa
commit, lần mất tiếp theo sẽ tốn hơn nhiều.

### 2. Toàn bộ test frontend không chạy được

`undici@8.10.0` ném `webidl.util.markAsUncloneable is not a function` dưới Node
20.19.1; jsdom import undici. Hệ quả: **mọi file test Svelte đều không chạy** —
`NotificationPanel.test.ts`, `notifications.test.ts`, `AlertRuleEditor.test.ts`,
`TransferDialog.test.ts`, `osNotify.test.ts`, `transferNotifications.test.ts`.

Trong khi đó `src/pages` và `src/components` vừa nhận rất nhiều code mới (trang
Security, notification centre, activity history). Toàn bộ phần đó hiện **chỉ có
svelte-check và build đứng sau** — tức là kiểm kiểu và kiểm cú pháp, không kiểm
hành vi.

`cargo test` 452 pass che mất điều này: backend được kiểm rất kỹ, frontend thì
không kiểm gì.

### 3. `check:i18n` đỏ, và lệch theo hướng bất ngờ

`ja.json`, `vi.json`, `zh.json` — mỗi file **605 key, đều pass**.
`en.json` — **thiếu** các key `security.*` mà code đang dùng.

Ngược đời: ba bản dịch có key, bản gốc thì không. `en` là `FALLBACK_LOCALE`, nên
người dùng tiếng Anh đang rơi vào `default:` viết inline trong lời gọi `t()`,
còn ba ngôn ngữ kia dùng bản dịch thật. Chạy được, nhưng không nhất quán và cổng
đang đỏ.

Nhỏ và sửa nhanh — nhưng nó là cổng duy nhất trong repo đang đỏ vì lý do nội tại.

## Thought 7/7 [FINAL] — Nhận định

**Tiến độ tốt hơn tôi dự đoán, và chất lượng backend giữ được.** Lớp 1 và Lớp 2
của security-posture xong trong hơn một ngày, bakeoff được dùng đúng, thứ tự ưu
tiên được tôn trọng, không plan nào khai khống.

**Rủi ro không nằm ở tiến độ, nằm ở chỗ tiến độ đó chưa được cất giữ và một nửa
chưa được kiểm.** Ba việc trên, theo thứ tự:

1. **Commit.** 346 file, trong đó phần lớn là công việc đã xong và đã qua cổng.
   Việc này nên làm trước mọi thứ khác, và nên chia thành vài commit theo plan
   thay vì một commit khổng lồ.
2. **Gỡ test frontend.** Không phải để tăng con số, mà vì trang Security và
   notification centre hiện không có gì kiểm hành vi. Reviewer đề xuất
   `happy-dom`; nên chẩn đoán undici/Node trước vì có thể chỉ cần nâng version.
3. **Thêm key `security.*` vào `en.json`.** Cổng xanh lại, và tiếng Anh thôi
   không còn là ngôn ngữ duy nhất không có bản dịch.

Và một quyết định đang chặn thật: **Unresolved #1 — ai giữ khoá ký, ký ở đâu.**
Chưa có nó thì phase 3 không đóng được, và rule pack vẫn mãi là bản nhúng trong
binary không cập nhật được.

## Unresolved

1. **Khoá ký cho catalog + rule pack** — chặn phase 3 security-posture đóng lại.
2. **`dc1a315` có nên khôi phục không?** Object còn dangling; file còn trong
   working tree nên nội dung không mất. Nếu định commit lại cả cụm thì bỏ qua
   được; nếu không thì `git cherry-pick dc1a315` trước khi `git gc` dọn nó.
3. **Ai đang reset nhánh?** Commit biến mất là dấu hiệu có quy trình git chồng
   chéo giữa các phiên. Cần biết để 346 file kia không gặp chuyện tương tự.
4. **Phase 6/7 security-posture có làm không, và khi nào?** Chưa bắt đầu, đúng
   thứ tự, nhưng cũng chưa có mốc.
