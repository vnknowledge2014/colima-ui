---
phase: 2
title: "Billing & License via MoR"
status: in_progress
priority: P1
effort: "4-6 ngày"
dependencies: [1]
---

# Phase 2: Billing & License via MoR

## Overview

Thanh toán + cấp phép, **dùng hạ tầng có sẵn của Merchant of Record** thay vì tự xây.

Bản v1 dự tính 10-13 ngày cho: license server, ký Ed25519, fingerprint máy, activation/deactivation, refresh, grace period, hàng đợi retry webhook, gửi email. Red-team chỉ ra toàn bộ danh sách này là tính năng tiêu chuẩn của các MoR hiện đại — và tự xây mâu thuẫn với chính nguyên tắc "license không phải DRM" của plan.

## Requirements

**Functional**
- Checkout do MoR host; không chạm dữ liệu thẻ.
- App có nút "Quản lý gói / Hoá đơn" **link ra portal MoR** (billing, hoá đơn,
  lấy lại key). Đây là billing, **không** phải đăng nhập danh tính — OAuth native là
  Phase 8, tách riêng.
- **Đa máy:** cùng một license key kích hoạt được trên nhiều máy tới giới hạn
  activation của MoR. App hiện số máy đã dùng / giới hạn khi kích hoạt.
- **Ranh giới với Phase 8:** entitlement (đã trả tiền) là **license key này**, không
  phải account. Account danh tính (Phase 8) không được đứng giữa việc kiểm Pro.
- License key do MoR phát hành và quản lý (bao gồm: cấp, thu hồi khi huỷ/hoàn tiền, giới hạn số máy, cổng khách hàng tự phục vụ).
- App xác thực key qua API của MoR khi kích hoạt, rồi **cache trạng thái cục bộ** để chạy offline.
- Đường phục hồi khi khách trả tiền mà không nhận được key: trang "lấy lại key" bằng email.
- Hết hạn → quay về Free an toàn, không mất dữ liệu.

**Non-functional**
- **Offline-first:** sau kích hoạt, mất mạng không được chặn người dùng. Đây là điều kiện không thương lượng.
- Ghi trạng thái license phải atomic (tmp + fsync + rename) + có `.bak` — hỏng file không được biến khách trả tiền thành Free.
- Không mâu thuẫn nội tại: nếu chọn "offline vô thời hạn" thì đừng đồng thời đặt refresh 7 ngày + grace 14 ngày (v1 mắc lỗi này — cộng lại thành khoá cửa sau 21 ngày).

## Polar — chốt kỹ thuật (2026-08-11)

Tra `api.polar.sh/openapi.json` (version 2026-04). Phát hiện thay đổi kiến trúc theo
hướng đơn giản hơn hẳn:

- **Validate/activate KHÔNG cần secret API key.** Endpoint `customer-portal`:
  - `POST /v1/customer-portal/license-keys/activate` — body `{key, organization_id, label}` → trả `{id (=activation_id), license_key:{...}}`
  - `POST /v1/customer-portal/license-keys/validate` — body `{key, organization_id, activation_id?, conditions?}` → trả `ValidatedLicenseKey {status, expires_at, limit_activations, validations, ...}`
  - `POST /v1/customer-portal/license-keys/deactivate` — body `{key, organization_id, activation_id}`
  - Không header Authorization. Chỉ cần **`organization_id` (công khai)** + license key của người dùng.
- **Hệ quả kiến trúc:** entitlement client sống được trong **core MIT**, không cần
  sidecar, không cần backend giữ secret. `organization_id` là hằng số công khai
  (như `PRICING_URL`). Điều này **mở khoá Phase 2 khỏi phụ thuộc sidecar/Phase 3**.
- **Ranh giới licensing (Nguyên tắc 4):** đặt check trong core MIT nghĩa là crack được
  bằng patch — chấp nhận, vì tính năng Pro thật vẫn ở sidecar (không có trong bundle
  Free). Check chỉ là hàng rào lịch sự.
- **Sandbox:** `https://sandbox-api.polar.sh` cho test-mode; `https://api.polar.sh` prod.
- **Mô hình thời hạn: (A)** — trust `expires_at` nhúng trong key; activate+validate 1 lần
  (cần mạng), cache lại, chạy offline tới `expires_at`. Re-validate cơ hội khi online
  nhưng **không chặn**. `status != "granted"` (revoke/refund) chỉ bắt được khi re-validate
  — chấp nhận theo Nguyên tắc 4.

