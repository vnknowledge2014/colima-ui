# Phase 3: Security UI + image catalog ký số

**Tier:** Free · **Chờ:** phase 2 · **Trạng thái: một nửa xong 2026-08-12**

Phase này tách được làm hai, và **chỉ một nửa bị chặn**:

| Phần | Cần khoá ký? | Trạng thái |
|---|---|---|
| Security UI + catalog nhúng | Không | ✅ **Xong 2026-08-12** |
| Kênh cập nhật có chữ ký (ed25519, verify-trước-parse) | **Có** | ⛔ Chờ Unresolved #1 |

Làm nửa đầu trước vì nó là toàn bộ giá trị người dùng thấy được, và vì bản nhúng
chính là **đường dự phòng** mà nửa sau cần đến dù sao đi nữa. Xem "Kết quả".

## Overview

Đưa mọi thứ phase 1–2 tạo ra lên màn hình, và trả lời câu người dùng thực sự
hỏi sau khi thấy điểm thấp: **"vậy tôi nên dùng image nào?"**

Câu trả lời đó đến từ **artifact tĩnh có ký số**, không phải API server —
quyết định #1 của chủ dự án.

## Requirements

**Functional**
- Trang Security: danh sách image + điểm, sắp xếp theo điểm.
- Chi tiết một image: 4 thành phần điểm, danh sách finding, danh sách rule fail, SBOM.
- Mỗi rule fail hiển thị `remediation` tự viết + link bài Knowledge Bank.
- Gợi ý image thay thế lấy từ catalog tĩnh (`python:3.12` → `python:3.12-slim` → distroless).
- Cập nhật catalog + rule pack: người dùng **bấm mới chạy**, không tự động.

**Non-functional**
- Chữ ký ed25519 xác minh **trước khi** parse nội dung tải về. Sai chữ ký → vứt, giữ bản cũ, báo người dùng.
- Không có mạng / chưa cập nhật bao giờ → dùng bản nhúng trong binary. Toàn bộ trang vẫn chạy.
- Không gửi danh sách image của người dùng đi đâu. Catalog là **tải xuống**, không phải truy vấn.

## Architecture

```
Kênh artifact tĩnh (CDN / GitHub Releases — KHÔNG phải server có state)
  ├─ rules-v<N>.json        + rules-v<N>.json.sig
  └─ catalog-v<N>.json      + catalog-v<N>.json.sig
        { "python:3.12": { alternatives: [...], notes, base_family } }

commands/security_catalog.rs
  ├─ PUBLIC_KEY: [u8; 32] = include_bytes!(...)   ← nhúng lúc build
  ├─ verify_and_load(bytes, sig) -> Result<Catalog>
  │     XÁC MINH TRƯỚC, PARSE SAU. Không bao giờ ngược lại.
  ├─ CATALOG_EMBEDDED = include_str!("../resources/catalog/v1.json")
  └─ lưu bản đã tải ở app data; hỏng/sai chữ ký → rơi về bản nhúng

src/pages/Security.svelte              (NEW — thêm route + Sidebar)
  ├─ ImageScoreList.svelte
  ├─ ScoreBreakdownCard.svelte    ← 4 thanh + 3 dòng ScoreInputs
  ├─ FindingsTable.svelte         ← lọc theo severity, có fixed_version
  ├─ RuleChecklist.svelte         ← pass/fail + remediation + link KB
  └─ AlternativesPanel.svelte     ← gợi ý từ catalog
```

**Vì sao xác minh trước khi parse:** parser JSON là bề mặt tấn công. Nếu parse
trước rồi mới kiểm chữ ký, một file độc đã đi qua parser rồi. Thứ tự này không
phải chi tiết, nó là toàn bộ giá trị của việc ký.

**Vì sao catalog là tải-xuống chứ không phải truy-vấn:** truy vấn "image X điểm
bao nhiêu" gửi danh sách image của người dùng lên server ta — đó là dữ liệu
nhạy cảm của họ, và là chi phí traffic tỉ lệ thuận với usage. Tải nguyên
catalog (vài trăm KB) giải quyết cả hai.

