---
phase: 4
title: "Telemetry & Auto-Update"
status: in_progress
priority: P1
effort: "4-5 ngày"
dependencies: [3]
---

# Phase 4: Telemetry & Auto-Update

## Overview

Hai hạ tầng bắt buộc trước khi bán rộng: đo được hành vi người dùng, và cập nhật được bản vá.

**Phụ thuộc Phase 3** (khác với v1 cho chạy song song): auto-update thay bundle, nên nó phải biết mô hình phân phối sidecar. Ship updater trước khi giải quyết version skew chính là cách tạo ra sự cố "khách trả tiền bị hạ cấp".

Repo đã có `.github/workflows/{ci,promote,release}.yml` — mở rộng, không dựng lại. **Chưa có** hạ tầng ký/notarize — phải làm mới, đã tính vào ước lượng.

## Requirements

**Functional — Telemetry**
- Mặc định **TẮT**. Hỏi 1 lần, mặc định từ chối, không hỏi lại nếu đã từ chối.
- Sự kiện: khởi động, phiên bản, OS + kiến trúc, mốc activation (container đầu tiên), tính năng dùng, lỗi theo `code` (từ hợp đồng lỗi v2), chạm ProGate, mở checkout.
- **Không** thu: tên container/image/volume, đường dẫn, biến môi trường, nội dung chat AI, API key, IP.
- Xem được payload thô trong app trước khi đồng ý.

**Functional — Crash reporting**
- Bắt panic Rust + lỗi JS chưa xử lý.
- **Bắt buộc đi qua `redact()`** của plan hotfix bảo mật. Đây là điểm v1 sai: v1 cho rằng enum đóng đảm bảo không có PII, nhưng crash report mang **chuỗi panic và stack trace** — chính xác là nơi chứa đường dẫn, tên tài nguyên, và (như `ai_chat.rs:180` cho thấy) cả URL kèm API key.
- Opt-in **riêng**, tách khỏi telemetry.

**Functional — Auto-update**
- Kênh `stable` và `beta`.
- Bản cập nhật phải được ký; từ chối nếu chữ ký sai.
- **Xử lý sidecar Pro** theo mô hình phân phối đã chốt ở Phase 3.
- Xem changelog trước khi cập nhật.

**Non-functional**
- Telemetry gửi theo lô, không chặn UI, thất bại thì im lặng.
- Định danh: UUID ngẫu nhiên cục bộ, **không** liên kết license, **không** phải fingerprint máy.

## Architecture

```
src-tauri/src/telemetry/ (trong repo MIT — để ai cũng kiểm tra được)
  ├─ consent.rs
  ├─ events.rs    — enum ĐÓNG, không nhận chuỗi tự do
  ├─ queue.rs     — hàng đợi có giới hạn, gửi lô
  └─ sink.rs

src-tauri/src/crash.rs — panic hook → redact() → gửi
src/lib/crashReporter.ts — window.onerror / unhandledrejection → redact

Auto-update: tauri-plugin-updater
  ├─ manifest theo kênh, public key nhúng
  └─ phối hợp với mô hình phân phối sidecar (Phase 3)
```

Enum đóng ở `events.rs` là biện pháp kỹ thuật chặn rò rỉ PII từ gốc — mạnh hơn cam kết bằng lời. Nhưng nó **chỉ phủ telemetry**, không phủ crash report; đó là lý do crash phải qua redact.

## Related Code Files

- Create: `src-tauri/src/telemetry/{mod,consent,events,queue,sink}.rs`
- Create: `src-tauri/src/crash.rs`
- Create: `src/lib/crashReporter.ts`
- Create: `src/components/settings/PrivacySettings.svelte`, `src/components/ConsentDialog.svelte`, `src/components/settings/UpdateSettings.svelte`
- Modify: `src-tauri/Cargo.toml` — `tauri-plugin-updater`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Modify: `.github/workflows/release.yml` — build + **ký + notarize** (mới) + manifest theo kênh
- Modify: `.github/workflows/promote.yml` — beta → stable
- Create: `docs/telemetry.md`
- Modify: `src/components/GettingStartedTour.svelte` — chèn bước hỏi đồng ý
- Modify: `src/locales/*` (4 locale)

