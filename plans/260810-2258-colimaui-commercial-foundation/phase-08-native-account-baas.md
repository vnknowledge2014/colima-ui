---
phase: 8
title: "Native Account (BaaS OAuth)"
status: pending
priority: P2
effort: "5-8 ngày"
dependencies: [2]
---

# Phase 8: Native Account (BaaS OAuth)

> **Kế hoạch thực thi chi tiết:** `plans/260811-1815-native-account-supabase-oauth/`
> (5 phase: Supabase & callback → account state & sign-in → profile & badge →
> Keychain & entitlement boundary → verify). Phase này là mục nhập cấp cao; chi tiết
> build ở plan kia để tránh trùng lặp.

## Overview

Đăng nhập native trong app bằng Google/GitHub (giống OrbStack), qua một BaaS
auth. Đây là **ghi đè có ý thức** quyết định "không backend" — chi tiết + cái giá ở
mục "Ghi đè quyết định — Tài khoản" trong `plan.md`.

**Ranh giới quan trọng:** account **chỉ là danh tính**. Entitlement (đã trả tiền)
**vẫn là license key MoR** ở Phase 2. Phase này không được đưa việc "có phải Pro
không" đi qua account — làm thế sẽ tái tạo bài toán offline-first và version-skew mà
Phase 2/3 đã giải. Account down / mất mạng **không** hạ cấp khách trả tiền.

## Quyết định đã chốt — 2026-08-11