## Related Code Files

- Create: `src-tauri/src/commands/security_catalog.rs`, `src-tauri/resources/catalog/v1.json`
- Create: `src/pages/Security.svelte`, `src/components/security/{ImageScoreList,ScoreBreakdownCard,FindingsTable,RuleChecklist,AlternativesPanel}.svelte`
- Modify: `src/App.svelte` (route), `src/components/Sidebar.svelte` (mục menu)
- Modify: `src-tauri/src/routes/security.rs` — `GET /api/security/catalog`, `POST /api/security/catalog/update`
- Modify: `src/locales/{en,vi,ja,zh}.json`
- Modify: `src-tauri/Cargo.toml` — thêm crate ed25519 (**kiểm tra Cargo.toml trước**, có thể đã có qua dependency khác)
- Modify: `src-tauri/src/commands/kb_articles.rs::seed` — bài KB cho từng rule id

## Implementation Steps

1. Chốt crate chữ ký. Kiểm `Cargo.lock` xem `ed25519-dalek` đã có sẵn qua cây phụ thuộc chưa trước khi thêm mới.
2. `verify_and_load` + test: chữ ký sai / file cắt cụt / khoá khác → đều từ chối, không panic.
3. Catalog v1 nhúng: 30–50 image phổ biến. Viết tay, không sinh tự động.
4. Route catalog + luồng cập nhật thủ công.
5. `Security.svelte` + route + Sidebar + i18n 4 ngôn ngữ.
6. `ScoreBreakdownCard` — 4 thanh, và ba dòng `ScoreInputs` **hiển thị thường trực**, không trong tooltip.
7. `RuleChecklist` — mỗi rule fail link tới bài KB qua `kb_get_article`.
8. Bài KB cho các rule đã ship ở phase 2, 4 ngôn ngữ, seed qua `kb_articles.rs::seed`.
9. Trạng thái rỗng: chưa cài scanner → hướng dẫn cài, lấy từ `install_hint` của phase 1.

## Tests

- Chữ ký hỏng → giữ catalog cũ, hiện cảnh báo, không crash.
- Chưa từng cập nhật + không mạng → trang render đầy đủ bằng bản nhúng.
- Điểm hiển thị luôn kèm pack version + ngày DB (test component).
- Mọi chuỗi UI có ở cả 4 locale (repo đã có 4 file locale — thiếu là regression).
- Không có image nào trên máy → trạng thái rỗng có nghĩa, không phải bảng trống.

## Risks

| Rủi ro | Xử lý |
|---|---|
| Khoá riêng để ký chưa có quy trình | **Unresolved #1 trong plan.md** — chặn phase này, phải chốt trước bước 1 |
| Catalog gợi ý image đã lỗi thời | Catalog ghi `updated_at`; UI hiện tuổi catalog. Không hứa cái ta không cập nhật nổi |
| Danh sách gợi ý thành lời khuyên sai cho use case cụ thể | Ngôn ngữ là "phổ biến hơn / nhỏ hơn / non-root sẵn", không phải "bạn nên dùng" |
| Trang Security thành nơi đổ mọi thứ | Chỉ 5 component ở trên. Lịch sử điểm là phase 5, không nhét vào đây |

## Success Criteria

- [x] Người dùng scan xong thấy điểm, hiểu vì sao thấp, và thấy 1–3 phương án thay thế.
- [x] Rút mạng → trang vẫn dùng được đầy đủ (catalog + rule pack nhúng trong
      binary; chưa có cache nào để xoá vì chưa có kênh tải).
- [ ] Sửa 1 byte trong file catalog đã ký → app từ chối và giữ bản cũ.
      **Chưa làm** — thuộc nửa bị chặn.

## Kết quả — 2026-08-12 (nửa không cần khoá ký)

