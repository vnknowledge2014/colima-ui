---
phase: 3
title: "Pro Boundary"
status: in_progress
priority: P1
effort: "5-7 ngày"
dependencies: [2]
---

# Phase 3: Pro Boundary

## Overview

Ranh giới giữa core MIT và tính năng Pro: repo private, tiến trình riêng, giao tiếp qua IPC.

## Câu hỏi phân phối — ĐÃ CHỐT (2026-08-11)

**Unresolved Question #3 đã trả lời: sidecar Pro ship KÈM BUNDLE, không tải riêng.**

Chủ dự án chốt bundle-kèm sau khi cân với phương án tải-riêng. Vì sao bundle-kèm:

- Auto-update (Phase 4) thay **toàn bộ bundle** → core và sidecar **luôn cùng
  version theo thiết kế**. Đây là cách triệt tiêu rủi ro Critical #2 tại gốc, thay
  vì dựng rào chắn quanh nó.
- `src-tauri/Cargo.toml` hiện chưa có `externalBin`; bundle-kèm nghĩa là **thêm
  `externalBin`** trỏ tới binary Pro đã ký, để Tauri gói cùng và ký cùng.
- Trên macOS, binary nằm trong bundle đã notarize chạy trơn; binary tải rời phải
  tự notarize và dễ bị Gatekeeper chất vấn. Bundle-kèm né hẳn lớp phức tạp đó.

**`cached_state.rs` vẫn giữ nguyên** dù đã bundle-kèm: nó là lưới an toàn cho trường
hợp sidecar bị người dùng xoá, bị AV cách ly, hay crash lúc chạy. Core phải luôn
biết người này đã trả tiền và hiện "cần khôi phục thành phần Pro" thay vì hạ cấp im
lặng thành Free.

## Mô hình bảo vệ Pro — GIỮ NGUYÊN TẮC 4

Chủ dự án đã cân nhắc obfuscation/anti-deobfuscation và **quyết định không làm**,
đúng Nguyên tắc 4 ("License là hàng rào lịch sự, không phải DRM"). Ghi lại lý do để
về sau không ai tưởng là thiếu sót:

- Rust binary **không chống dịch ngược được** — Ghidra/IDA phục hồi logic; người
  crack chỉ cần patch một lệnh nhảy ở chỗ check license.
- Obfuscation nặng **phá notarization** của Apple và làm AV gắn cờ false-positive.
- Với sản phẩm $6/tháng, người bỏ công crack gần như chắc chắn **không phải khách
  hàng tiềm năng** — chống crack không cứu được doanh thu nào.

**Cái thật sự bảo vệ Pro, xếp theo hiệu quả:**

1. **Không bao giờ phát source Pro.** Repo `colima-ui-pro` private; chỉ ship binary
   đã compile. Đây là ~90% giá trị bảo vệ — crack binary khó hơn đọc source nhiều lần.
2. **Giá trị nằm ở phần không nằm hết trong binary.** Flagship compose auto-fix dựa
   vào KB + chẩn đoán; crack binary không cho kẻ tấn công dữ liệu đó.
3. **License verify qua MoR (Phase 2).** Hàng rào lịch sự. Ai vượt được thì kệ.

**KHÔNG làm trong plan này:** obfuscation, control-flow flattening, anti-debug,
dynamic code loading (tải dylib/module từ zip rồi chạy). Cái cuối vừa bị Gatekeeper
chặn vừa trông như malware — xem "Đã bác" bên dưới.

### Đã bác — dynamic module loading

Phương án "download package từ zip → chạy module ngầm" đã được cân và **bác**:

- Trên macOS, dylib tải rời không đi qua bundle đã ký; Hardened Runtime (bắt buộc để
  notarize) mặc định cấm nạp code chưa ký. Muốn chạy phải tắt library-validation —
  đúng cờ mà rà soát bảo mật sẽ soi.
- Pattern "tải payload, chạy ngầm, tránh soi" là định nghĩa giáo khoa của
  loader/dropper malware. Sản phẩm thương mại không nên trông như vậy.
- Nó **tái tạo đúng rủi ro Critical #2** (version skew) mà bundle-kèm vừa triệt tiêu.

## Sửa lại từ v1

