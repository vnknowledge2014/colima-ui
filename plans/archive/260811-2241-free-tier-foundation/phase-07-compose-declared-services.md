---
phase: 7
title: "Service compose chưa tạo + cạnh depends_on"
status: done
priority: P3
dependencies: [4, 5, 6]
effort: ""
---

# Phase 7: Service compose chưa tạo + cạnh `depends_on`

Trả nợ kỹ thuật do phase 5 và 6 để lại.

## Vấn đề

`docker ps` chỉ biết container **đã được tạo**. Một project compose đang down, hoặc up dở, có service mà engine chưa từng nghe tới. Đó đúng là những service người dùng đi tìm khi có thứ không lên được — và đồ thị lại đang hiện project rỗng, hoặc không hiện gì cả.

## Kiến trúc

Module riêng `src-tauri/src/commands/compose_services.rs`: nó biết YAML và filesystem, `topology.rs` biết dựng đồ thị. Không bên nào cần ôm vấn đề của bên kia.

```
docker compose ls  →  config_files (đường dẫn)
   → compose_services::declared_services()  → Vec<ComposeService>
   → topology::get_topology()
        ├─ service đã có container  → dùng chính node container đó
        └─ chưa có                  → node kind="service", status="notCreated"
        └─ depends_on               → cạnh giữa hai node, bất kể loại nào
```

**Điểm mấu chốt**: giải nghĩa service → node **trước** khi dựng cạnh. Nhờ vậy `depends_on` trỏ tới container hay tới node ma đều được, không cần biết là loại nào.

## Chính sách bảo mật / an toàn

| Quyết định | Lý do |
|---|---|
| Đường dẫn lấy từ `docker compose ls`, không nhận từ UI | Đây là thứ daemon của chính người dùng ghi lại khi tạo project — cùng mức tin cậy với tên container, image, mount mà app đã tin sẵn |
| Chặn file > 2 MB | Compose file là cấu hình, không phải dữ liệu. File to hơn thế không phải compose file, và đọc nó tốn hơn đồ thị nó sinh ra |
| **Không** đi theo `extends` / `include` | Đó là giải đường dẫn tuỳ ý *từ bên trong file*, một câu hỏi tin cậy khác hẳn với việc đọc thứ daemon đã trỏ tới |
| Đọc best-effort, lỗi thành warning | Một file hỏng chỉ mất service của file đó, không bao giờ mất cả đồ thị |
| Chỉ trích tên service / image / depends_on | Không hiển thị nội dung file, nên không có đường rò nội dung ra UI |

## Quyết định thiết kế

- **`notCreated` là trạng thái riêng, không gộp vào `stopped`.** Container stopped có tồn tại và `start` được; service notCreated chỉ tồn tại trong file, phải `compose up`. Gộp hai cái = chỉ người dùng sang lệnh sai.
- **Service đã chạy không sinh node ma.** Khớp qua nhãn `com.docker.compose.service` + `com.docker.compose.project`. Không có bước này thì mỗi service đang chạy bị vẽ hai lần.
- **`depends_on` đọc cả hai dạng cú pháp** (list và map có `condition`). Chỉ đỡ một dạng là âm thầm mất toàn bộ dependency ở các project dùng dạng kia.
- **Dependency trỏ tới service không tồn tại thì bỏ qua.** Đó là lỗi chính tả trong compose file của người dùng; vẽ cạnh tới node không có sẽ biến nó thành lỗi render.
- **Project node được tạo bổ sung từ `docker compose ls`.** Project down hoàn toàn không có container nào để sinh ra node project của nó.

## Hình ảnh

| | Thể hiện |
|---|---|
| Chip service chưa tạo | viền nét đứt, nền rỗng, chữ mờ |
| Dấu trạng thái | vòng tròn **nét đứt** — khác vòng rỗng (stopped) kể cả ở thang xám |
| Cạnh `dependsOn` | tím, nét đứt 7-3 — màu riêng vì nó nghĩa là "trước/sau", không phải "gắn vào" |
| Rail | thêm chip lọc "Not created" + dấu chú giải nét đứt |

## Files

| File | Việc |
|---|---|
| `src-tauri/src/commands/compose_services.rs` | **mới** — parse compose YAML, 5 test |
| `src-tauri/src/commands/mod.rs` | + `pub mod compose_services` |
| `src-tauri/src/commands/topology.rs` | khớp service↔container qua nhãn; node `service`; cạnh `dependsOn`; project bổ sung; + 1 test |
| `src/lib/api/topology.ts` | + kind `service`, status `notCreated`, edge `dependsOn` |
| `src/components/topology/GraphCanvas.svelte` | chip nét đứt, dấu nét đứt, style cạnh `dependsOn` |
| `src/components/topology/NodeDetail.svelte` | nhãn loại/trạng thái; "Open in Compose" cho service dùng **tên project**, không phải tên service |
| `src/pages/Topology.svelte` | chip lọc + dấu chú giải mới |
| `src/locales/{en,vi,ja,zh}.json` | 3 khoá × 4 ngôn ngữ |

## Kiểm chứng (2026-08-12)

- `cargo test --lib` **285/285 pass**, `cargo clippy --lib --all-targets` 0 cảnh báo ở file đụng tới.
- `pnpm vitest run` **151/151 pass** — thêm test khẳng định service chưa tạo vẽ nét đứt, **không** mang dấu stopped, và cạnh `dependsOn` được vẽ.
- `pnpm build` OK. `svelte-check` 143 lỗi/36 file — y hệt baseline, 0 lỗi mới.

## Ghi chú

Một lần chạy `cargo test --lib` giữa chừng báo fail ở `commands::diagnostics::tests::sections_are_never_constructed_without_redacting` — tên test đó **không tồn tại** trong `diagnostics.rs`, và file đó đang untracked (`??`), tức đang được sửa song song ngoài phase này. Chạy lại: 15/15 test diagnostics pass, toàn bộ 285/285 pass. Không liên quan tới phase này.

## Chưa làm

- `extends` / `include` trong compose file (quyết định có chủ đích, xem bảng chính sách).
- Merge compose theo đúng ngữ nghĩa của Docker: hiện chỉ union theo tên service, file sau đè file trước. Đủ cho việc liệt kê service; **không** đủ nếu sau này muốn hiện port/env đã merge.
- Volume/network mà service khai báo nhưng chưa tạo — mới chỉ có service. Chưa có nhu cầu rõ.