`cargo test` 350/350, frontend 285/285 (10 test component mới), clippy/typecheck/
lint/check/build đều sạch.

**Tạo:** `resources/catalog/v1.json` (26 base image, viết tay),
`commands/security_catalog.rs`, `pages/Security.svelte`,
`components/security/{ScoreBreakdownCard,FindingsTable,RuleChecklist,AlternativesPanel}.svelte`,
bài KB `image-hardening` ×4 ngôn ngữ.
**Sửa:** `routes/security.rs` (+`GET /api/security/alternatives`), `App.svelte`
(route), `Sidebar.svelte`, `lib/api/security.ts`, 4 file locale, `kb_articles.rs`,
`docs/{api,frontend,telemetry}.md`.

### Ba chỗ lệch so với đặc tả, và lý do

**Không dựng kênh SSE thứ hai cho tiến trình scan.** Đặc tả muốn hiện riêng giai
đoạn "đang tải DB" và "đang quét". Backend **có** phát sự kiện
`security-scan-progress`, nhưng frontend chưa có bus SSE dùng chung —
`transferEvents.ts` tự mở `EventSource` riêng, và nhân bản ~80 dòng đó cho một
nhãn trạng thái là cái giá sai. Thay bằng một câu nói thẳng: *"lần quét đầu sau
một thời gian sẽ tải cơ sở dữ liệu ~1.2 GB trước khi bắt đầu."* Cùng thông tin,
không thêm đường ống.

**Một bài KB, không phải một bài cho mỗi rule.** 12 rule × 4 ngôn ngữ = 48 file
lặp lại đúng những gì `rationale`/`remediation` trong rule pack đã nói, và sẽ
lệch nhau ngay lần sửa rule đầu tiên. `image-hardening` giải thích cả nhóm và
được link từ checklist.

**26 mục catalog, không phải 30–50.** Viết tay, và chỉ viết những image tôi mô tả
được trung thực việc "đổi sang cái này thì đổi cái gì". Thêm mục là một dòng JSON.

### Lỗi nghiêm trọng nhất review tìm ra: lời hứa riêng tư chưa được code cưỡng chế

Trang viết "không có gì về image được gửi đi đâu". Nhưng `trivy image` mặc định
có `--image-src docker,containerd,podman,**remote**`: nếu tra daemon local thất
bại vì bất kỳ lý do gì, Trivy **im lặng rơi xuống registry công cộng và gửi tên
image đi**. Đúng loại lỗi đã loại Grype ở bakeoff phase 1 — chỉ khác là Trivy né
được nhờ cấu hình mặc định, không phải nhờ thiết kế.

Đã ghim `--image-src docker,containerd,podman` ở cả scan lẫn xuất SBOM. Lời hứa
giờ nằm trong argv chứ không nằm trong câu chữ UI.

### Ba lỗi tương tranh ở trang, cùng một gốc

`scanning` và `scanError` từng là **một** giá trị cho cả trang. Quét image A,
chọn image B, quét tiếp: cái nào xong trước cũng xoá cờ của cả hai — A mất nút
Cancel trong khi tiến trình vẫn chạy, lỗi của A hiện dưới B, và `cancel()` tính
lại `scanId` lúc bấm nên huỷ nhầm image. Giờ cả hai keyed theo image ref, và
`cancel(imageRef)` nhận đúng ref mà nút được vẽ ra cho.

`changeLevel` cũng xoá sạch `audits` **trước** vòng lặp, nên một lần re-audit
hỏng là mất vĩnh viễn kết quả đó.

### Ngôn ngữ của catalog là điều đáng giữ

Mỗi gợi ý nói **đổi thì khác gì**, không nói *nên* dùng gì. Alpine có hợp hay
không phụ thuộc vào việc phụ thuộc native có build được với musl — catalog không
biết điều đó, và giả vờ biết thì lời khuyên còn tệ hơn không có. Có test chặn
chuỗi "you should" lọt vào file.
