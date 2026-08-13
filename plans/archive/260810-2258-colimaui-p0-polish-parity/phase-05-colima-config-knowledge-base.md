---
phase: 5
title: "Colima Config & Knowledge Base"
status: completed
priority: P2
effort: "4-5 ngày"
dependencies: [2]
---

# Phase 5: Colima Config & Knowledge Base

## Overview

Sửa config Colima trong UI, và bộ Knowledge Base offline làm đích đến cho `hint`/`doc_id` từ Phase 1 và Phase 2.

## Sửa lại từ v1 — cảnh báo mất dữ liệu

**v1 nói:** "tái dùng `instance_reader.rs` để đọc/ghi config (DRY)".

**Thực tế** (`src-tauri/src/instance_reader.rs:24-48`): struct đó chỉ **Deserialize**, parse **7 field**, **không có `#[serde(flatten)]`**, và tại `:83` dùng `unwrap_or_default()` — parse lỗi thì trả về cpu/mem/disk = 0 thay vì báo lỗi.

Dùng struct này để ghi sẽ **xoá sạch** `mounts`, `provision`, `env`, và mọi field người dùng tự thêm. Ghi atomic chỉ đảm bảo việc phá hoại diễn ra nguyên tử.

**v2 làm:** đọc/sửa/ghi trên `serde_yml::Value` — mutate đúng các key cần đổi, giữ nguyên phần còn lại. **Hard-fail khi parse lỗi**, không bao giờ `unwrap_or_default`.

## Sửa lại từ v1 — Knowledge Base

**v1 nói:** "tái dùng `knowledge_bank.rs` (883 dòng, 22+ solution, FTS)".

**Thực tế:** không có bảng article; FTS5 chỉ phủ `agent_memory`, **không phủ** `solutions`; 23 pattern có sẵn là lỗi khởi động VM, gần như không giao với nội dung hướng dẫn cài đặt.

**v2 làm:** coi đây là **migration schema** — bảng `articles` mới + index FTS mới + khoá dedupe để seed idempotent + version nội dung. Không phải "cắm vào là chạy".

## Sửa lại từ v1 — hai nơi cùng ghi config

`src-tauri/src/commands/colima.rs:87-130` — `start_instance` **đã** set cpu/memory/disk/runtime/vm-type/mount-type/dns qua cờ CLI. Nếu Phase này ghi thẳng YAML thì có 2 nguồn ghi cạnh tranh.

**v2 làm:** chốt một đường duy nhất trước khi code. Ưu tiên: UI sửa YAML là nguồn sự thật, `start_instance` **đọc** từ đó thay vì tự truyền cờ.

### Quyết định đã chốt (2026-08-11)

**YAML là nguồn sự thật.** `start_instance` chỉ truyền bộ cờ đầy đủ khi profile **chưa có** `colima.yaml` — tức lần tạo đầu từ SetupWizard, thời điểm duy nhất chưa có file để đọc. Profile đã tồn tại thì không truyền cờ tài nguyên nào; colima tự đọc yaml mà UI sở hữu. Giống hệt `start_instance_saved` đang làm.

Hệ quả: sửa CPU/RAM trong UI rồi restart sẽ có hiệu lực, thay vì bị cờ CLI của lần start sau ghi đè ngược.

**Knowledge Base: đủ 4 locale** (en/vi/ja/zh). Bảng `articles` có cột `locale`, khoá dedupe là `(slug, locale)`; 5 bài × 4 locale = 20 bản ghi seed.

## Requirements

**Functional**
- Sửa: CPU, memory, disk, runtime, VM type, mount type, DNS, network, Kubernetes on/off.
- Preset Nhẹ / Cân bằng / Hiệu năng cao, tính từ tài nguyên máy thật.
- Diff trước khi áp dụng; chỉ rõ field nào cần restart.
- Backup `.bak` có timestamp trước mỗi lần ghi; ghi atomic (file tạm + rename).
- KB: trang Help offline, tìm kiếm được, ≥5 bài, liên kết từ `doc_id` của lỗi.

**Non-functional**
- **Không bao giờ** ghi khi parse thất bại hoặc validate thất bại.
- Giữ nguyên mọi field không được UI quản lý.
- Nội dung KB đóng gói trong `src-tauri/resources/`, không cần mạng.

**Bỏ khỏi phạm vi:** editor YAML thô (cắt theo khuyến nghị red-team — rủi ro cao, giá trị thấp ở giai đoạn này).

## Architecture