## Kiến trúc

```
Người dùng bấm "Nâng cấp Pro" → mở trình duyệt (tauri_plugin_opener đã có)
        ▼
Checkout MoR (hosted) → MoR phát key + gửi email + quản lý subscription
        ▼
Người dùng dán key vào Settings → License
        ▼
colima-ui-pro (sidecar)
  ├─ validate key qua API MoR (lần đầu, cần mạng)
  ├─ ghi trạng thái cục bộ: atomic + .bak, quyền 0600
  └─ kiểm tra lại định kỳ; MẤT MẠNG KHÔNG CHẶN NGƯỜI DÙNG
```

**Chốt mô hình thời hạn** (chọn 1, ghi vào code comment, không để mơ hồ):
- (A) Key có hạn nhúng sẵn → offline hoàn toàn tới ngày hết hạn, không cần refresh.
- (B) Kiểm tra định kỳ + grace → phải nói rõ tổng thời gian offline tối đa là bao nhiêu.

Khuyến nghị (A) — đơn giản và đúng tinh thần offline-first.

## Related Code Files

**Repo private (`colima-ui-pro`):**
- Create: `src/license/{validate,store,state}.rs` — mỏng, chủ yếu gọi API MoR + cache
- Create: test với key mẫu của môi trường test MoR

**Core MIT:**
- Create: `src/lib/license.svelte.ts` — store chỉ đọc
- Create: `src/pages/settings/License.svelte` — nhập key, trạng thái, hết hạn
- Reuse: `src/components/UpgradeDialog.svelte` (đã tạo ở scaffolding Phase 3)
- Reuse: `src/lib/external-links.ts` `openExternal()` (đã có) cho link portal MoR
- Modify: `src/pages/Settings.svelte`, `src/locales/*`

**Website:**
- Create: trang "lấy lại key" bằng email

## Implementation Steps