## Implementation Steps

1. Viết `docs/telemetry.md` **trước khi code**. Sự kiện nào khó giải thích công khai thì không thu.
2. `events.rs` enum đóng khớp chính xác tài liệu.
3. `consent.rs` + `ConsentDialog.svelte` — mặc định từ chối.
4. `queue.rs` + `sink.rs` — gom lô, giới hạn, drop khi đầy, không retry vô hạn.
5. `PrivacySettings.svelte` — bật/tắt + nút "Xem dữ liệu sẽ gửi" hiện JSON thật.
6. `crash.rs` + `crashReporter.ts` — **redact trước khi gửi**, kể cả chuỗi panic và stack.
7. Dựng hạ tầng ký + notarize macOS (Apple Developer ID) — chưa từng có, cần thời gian setup.
8. Cài `tauri-plugin-updater`, sinh cặp khoá ký cập nhật, nhúng public key.
9. Mở rộng `release.yml`: build → ký → notarize → manifest theo kênh → upload. **Bao gồm cả sidecar Pro** theo mô hình Phase 3.
10. `UpdateSettings.svelte`: chọn kênh, kiểm tra ngay, changelog.
11. Test cập nhật thật: bản cũ → bản mới; manifest ký sai bị từ chối; **core+sidecar cập nhật đồng bộ**.

## Tests / Validation

- Unit: sự kiện không thể chứa chuỗi tự do (test này là hàng rào PII).
- Unit: `redact()` áp dụng cho crash payload — test với panic chứa API key giả.
- Unit: consent mặc định tắt, không hỏi lại sau khi từ chối.
- **Thủ công bắt buộc:** chưa đồng ý → bắt gói mạng, xác nhận **0 request telemetry**.
- Thủ công: cập nhật ở cả 2 kênh; manifest ký sai bị từ chối.
- **Thủ công:** cập nhật xong, người dùng Pro vẫn là Pro (không skew).

## Success Criteria

- [ ] `docs/telemetry.md` công khai, khớp chính xác `events.rs`.
- [ ] Chưa đồng ý → không request telemetry nào (kiểm chứng bằng bắt gói).
- [ ] Crash report đi qua `redact()`, có test với payload chứa key giả.
- [ ] Sự kiện telemetry không thể chứa chuỗi tự do (enforce ở kiểu dữ liệu).
- [ ] Ký + notarize hoạt động; auto-update từ chối bản không hợp lệ.
- [ ] Cập nhật không gây version skew core/sidecar.
- [ ] `release.yml` sinh manifest tự động theo kênh.

## Tiến độ — 2026-08-11

**Telemetry/crash core (không endpoint, không ký) đã xong. Auto-update hoãn.**

Đã làm:

- [x] `docs/telemetry.md` — danh mục sự kiện + tư thế riêng tư, khớp `events.rs`.
- [x] `src-tauri/src/telemetry/events.rs` — enum `TelemetryEvent` **đóng**, không
      field chuỗi tự do. `Error` mang `ErrorCode` chứ không mang message. Đây là rào
      PII ở tầng kiểu.
- [x] `src-tauri/src/telemetry/consent.rs` — 3 trạng thái (unset/declined/granted),
      **mặc định không ghi**, không hỏi lại sau khi từ chối. Lưu qua settings.
- [x] `src-tauri/src/telemetry/mod.rs` — `record()` kiểm consent mỗi lần; buffer
      trong-bộ-nhớ có giới hạn (không network); command consent + preview.