| v1 | Vấn đề | v2 |
|---|---|---|
| `src-tauri/src/pro/registry.rs` trong core MIT | Registry chứa danh sách + ngữ nghĩa capability Pro → logic Pro nằm trong repo MIT, mâu thuẫn ranh giới | Core chỉ có **client gọi capability**. Sidecar tự khai báo capability lúc handshake; core không hardcode gì |
| Sidecar định vị qua biến môi trường | `path_util.rs:142-163` và `instance_reader.rs:52` cho thấy repo đã có tiền lệ tin PATH/env. Env var trỏ sidecar = cho phép thực thi mã tuỳ ý trong tiến trình được tin cậy | Env override **chỉ trong `debug_assertions`**; production chỉ tìm cạnh executable + verify chữ ký |
| "Socket 0600 là ranh giới bảo mật" | 0600 không phải xác thực peer; và trên máy này bearer token của API server mới là ranh giới thật | Thêm nonce lúc spawn + kiểm tra peer; nói đúng mức độ đảm bảo, không thổi phồng |
| Làm Phase 1, trước cả thanh toán | Xây IPC ở doanh thu $0 mà chưa biết payload thật trông thế nào | Làm sau Phase 2; protocol thiết kế dựa trên payload thật của compose auto-fix |

## Requirements

**Functional**
- Core phát hiện sidecar; không có thì chạy đầy đủ Free, không lỗi, không nag chặn.
- Handshake có version; lệch version → thông báo cụ thể cần cập nhật bên nào, **và giữ nguyên nhận diện khách trả tiền**.
- Sidecar chết → core sống, tính năng Pro degrade có thông báo, tự thử lại có giới hạn.
- Core cache trạng thái license đã ký để sống sót qua version skew.

**Non-functional**
- IPC qua Unix domain socket quyền 0600 + nonce lúc spawn.
- Sidecar chạy quyền người dùng, không cần root.
- Overhead khởi động <200ms khi có sidecar, ~0 khi không.
- **Browser mode:** phải quyết định rõ Pro có hoạt động ở browser mode không. v1 bỏ sót hoàn toàn điểm này.

## Architecture

```
colima-ui (MIT)
  src-tauri/src/pro/
    ├─ bridge.rs        — IPC client, spawn + handshake + health
    ├─ protocol.rs      — message + version (MIT, để repo private import lại)
    └─ cached_state.rs  — trạng thái license đã ký, sống sót khi sidecar vắng
    ── KHÔNG có registry.rs, KHÔNG có logic Pro ──
                │ Unix socket 0600 + nonce
                ▼
colima-ui-pro (private repo; build ra binary, gói vào bundle qua externalBin,
               ký + notarize CÙNG app — không ký rời)
  ├─ khai báo capability lúc handshake
  ├─ license validate (Phase 2)
  └─ tính năng Pro (Phase 5-6)
```

## Related Code Files

**Core MIT:**
- Create: `src-tauri/src/pro/{mod,bridge,protocol,cached_state}.rs`
- Modify: `src-tauri/src/lib.rs` — khởi động bridge trong `setup()`
- Modify: `src-tauri/tauri.conf.json` — thêm `externalBin` trỏ tới binary Pro (đã chốt bundle-kèm)
- Create: `src/lib/pro.svelte.ts`, `src/components/ProGate.svelte`
- Create: `docs/pro-boundary.md` — tài liệu công khai giải thích mô hình

**Repo private:**
- Create: skeleton binary + CI build xuất artifact cho macOS/Linux (ký ở bước bundle của core, không ký rời)

## Implementation Steps

1. Viết `docs/pro-boundary.md` — ranh giới pháp lý + mô hình phân phối (bundle-kèm, đã chốt) + mô hình bảo vệ source (giữ Nguyên tắc 4), công khai được.
2. Định nghĩa `protocol.rs` tối giản, có version. Thiết kế dựa trên payload thật của compose auto-fix (Phase 5), không thiết kế trừu tượng.
3. `cached_state.rs` — core lưu trạng thái license đã ký; **đây là thứ chống version skew**, làm trước bridge.
4. `bridge.rs` — spawn sidecar từ đường dẫn `externalBin` cạnh executable trong bundle; env override chỉ debug; nonce, handshake, health check, retry có giới hạn.
5. Tạo repo private; sidecar tối thiểu khai báo 1 capability giả để chứng minh đường ống.
6. `pro.svelte.ts` + `ProGate.svelte` — hỏi capability từ handshake, không hardcode danh sách trong core.
7. **Quyết định browser mode:** Pro chạy hay không; nếu có thì đường nào; nếu không thì UI phải nói rõ.
8. Ma trận test: (có/không sidecar) × (chạy/chết/lệch version) × (license hợp lệ/hết hạn).
9. CI repo private: build binary Pro theo nền tảng, xuất artifact để core bundle nhận qua `externalBin`. **Không ký rời** — ký + notarize diễn ra ở bước bundle của core.

## Tests / Validation

- Rust unit: protocol serialize + xử lý lệch version.
- Rust integration: spawn sidecar giả, handshake, invoke, kill giữa chừng → core sống.
- **Test lưới an toàn license (quan trọng nhất):** sidecar vắng mặt (bị xoá/AV cách ly) hoặc lệch version → người dùng đã trả tiền **vẫn được nhận diện là Pro** nhờ `cached_state`, chỉ hiện "cần khôi phục thành phần Pro". (Bundle-kèm khiến skew qua auto-update không xảy ra, nhưng sidecar vắng thì vẫn có thể.)
- Thủ công: build core sạch không sidecar → Free đầy đủ, không lỗi trong log.
- Bảo mật: env override không có tác dụng trong bản release build.
- Pháp lý: grep repo MIT xác nhận không có logic Pro, kể cả danh sách capability.

