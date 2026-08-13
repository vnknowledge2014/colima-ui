---
phase: 3
title: Tray & Menubar
status: completed
priority: P2
effort: 3-4 ngày
dependencies:
  - 2
---

# Phase 3: Tray & Menubar

## Overview

Tray icon macOS menubar + Linux system tray: xem trạng thái và điều khiển instance không cần mở cửa sổ chính.

`Cargo.toml:9` đã có `tauri = { version = "2", features = ["tray-icon"] }` nhưng `lib.rs` chưa có code tray — không cần thêm dependency.

## Sửa lại từ v1

| v1 giả định | Thực tế | v2 làm |
|---|---|---|
| Icon phản ánh 4 trạng thái: chạy / dừng / **lỗi** / **đang chuyển** | `instance_reader.rs:71-74` xác định trạng thái bằng sự tồn tại của `ha.sock`/`ha.pid` → **chỉ Running hoặc Stopped**. Không có nguồn cho Error/Transitioning | Thêm **lớp pending-state ở phía app** (theo dõi thao tác đang chạy), không giả vờ đọc được từ instance |
| "Cập nhật trong ≤3s" | `poller.rs:39,61` phát event vô điều kiện theo nhịp **5s** | Đổi tiêu chí thành ≤6s, hoặc phát event ngay sau thao tác |
| Chỉ có 1 nguồn phát event | `sse.rs:195-208` là publisher **thứ hai**, độc lập | Tray phải subscribe đúng nguồn; ghi rõ nguồn nào |
| Menu event id parse rồi chạy lệnh | `validation.rs` chưa có validator tên profile | Bắt buộc dùng validator từ plan hotfix bảo mật |

## Requirements

**Functional**
- Icon tray phản ánh trạng thái tổng: Running / Stopped / **Pending** (có thao tác đang chạy).
- Menu: danh sách instance (start/stop), số container đang chạy, mở app, mở Terminal, Settings, Quit.
- **Menu item đang có thao tác chạy dở phải bị disable** — chống double-invoke khi menu stale.
- Click icon → toggle cửa sổ chính.

**Non-functional**
- Chỉ rebuild menu khi snapshot thực sự đổi (diff), tránh nhấp nháy.
- Icon template mode trên macOS.
- Không tạo poller mới — tái dùng nguồn event có sẵn.
- Linux không hỗ trợ tray → log warning, **không crash**.

## Architecture

```
src-tauri/src/tray.rs (MỚI)
  ├─ init_tray(app)
  ├─ rebuild_menu(app, snapshot)      — diff trước khi rebuild
  ├─ update_icon(app, aggregate)      — Running | Stopped | Pending
  ├─ handle_menu_event(app, id)       — parse id → VALIDATE profile → chạy
  └─ PendingOps: HashSet<profile>     — đánh dấu thao tác đang chạy, disable item

Nguồn dữ liệu: poller.rs (instances, nhịp 5s) + docker_state.rs (containers)
CẢNH BÁO: sse.rs:195-208 là publisher độc lập — không subscribe nhầm nguồn
```

Trạng thái tổng: `Pending` (có op đang chạy) > `Running` (≥1 instance running) > `Stopped`.

## Related Code Files

- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs` — `pub mod tray;` + gọi trong `setup()`
- Modify: `src-tauri/src/poller.rs` — phát event ngay sau thao tác, không chỉ theo nhịp
- Modify: `src-tauri/src/validation.rs` — dùng `is_valid_profile_name` (từ plan hotfix)
- Create: `src-tauri/icons/tray/*.png` — 3 trạng thái, template mode
- Modify: `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json` nếu Tauri yêu cầu quyền
- Modify: `src/lib/settingsStore.svelte.ts`, `src/pages/Settings.svelte` — bật/tắt tray

**Lưu ý về settings:** `settingsStore.svelte.ts` là map chuỗi phẳng bất đồng bộ và nuốt lỗi ghi bằng `console.error`. Tray cần đọc setting từ **phía Rust lúc `setup()`**, trước khi frontend sẵn sàng — cần đường đọc riêng, không đi qua store frontend.

## Implementation Steps

1. Thiết kế 3 icon trạng thái, xuất template PNG (macOS: ảnh đen + alpha, `is_template(true)`).
2. Xác định đường đọc setting "hiện tray" từ Rust lúc `setup()` (đọc thẳng file settings, không qua frontend).
3. `init_tray` với menu tĩnh (mở app / quit) trước, xác nhận đường ống chạy.
4. `rebuild_menu` sinh mục instance động; id mã hoá `instance:start:<name>`.
5. **`handle_menu_event` validate tên profile qua `is_valid_profile_name` trước khi làm bất cứ gì** — menu id là input không tin cậy.
6. `PendingOps`: đánh dấu khi bắt đầu thao tác, xoá khi xong/lỗi; item đang pending bị disable.
7. Subscribe đúng nguồn event; diff snapshot trước khi rebuild.
8. Phát event ngay sau thao tác thay vì chờ nhịp 5s.
9. `update_icon` theo trạng thái tổng.
10. Test trên Linux (GNOME + KDE) — init lỗi thì log warning, app vẫn chạy.

## Tests / Validation

- Rust unit: parse menu event id, bao gồm tên profile độc (`-rf`, `../`, null byte) → bị từ chối.
- Rust unit: trạng thái tổng từ danh sách instance + PendingOps.
- Rust unit: diff snapshot — không rebuild khi dữ liệu không đổi.
- Thủ công: bấm start 2 lần liên tiếp trên menu stale → chỉ 1 lệnh chạy.
- Thủ công macOS: menubar light/dark; start/stop từ tray; số container cập nhật.
- Thủ công Linux: tray hiện hoặc degrade an toàn.
- So sánh CPU nền trước/sau.

## Sai lệch so với plan

| Plan nói | Thực tế làm | Lý do |
|---|---|---|
| Tạo `src-tauri/icons/tray/*.png` | **Sinh icon lúc chạy** thành RGBA trong `status_icon()` | Template macOS là mặt nạ đơn sắc — icon phân biệt bằng **hình** (đĩa đặc / vòng rỗng / chấm nhỏ), không phải màu. Sinh bằng code thì không phải quản lý asset nhị phân và có test khẳng định 3 trạng thái khác nhau |
| Start từ tray | Thêm `start_instance_cli(profile)` **không truyền cờ tài nguyên** | Tray không có UI chọn CPU/RAM. `colima start --profile X` dùng lại config đã lưu; truyền giá trị mặc định sẽ **âm thầm resize VM của người dùng** |
| Đọc setting lúc `setup()` | Dựng tray trước, rút lại nếu setting = `false` | Setting nằm trong SQLite, chỉ đọc được bất đồng bộ. Cách này tránh chặn khởi động và không phải chờ frontend |

## Success Criteria

- [x] Tray dựng trên macOS với icon template (`icon_as_template(true)`), phân biệt bằng hình nên đúng ở cả light/dark.
- [x] Start/stop từ tray hoạt động; `LAST_SNAPSHOT` bị xoá sau thao tác nên rebuild ngay, không chờ nhịp 5s.
- [x] Item đang pending bị disable; `PENDING.insert` trả false thì bỏ qua — chặn double-invoke ở cả hai lớp.
- [x] Tên profile từ menu id qua `is_valid_profile_name` (có test với `-rf`, `../`, null byte, khoảng trắng).
- [x] Click icon toggle cửa sổ chính.
- [x] Tray init thất bại chỉ log warning, `setup()` vẫn trả `Ok`.
- [x] Setting `colimaui_show_tray` đọc từ Rust, không phụ thuộc frontend; có toggle ở `TraySettings.svelte`.
- [x] Subscribe `poller.rs` (đã ghi comment cảnh báo `sse.rs` là publisher khác).
- [x] 7 test Rust mới (45 tổng).

## Chưa kiểm chứng được ở đây

Hành vi tray thật (hiện icon, click, menu) **không test được bằng `cargo test`** — cần chạy bản bundle. Đã kiểm chứng: biên dịch, logic parse id, trạng thái tổng, diff snapshot, khác biệt icon. Cần bạn thử tay: menubar light/dark, start/stop, và Linux (GNOME/KDE) xem degrade có an toàn không.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Menu stale → double-invoke thao tác phá hoại | PendingOps + disable item; đây là lý do phase này dài hơn v1 |
| Tên profile từ menu id chạy như cờ CLI | Validator bắt buộc (plan hotfix bảo mật chặn phase này) |
| Subscribe nhầm publisher (`sse.rs` vs `poller.rs`) | Ghi rõ nguồn trong code comment; test rằng tray cập nhật ở cả 2 mode |
| Tray API khác nhau macOS/Linux | Bọc trong `platform.rs`, degrade an toàn |
| Đọc settings lúc `setup()` phức tạp hơn dự kiến | Nếu bí thì mặc định bật tray, cho tắt sau khi frontend load |