| | |
|---|---|
| **BaaS** | **Supabase Auth** (Q#6 resolved). Bậc free đủ; OAuth Google/GitHub + PKCE sẵn; SDK JS hợp desktop; self-host được |
| **Phương thức** | **OAuth Google + GitHub only.** "Signup" = lần đăng nhập đầu (Supabase tự tạo tài khoản). **Không** form email/password |
| **User badge** | **Chỉ danh tính** (avatar + tên). KHÔNG hiện trạng thái Pro — Pro nằm ở trang License, tránh ngụ ý "đăng nhập = Pro" |

## Điều kiện & phụ thuộc

- **Q#6 đã chốt: Supabase.** UI + account state code được ngay; cần bạn tạo project
  Supabase + đăng ký OAuth Google/GitHub để chạy live (xem Bước 1-2).
- `dependencies: [2]` — cần license-key entitlement tồn tại để account "gắn" vào,
  và để phần tự-lấy-key (tuỳ chọn) có cái để lấy.
- **Ưu tiên P2, không chặn launch.** Với entitlement ở key và Free không cần account,
  đăng nhập là tiện lợi/cảm giác native, không mở khoá tính năng. Không nằm trong cổng
  launch (Phase 6).

## Supabase — chốt kỹ thuật

- **Không secret trong app.** Chỉ nhúng `SUPABASE_URL` + `SUPABASE_ANON_KEY` (đều
  **công khai**, như `POLAR_ORG_ID`). Hằng số ở một chỗ (vd `src/lib/supabase.ts`).
- **Luồng OAuth desktop** (chặn cứng nhỏ, chốt khi build): Supabase mở trình duyệt
  (`signInWithOAuth`, PKCE) → sau auth redirect về app. Hai cách:
  - **(A) Deep link** `colimaui://auth-callback` qua `tauri-plugin-deep-link` — đăng
    ký scheme, thêm vào Supabase redirect allowlist. Sạch, chuẩn desktop.
  - **(B) Loopback** `http://localhost:<port>/auth-callback` — tái dùng HTTP server
    cục bộ đã có (`api_server.rs`); Supabase redirect về đó, app đổi code lấy session.
  - Khuyến nghị (A) nếu thêm plugin chấp nhận được; (B) nếu muốn tránh dep mới.
- Session (access/refresh token) lưu **Keychain** qua lệnh cầu nối Rust, không plaintext.
- Identity lấy từ `user.user_metadata`: `full_name`/`name`, `avatar_url`, `email`, provider.

## Requirements

**Functional**
- Đăng nhập Google + GitHub qua BaaS SDK. Luồng desktop: loopback redirect/PKCE hoặc
  device code — **không** nhúng client secret trong app.
- Đăng ký free không cần mua. Đăng xuất. Hiện trạng thái đăng nhập ở Settings.
- Màn đăng nhập **bỏ qua được, không chặn**; **không** đặt ở first-launch trước
  `SetupWizard`. Free chạy đầy đủ khi chưa đăng nhập.
- (Tuỳ chọn, tăng dần) Sau đăng nhập, **tự lấy license key** của người dùng thay vì
  dán tay — cần phần glue map BaaS-user ↔ MoR-customer (xem Rủi ro).

**Non-functional**
- Session token lưu an toàn (Keychain macOS), không để plaintext trên đĩa.
- **Không** đưa entitlement qua account: nếu BaaS/mạng down, app vẫn đọc license key
  cache (Phase 2) và giữ Pro.
- BYOK **không đổi**: auth danh tính không liên quan đường đi AI; không có gateway.

## Related Code Files

**Lớp account (core MIT):**
- Create: `src/lib/supabase.ts` — client Supabase + hằng số URL/anon key (công khai)
- Create: `src/lib/account.svelte.ts` — state `{ user: {name,email,avatar,provider} | null, loaded }`;
  `signIn(provider)`, `signOut()`, `loadSession()`, `isSignedIn()`. Chỉ đọc ở UI.

**UI — login / profile / badge:**
- Create: `src/components/account/SignInPanel.svelte` — 2 nút "Continue with Google/GitHub"
  + nút "Bỏ qua". **Không chặn.** Dùng cho cả login lẫn signup (OAuth: signup = login đầu)
- Create: `src/pages/settings/Account.svelte` — **profile**: avatar, tên, email, provider;
  nút "Đăng xuất"; "Quản lý tài khoản" → portal Supabase (`openExternal`)
- Create: `src/components/account/UserBadge.svelte` — badge nhỏ ở **footer sidebar**:
  avatar + tên (thu gọn còn avatar khi `sidebar.collapsed`); chưa đăng nhập → nút
  "Sign in" mở `SignInPanel`. **Chỉ danh tính, không dot Pro.**
- Modify: `src/components/Sidebar.svelte` — chèn `UserBadge` vào khối footer
- Modify: `src/pages/Settings.svelte` — thêm `Account` (đặt cạnh `License`)
- Modify: `src/App.svelte` — `loadSession()` lúc khởi động (cạnh `loadProStatus`/`loadLicense`)
- Modify: `src/locales/*` (4 locale) — chuỗi `account.*`

**Backend (Keychain + callback):**
- Modify: `src-tauri/` — lệnh cầu nối lưu/đọc session token qua Keychain
- Deep-link plugin **hoặc** route callback trong `api_server.rs` (theo cách A/B đã chốt ở trên)

**Cấu hình (việc của bạn):**
- Project Supabase + bật provider Google + GitHub + thêm redirect URL (deep-link/loopback)

## Implementation Steps

1. **Tạo project Supabase**, bật Google + GitHub provider, thêm redirect URL desktop.
   Lấy `SUPABASE_URL` + anon key (công khai) → điền `src/lib/supabase.ts`.
2. Chọn luồng redirect **(A) deep-link** hay **(B) loopback** (xem "Supabase — chốt
   kỹ thuật"); dựng phần callback tương ứng.
3. `supabase.ts` + `account.svelte.ts` — bọc SDK: `signInWithOAuth(provider, PKCE)`,
   `signOut`, `getSession`, map `user_metadata` → `{name,email,avatar,provider}`.
4. `SignInPanel.svelte` — 2 nút provider + "Bỏ qua". Không chặn. Mở được từ badge + Settings.
5. `Account.svelte` (profile) trong Settings — avatar/tên/email/provider, đăng xuất,
   "Quản lý tài khoản" → portal Supabase.
6. `UserBadge.svelte` — footer sidebar; đã đăng nhập: avatar+tên; chưa: "Sign in";
   collapsed-aware. Wire vào `Sidebar.svelte`.
7. Lưu session token qua **Keychain** (lệnh Rust); không plaintext.
8. `loadSession()` ở `App.svelte` lúc khởi động; i18n `account.*` × 4.
9. **Kiểm ranh giới entitlement:** tắt mạng/đăng xuất → license key vẫn giữ Pro. Test.
10. (Tuỳ chọn) Glue tự lấy key: map Supabase-user ↔ Polar-customer, kéo key sau đăng
    nhập. Đánh giá riêng — đây là nơi phát sinh thêm hạ tầng (backend nhỏ).

## Tests / Validation

- Thủ công: đăng nhập Google + GitHub thành công; đăng xuất; skip vẫn dùng Free đầy đủ.
- **Ranh giới (quan trọng nhất):** đăng xuất / BaaS down / mất mạng → khách có license
  key **vẫn là Pro**. Entitlement không đi qua account.
- Bảo mật: token trong Keychain, không plaintext trên đĩa; không có client secret trong bundle.
- First-launch: `SetupWizard` chạy trước, không có màn login chặn.
- Browser mode: quyết định rõ đăng nhập có hoạt động không; nếu không thì nói rõ trong UI.

## Success Criteria

- [ ] Đăng nhập Google + GitHub native trong app, qua Supabase, không secret trong bundle.
- [ ] Skip được; Free đầy đủ khi chưa đăng nhập; không màn login chặn ở first-launch.
- [ ] **User badge** ở footer sidebar: đã đăng nhập hiện avatar+tên; chưa thì "Sign in";
      collapsed hiện avatar. Chỉ danh tính, không dot Pro.
- [ ] **Trang Profile** (Settings → Account): avatar/tên/email/provider, đăng xuất, link portal.
- [ ] **Entitlement vẫn ở license key MoR** — có test chứng minh account down không hạ cấp Pro.
- [ ] Token lưu an toàn (Keychain).
- [ ] BYOK không đổi; không có gateway AI mới.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Vô tình đưa entitlement qua account | Cao | Ranh giới cứng: Pro chỉ đọc từ license key; test account-down-vẫn-Pro |
| Màn login thành rào cản, xói mòn "không bắt buộc tài khoản" | Cao | Bỏ qua được, không ở first-launch, không chặn Free |
| Glue BaaS-user ↔ MoR-customer kéo theo backend thật | TB | Để ở bước 8 tuỳ chọn; đánh giá riêng trước khi làm |
| Phụ thuộc + phí BaaS | TB | Chọn BaaS có bậc free đủ dùng lúc đầu (Q#6) |
| OAuth desktop phức tạp (redirect) | TB | Dùng SDK BaaS hỗ trợ desktop sẵn; loopback/PKCE hoặc device code |
| Lộ client secret | Cao | Không nhúng secret trong app; BaaS giữ; chỉ dùng public/anon key + PKCE |