1. **Chốt MoR** (Unresolved Question #1). Tiêu chí: có license key API, hỗ trợ pháp nhân VN, phí hợp lý, có cổng khách hàng.
2. **Chốt mô hình thời hạn (A) hay (B)** và ghi lại. Không code trước khi chốt — v1 hỏng vì bỏ qua bước này.
3. Tạo sản phẩm + giá tháng/năm trên MoR (test mode).
4. `validate.rs` — gọi API validate của MoR; xử lý mọi mã lỗi mạng bằng cách **giữ nguyên trạng thái cache**, không hạ cấp.
5. `store.rs` — ghi atomic (tmp + fsync + rename), `.bak`, quyền 0600. Hỏng file → thử `.bak` trước khi kết luận Free.
6. `License.svelte` — nhập key, hiện trạng thái + ngày hết hạn, thông báo rõ ràng khi sắp hết hạn.
7. `UpgradeDialog.svelte` — nói rõ Pro có gì, mở checkout.
8. Trang "lấy lại key": nhập email → MoR gửi lại. Thêm link này ngay trong app.
9. Thêm nút "Quản lý gói / Hoá đơn" trong `License.svelte` → mở portal MoR (dùng
   `openExternal` đã có). Đây là billing, không phải đăng nhập (OAuth ở Phase 8).
10. Kích hoạt đa máy: hiện "máy X / Y" từ API MoR; xử lý trường hợp vượt giới hạn bằng
    thông báo rõ (huỷ kích hoạt máy cũ ở portal), không im lặng hạ cấp.
11. E2E test mode: mua → nhận mail → kích hoạt → **tắt mạng, restart, vẫn Pro** → huỷ → về Free.
12. Chuyển production khi MoR duyệt; mua thật 1 đơn, rồi hoàn tiền để kiểm tra đường thu hồi.

## Tests / Validation

- Unit: máy trạng thái license, đặc biệt đường xuống Free.
- Unit: ghi atomic — kill giữa chừng (giả lập) không làm mất key.
- Unit: mạng lỗi → giữ nguyên trạng thái, **không** hạ cấp.
- E2E test mode: đủ vòng đời mua → dùng → huỷ.
- **Thủ công bắt buộc:** kích hoạt → ngắt mạng → restart app → vẫn Pro.
- Thủ công: xoá file license → app về Free, không crash, có hướng dẫn khôi phục.

## Success Criteria

- [ ] MoR đã chốt và duyệt production.
- [ ] Không tự xây license server.
- [ ] Kích hoạt xong, **offline không bị chặn** (kiểm chứng thủ công).
- [ ] Ghi trạng thái atomic + `.bak`; hỏng file không biến khách trả tiền thành Free.
- [ ] Mô hình thời hạn được chốt và ghi rõ, không mâu thuẫn nội tại.
- [ ] Có đường "lấy lại key" ngay trong app.
- [ ] Huỷ/hoàn tiền → về Free sạch, không mất dữ liệu.

## Tiến độ — 2026-08-11

**License client + UI đã build trong core MIT (không cần secret, không cần sidecar).**

Đã làm:

- [x] `src-tauri/src/license/client.rs` — activate/validate/deactivate qua Polar
      customer-portal (sandbox mặc định; `POLAR_ENV=production` để đổi). Không header auth.
- [x] `src-tauri/src/license/cache.rs` — ghi atomic (tmp+fsync+rename) + `.bak` +
      0600; **offline-first** `is_entitled_at()` (granted + chưa hết hạn, fail-closed).
- [x] `src-tauri/src/license/mod.rs` — `organization_id()` (const/env), mô hình (A);
      commands `license_activate/state/refresh/deactivate` + REST routes.
- [x] `src/pages/settings/License.svelte` + `api/license.ts` — nhập key, trạng thái,
      hết hạn, refresh, huỷ kích hoạt. i18n 4 ngôn ngữ. Lắp vào Settings.
- [x] 7 test Rust (offline-first: granted/expired/revoked/no-expiry/unparseable + state).
- [x] Thêm dep `chrono` (đã có transitively) để parse expiry an toàn.

**Cần bạn để verify live (không dán vào chat):**

- [ ] Tạo Polar **sandbox** org + product + License Key benefit (bật activation limit).
- [ ] Đặt `POLAR_ORG_ID=<org id công khai>` (env dev, hoặc điền hằng số `POLAR_ORG_ID`
      trong `license/mod.rs`).
- [ ] Một license key test → dán vào Settings → License để chạy activate/validate thật.

- [x] Bề mặt mua hàng: `CHECKOUT_URL` + `PORTAL_URL` (hằng số placeholder, fallback
      về pricing). `UpgradeDialog` có nút "Get Pro" → checkout. `License.svelte` có
      "Buy Pro" (chưa activate) và "Quản lý / hoá đơn" → portal (đã activate). i18n × 4.

**Chưa làm:**

- [ ] Điền `CHECKOUT_URL`/`PORTAL_URL` thật khi product Polar live (một dòng mỗi cái).
- [ ] Hiển thị "máy X/Y" từ API MoR (bước 10) — cần org_id + key test để verify.
- [ ] Hợp nhất với `ProStatus`: hiện license entitlement (core/Polar) và capability
      (sidecar) là hai nguồn tách. Merge "đã trả tiền ∧ có capability" khi sidecar ship.

### Quyết định lệch so với chữ plan

**License client ở core MIT, không phải sidecar.** Plan v2 đặt `license/*` trong repo
private vì giả định cần secret. Docs Polar cho thấy validate/activate **không cần
secret** — nên đặt ở core, mở khoá Phase 2 khỏi phụ thuộc sidecar/Phase 3. Ranh giới
Pro thật vẫn ở sidecar; check license là hàng rào lịch sự (Nguyên tắc 4).

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| MoR duyệt chậm (pháp nhân VN) | Cao | Nộp hồ sơ từ Phase 1; có nhà cung cấp dự phòng |
| Phụ thuộc API MoR — họ down thì sao? | Cao | Cache cục bộ là nguồn sự thật lúc chạy; API chỉ dùng khi kích hoạt |
| Khách trả tiền mà không nhận key | Cao | Trang lấy lại key + link ngay trong app; đây là nguyên nhân chargeback phổ biến nhất |
| Ghi file license hỏng làm mất quyền | Cao | Atomic + `.bak` + thử khôi phục trước khi kết luận |
| Mô hình thời hạn mơ hồ như v1 | TB | Bước 2 là cổng bắt buộc |