```
CONFIG
src-tauri/src/commands/colima_config.rs (MỚI)
  ├─ read_raw(profile)  → serde_yml::Value      (hard-fail nếu parse lỗi)
  ├─ validate(value)    → Vec<ValidationIssue>
  ├─ apply_changes(value, changes) → Value      (mutate key, giữ phần còn lại)
  ├─ diff(old, new)     → Vec<FieldChange { field, from, to, requires_restart }>
  └─ write_atomic(profile, value)               (backup .bak → tmp → fsync → rename)
      ⚠ profile phải qua is_valid_profile_name + assert_path_within (plan hotfix)

KNOWLEDGE BASE — migration schema
src-tauri/src/commands/knowledge_bank.rs
  ├─ bảng articles MỚI (id, slug, title, body, platform, version, updated_at)
  ├─ index FTS5 MỚI cho articles (hiện FTS chỉ phủ agent_memory)
  └─ seed idempotent theo slug + version
src-tauri/resources/kb/*.md
src/pages/Help.svelte  (render bằng src/lib/markdown.ts sẵn có)
```

## Related Code Files

- Create: `src-tauri/src/commands/colima_config.rs`
- Modify: `src-tauri/src/commands/colima.rs` — `start_instance` không tự truyền cờ nữa (xem quyết định ở trên)
- Modify: `src-tauri/src/commands/knowledge_bank.rs` — migration bảng articles + FTS
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/routes/`
- **Không** dùng: `instance_reader::ColimaConfig` cho việc ghi
- Create: `src-tauri/resources/kb/{install-colima,install-docker-cli,install-kubectl,common-errors,performance-tuning}.md`
- Create: `src/components/DiffView.svelte` — dùng chung; plan thương mại Phase 5 sẽ dùng lại
- Create: `src/pages/settings/ColimaConfig.svelte`, `src/pages/Help.svelte`
- Modify: `src/pages/Settings.svelte`, `src/components/Sidebar.svelte`, `src/lib/errorReporter.ts` (nối `doc_id` → bài KB), `src/locales/*`

## Implementation Steps

1. **Chốt nguồn ghi duy nhất** giữa `start_instance` (cờ CLI) và UI (YAML). Không code trước khi chốt.
2. `read_raw` trả `serde_yml::Value`, hard-fail khi parse lỗi (kèm số dòng nếu lấy được).
3. `apply_changes` mutate đúng key trên `Value`; test round-trip khẳng định `mounts`/`provision`/`env` còn nguyên.
4. `validate` — cảnh báo CPU/RAM vượt máy thật, disk nhỏ hơn dung lượng đang dùng, enum sai.
5. `write_atomic` — profile qua validator; backup `.bak` timestamp; tmp + fsync + rename.
6. `diff` + đánh dấu `requires_restart`.
7. Migration KB: bảng articles + FTS5 + seed idempotent theo slug+version.
8. Viết 5 bài KB theo cấu trúc "Triệu chứng → Nguyên nhân → Lệnh khắc phục".
9. `DiffView.svelte` + `ColimaConfig.svelte` + `Help.svelte`; nối `doc_id` từ Phase 1.
10. i18n 4 locale.

## Tests / Validation

- **Rust unit quan trọng nhất:** đọc file colima.yaml có `mounts`/`provision`/`env`/field lạ → đổi 1 field → ghi → khẳng định **mọi field khác còn nguyên vẹn từng ký tự có ý nghĩa**.
- Rust unit: parse lỗi → trả Err, **không** ghi gì.
- Rust unit: `validate` bắt đúng case vượt tài nguyên.
- Rust unit: `diff` đánh dấu `requires_restart` đúng.
- Rust unit: seed KB idempotent — chạy 3 lần không nhân bản.
- Thủ công: sửa CPU/RAM → apply → restart → `colima status` phản ánh đúng.
- Thủ công: config hỏng có chủ ý → bị chặn, file gốc nguyên vẹn, có `.bak`.
- Thủ công: ngắt mạng → Help vẫn đầy đủ.

## Success Criteria

- [x] Ghi config **không mất** field ngoài phạm vi UI quản lý — `round_trip_preserves_unmanaged_fields` + `nested_edits_preserve_siblings` khẳng định `mounts`/`provision`/`env`/`customFutureKey` và các key anh em trong `network`/`kubernetes` còn nguyên.
- [x] Parse lỗi → hard-fail, không ghi, không `unwrap_or_default` — `malformed_yaml_is_an_error_and_nothing_is_written`, `non_mapping_yaml_is_rejected`.
- [x] Chỉ có **một** nguồn ghi config — `start_instance` chỉ truyền cờ khi chưa có `colima.yaml`.
- [x] Backup `.bak` + ghi atomic; profile qua validator — `write_atomic_leaves_a_backup_of_the_previous_content`, `traversal_profile_names_are_rejected`.
- [x] Diff hiện trước khi áp dụng, chỉ rõ field cần restart — `DiffView.svelte` + `diff_lists_only_changed_managed_fields_and_marks_restart`.
- [x] KB: bảng articles + FTS mới, seed idempotent, ≥5 bài, offline — 6 slug × 4 locale = 24 bài, `include_str!` nên không cần mạng lẫn resource path.
- [x] ≥5 loại lỗi phổ biến ở Phase 1 mở được bài KB tương ứng — `every_referenced_doc_id_has_an_article` khoá 5 slug mà `error.rs` và `system_capabilities.rs` phát ra.

## Kết quả thực hiện (2026-08-11)

### Ngoài phạm vi kế hoạch ban đầu

- **Thêm bài `start-colima`** (thành 6 slug, không phải 5). `error.rs:213` phát `doc_id = "start-colima"` cho `NotRunning` — lỗi phổ biến nhất — nhưng slug đó không có trong danh sách 5 bài của kế hoạch. Thiếu nó thì tiêu chí "≥5 loại lỗi mở được bài tương ứng" không đứng vững.
- **Sửa `system_capabilities.rs:120`**: `doc_id("kubectl")` trả `"kubectl"` trong khi kế hoạch đặt tên bài là `install-kubectl` → link chết. Đã đổi thành `install-kubectl`.
- **`ValidationIssue` mang thêm `params`**. i18n dịch theo `code`, nhưng thông báo hữu ích nằm ở các con số ("32 CPU trong khi máy có 8"). Không có params thì bản dịch buộc phải bỏ số. Test `issues_carrying_numbers_also_carry_them_as_params` khoá hợp đồng này.
- **KB đủ 4 locale** thay vì 1 (quyết định của người dùng). Bảng `articles` có cột `locale`, khoá dedupe `(slug, locale)`, fallback về `en` khi thiếu bản dịch.

### Khác với thiết kế trong kế hoạch

- **Nội dung KB `include_str!` vào binary** thay vì đọc `src-tauri/resources/` lúc chạy. File markdown vẫn nằm đúng chỗ kế hoạch nêu, nhưng browser mode không có `AppHandle` để resolve resource path — biên dịch vào binary biến "chạy offline" thành thuộc tính của binary chứ không phải của bản cài.
- **Tách `kb_articles.rs` khỏi `knowledge_bank.rs`**. File đó đã 929 dòng và `articles` là mối quan tâm khác `solutions` (bài đọc cho người dùng vs cặp pattern→remedy cho AI agent).
- **`config_path` không gọi `assert_path_within`**. Hàm đó canonicalize thư mục cha nên thất bại với profile chưa tồn tại, khiến người gõ nhầm tên profile nhận lỗi "đường dẫn không hợp lệ". Chặn traversal đã do `ensure_valid_profile` đảm nhiệm; kiểm tra canonicalize chuyển vào `existing_config_path` và `write_atomic`, nơi đường dẫn thật sự tồn tại.
- **Thêm kiểm tra mtime** (không có trong kế hoạch, có trong Risk Assessment). `colima start` ghi lại `colima.yaml`, nên form mở lâu rồi bấm Apply sẽ lặng lẽ ghi đè thứ colima vừa ghi. `expected_mtime` chặn việc đó.

### Kiểm chứng

- `cargo test --lib`: **72 passed, 0 failed** (21 test mới).
- `npx vitest run --environment node`: **112 passed**.
- `pnpm run build`: xanh.
- `pnpm run check`: 152 lỗi / 39 file — **toàn bộ có từ trước**; không file nào của phase này xuất hiện trong danh sách.

### Chưa làm / đã biết

- Kiểm chứng thủ công (sửa CPU → restart → `colima status`; config hỏng có chủ ý; ngắt mạng) **chưa chạy** — cần máy có Colima.
- `pnpm test` trong Acceptance Criteria của `plan.md` **không tồn tại** như một script trong `package.json`. Đã chạy `npx vitest` thay thế. Ngoài ra vitest với `environment: jsdom` mặc định sập vì undici 8 không tương thích Node 20.19 (`webidl.util.markAsUncloneable is not a function`) — lỗi môi trường có từ trước, không liên quan phase này.
- `src/hooks/useHotkeys.ts` (finding #9, code React chết) vẫn còn và vẫn sinh lỗi typecheck. Không thuộc phạm vi phase 5.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| **Ghi config xoá mất cấu hình người dùng** — v1 sẽ gây ra chính xác điều này | **Cao** | Mutate `serde_yml::Value`, không dùng struct typed; test round-trip là điều kiện merge |
| Hai nguồn ghi cạnh tranh (`start_instance` vs UI) | Cao | Chốt ở bước 1 trước khi code |
| Colima ghi đè file trong lúc app đang sửa | TB | Kiểm tra mtime trước khi ghi; nếu đổi thì báo và yêu cầu đọc lại |
| Migration KB phức tạp hơn "tái dùng" như v1 tưởng | TB | Đã tính vào ước lượng; migration có version để nâng cấp nội dung sau |
| Schema colima.yaml đổi theo version | TB | Làm việc trên `Value`, không gắn cứng schema → tự nhiên chịu được thay đổi |