- [x] `src-tauri/src/crash.rs` — panic hook **redact trước khi log** (red-team #8).
      Test với API key + bearer token giả trong panic.
- [x] `src/lib/crashReporter.ts` — window.onerror/unhandledrejection → redact.
- [x] `ConsentDialog.svelte` (hỏi 1 lần) + `settings/PrivacySettings.svelte` (bật/tắt
      + "Xem dữ liệu sẽ gửi"). Mount ở App/Settings. i18n 4 ngôn ngữ.
- [x] REST routes `/api/telemetry/*` cho browser mode.
- [x] **Đo phễu** (Phase 6 yêu cầu): record `AppStarted` lúc startup, `FeatureUsed
      (ComposeDiagnose)` trong command, `ProGateReached` khi ProGate hiện locked,
      `CheckoutOpened` khi mở checkout. Command nhận **enum** (`Feature`/
      `GatedCapability`) — rào PII giữ nguyên; frontend map dotted→snake (2 mục).
      Gated by consent, best-effort, Tauri-only. Populate buffer preview thật.
- [x] 11 test Rust (events/consent/buffer/crash-redact).
- [x] `ErrorCode` thêm `Deserialize` (non-breaking) để `TelemetryEvent` đọc ngược được.

**Auto-update logic đã build (chừa bước ký Apple):**

- [x] `tauri-plugin-updater` (crate + npm) + init trong `lib.rs` + permission
      `updater:default` trong capability.
- [x] `tauri.conf.json` — `plugins.updater` (endpoint GitHub releases `latest.json`
      + **pubkey updater dev** đã sinh); `bundle.createUpdaterArtifacts: true`.
- [x] Cặp khoá **updater của Tauri** (ed25519, **khác Apple codesign**) sinh bằng
      `tauri signer generate --ci`; private key ở `.tauri-updater-dev.key` **đã gitignore**.
- [x] `UpdateSettings.svelte` — kiểm tra cập nhật, hiện phiên bản + changelog,
      tải & cài; guard browser mode; lỗi/không có bản → báo mượt. i18n × 4. Lắp Settings.

**Hoãn (cần hạ tầng ngoài / Apple):**

- [ ] **Ký + notarize macOS (Apple Developer ID)** — chủ dự án **chưa có tài khoản**.
      Đây là chặn cứng để phân phối bản trả phí không cảnh báo Gatekeeper.
- [ ] Sinh **updater key production** (regenerate; private key vào CI secret, không
      dùng key dev đang commit pubkey). Thay pubkey trong config.
- [ ] `release.yml`/`promote.yml` — build → **ký updater** → (khi có Apple) notarize →
      sinh `latest.json` theo kênh → upload GitHub releases. Gói cả sidecar Pro.
- [ ] Kênh beta — endpoint manifest thứ hai (bỏ selector vòng này, YAGNI: chưa có infra).
- [ ] Sink mạng telemetry — chưa có endpoint nhận (như phần telemetry ở trên).

### Ghi chú: updater signing ≠ Apple signing

Hai chữ ký hoàn toàn khác nhau, dễ nhầm:
- **Tauri updater signature** (ed25519): để app tin bản cập nhật tải về đúng nguồn.
  **Không cần Apple.** Đã set up (key dev; regenerate cho prod).
- **Apple codesign + notarize**: để macOS Gatekeeper không chặn app. **Cần Apple
  Developer ID ($99/năm).** Chưa làm — chờ tài khoản.

### Quyết định lệch so với chữ plan

**Không xây `queue.rs`/`sink.rs` vòng này.** Đó là lớp mạng, mà chưa có nơi gửi.
Xây một hàng đợi gửi tới hư vô là speculative. Thay bằng buffer trong-bộ-nhớ có giới
hạn, backing cho preview "xem dữ liệu sẽ gửi". Khi có endpoint thì sink đọc buffer này.

**Auto-update tách hẳn khỏi vòng này** vì phụ thuộc Apple Developer ID (chưa có) —
đúng như plan đã ghi "chưa có hạ tầng ký/notarize". Không giả lập.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| **Crash report rò rỉ API key** (v1 bỏ sót) | Cao | Bắt buộc redact; test với key giả trong panic |
| **Auto-update đẩy bản hỏng cho tất cả** | Cao | Kênh beta bắt buộc trước stable; dừng phát hành được; giữ bản trước để lùi |
| Cập nhật gây version skew | Cao | Phối hợp với Phase 3; test riêng |
| Khoá ký bị lộ | Cao | GitHub secrets, không vào repo; có kế hoạch xoay khoá |
| Setup notarize Apple tốn thời gian ngoài dự kiến | TB | Bắt đầu bước 7 sớm, song song các bước khác |
| Tỉ lệ đồng ý telemetry thấp | TB | Chấp nhận; bổ sung bằng phỏng vấn người dùng và số liệu tải về |