## Success Criteria

- [ ] `externalBin` gói binary Pro vào bundle và **được ký + notarize cùng** app (đã chốt bundle-kèm).
- [ ] `docs/pro-boundary.md` ghi rõ: bundle-kèm, giữ Nguyên tắc 4, đã bác dynamic loading.
- [ ] Core chạy đầy đủ khi không có sidecar.
- [ ] **Version skew không hạ cấp khách trả tiền** — có test chứng minh.
- [ ] Core không chứa registry hay danh sách capability Pro.
- [ ] Env override sidecar chỉ hoạt động ở debug build.
- [ ] Quyết định về browser mode đã được ghi rõ và implement.
- [ ] Overhead khởi động <200ms.
- [ ] Repo private có CI build + ký.

## Tiến độ — 2026-08-11

**Scaffolding core-MIT (không phụ thuộc MoR) đã xong.** Chủ dự án ưu tiên lát cắt
này vì nó không lãng phí dù chọn MoR nào sau này.

Đã làm:

- [x] `src-tauri/src/pro/protocol.rs` — envelope có version + capability + Invoke
      payload JSON đục (hình theo payload compose thật). Logic version-skew tách
      riêng, test đầy đủ. **Không có logic Pro.**
- [x] `src-tauri/src/pro/mod.rs` — enum `ProStatus` (Free / Active / NeedsUpdate /
      LicenseInactive). `NeedsUpdate` tách biệt `Free` — khách trả tiền lệch version
      **không** bị coi là "không phải khách hàng".
- [x] `src-tauri/src/pro/bridge.rs` — định vị socket (env override chỉ debug),
      connect, handshake newline-JSON, kiểm nonce, map ra `ProStatus`. **Vắng
      sidecar → Free im lặng**, có test. Test dùng sidecar giả in-process.
- [x] Tauri command `pro_status` + REST `/api/pro/status` (browser mode).
- [x] Frontend: `pro.svelte.ts` (state capability), `ProGate.svelte` (bọc tính năng,
      offer không chặn), `UpgradeDialog.svelte` + store; mount ở `App.svelte`,
      `loadProStatus()` chạy lúc khởi động. i18n `pro.gate.*` 4 ngôn ngữ.
- [x] 15 test Rust cho pro::*, gồm test chống-strand-khách-trả-tiền (version skew →
      NeedsUpdate chứ không Free).

**Hoãn (phụ thuộc MoR / có sidecar thật):**

- [ ] `cached_state.rs` — cần format license key của MoR (Q#1). Hiện `ProStatus`
      suy ra live từ handshake, chưa có bản ký lưu bền.
- [ ] Repo private `colima-ui-pro` + binary thật + `externalBin` + ký/notarize.
- [ ] `bridge.rs` spawn binary thật + verify chữ ký — hiện chỉ connect tới socket
      có sẵn; đường spawn+verify chờ hạ tầng ký (chưa có trong `tauri.conf.json`).
- [ ] Quyết định browser mode: route đã có, nhưng chưa quyết Pro có chạy ở browser
      mode không (sidecar là tiến trình cục bộ — browser mode có thể không tới được).

### Ghi chú kỹ thuật

- Socket path production hiện là placeholder (`temp_dir()/colima-ui-pro.sock`); chốt
  cùng lúc dựng binary sidecar. Env override `COLIMA_UI_PRO_SOCK` chỉ debug build.
- Nonce hiện từ pid + thời gian nano — đủ chống tiến trình cũ squat socket, không
  phải bí mật mật mã. Đúng mức "defense in depth", không thổi phồng.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| **Version skew biến khách trả tiền thành Free** | **Cao** | `cached_state.rs` làm trước bridge; có test riêng |
| Protocol thiết kế sai, phải sửa sau khi có tính năng | Cao | Thiết kế sau khi biết payload thật của Phase 5; có version từ v1 |
| Env var sidecar → thực thi mã tuỳ ý | Cao | Chỉ debug build; verify chữ ký ở production |
| Ranh giới pháp lý bị chất vấn | TB | Tiến trình riêng + IPC; tài liệu công khai; hỏi luật sư trước launch nếu cần |
| Browser mode không có Pro làm người dùng bối rối | TB | Quyết định rõ ở bước 7 và nói thẳng trong UI |
| Duy trì 2 repo làm chậm phát triển | TB | Protocol tối giản; core không cần biết chi tiết Pro |
