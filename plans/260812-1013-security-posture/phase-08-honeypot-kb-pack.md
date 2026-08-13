# Phase 8: Honeypot pack (Knowledge Bank)

**Tier:** Free · **Chờ:** không chờ gì

## Overview

Honeypot là thứ **rẻ bất ngờ** trong plan này, vì nó không cần code mới. Nó là
compose stack + hướng dẫn. Repo đã có chỗ đặt: `commands/kb_articles.rs`
(`Article:86`, `seed:140`, `kb_search_articles:256`).

**Làm nó như nội dung, không làm như tính năng.** Nếu phase này bắt đầu cần
module Rust mới, đó là dấu hiệu đã đi sai hướng.

Phase này chạy được bất cứ lúc nào, kể cả song song với phase khác, kể cả trước
phase 1. Nó là phase duy nhất trong plan không có phụ thuộc.

## Requirements

**Functional**
- 3–5 bài Knowledge Bank, mỗi bài một honeypot phổ biến, kèm compose file chạy được.
- Mỗi bài: nó bắt gì, chạy thế nào trên Colima, đọc log ở đâu, và **khi nào KHÔNG nên dùng**.
- Một bài tổng quan: honeypot là gì, dùng để học hay để phát hiện, khác nhau ra sao.
- Compose file mở được thẳng bằng trang Compose đang có.

**Non-functional**
- Mọi honeypot **mặc định bind `127.0.0.1`**. Không bài nào hướng dẫn phơi ra internet mà không có cảnh báo tường minh ở ngay đoạn đó.
- 4 ngôn ngữ như mọi nội dung khác trong repo.
- Không tự động triển khai gì. Bài viết là bài viết.

## Nội dung đề xuất

| Bài | Honeypot | Bắt gì | Ghi chú |
|---|---|---|---|
| 1 | Tổng quan | — | Honeypot để học vs để phát hiện; rủi ro pháp lý khi phơi ra internet |
| 2 | Cowrie | SSH/Telnet brute force | Kinh điển, log dễ đọc, giá trị dạy học cao nhất |
| 3 | Dionaea | Malware qua SMB/HTTP/FTP | Cảnh báo: nó **thu thập mẫu malware thật** vào disk |
| 4 | OpenCanary | Cảnh báo dịch vụ giả | Nhẹ, gần với dùng thật nhất |
| 5 | Đọc log honeypot | — | Nối sang trang Activity và Compose logs đang có |

## Cảnh báo phải có trong bài 1 và bài 3

Không phải văn bản trang trí — đây là nội dung có hệ quả thật:

- Honeypot phơi ra internet **thu hút** lưu lượng độc hại tới địa chỉ IP của người dùng. Đó là quyết định có hậu quả, không phải trò vui.
- Dionaea lưu **malware thật** xuống disk. Trên máy có antivirus doanh nghiệp, việc này sẽ kích hoạt cảnh báo và có thể là quy trình xử lý sự cố của công ty.
- Ở một số nơi làm việc, chạy honeypot trên mạng công ty vi phạm chính sách nội bộ. Bài viết phải nói người dùng nên hỏi trước.

## Related Code Files

- Modify: `src-tauri/src/commands/kb_articles.rs::seed` — thêm article + slug
- Create: `src-tauri/resources/compose-examples/honeypot/{cowrie,dionaea,opencanary}.yml`
- Modify: `src/locales/{en,vi,ja,zh}.json` nếu có chuỗi UI mới (nhiều khả năng không)

## Implementation Steps

1. Viết bài 1 (tổng quan + cảnh báo) trước. Nếu chỉ làm được một bài, đây là bài đó.
2. Compose file cho từng honeypot, chạy thử thật trên Colima, xác nhận bind localhost.
3. Bài 2–4, mỗi bài có đoạn "khi nào không nên dùng".
4. Bài 5 nối sang trang Compose/Activity đang có.
5. Seed qua `kb_articles.rs::seed`, 4 ngôn ngữ.

## Tests

- Mỗi compose file `docker compose config` hợp lệ.
- Mỗi bài tồn tại ở cả 4 locale (`kb_list_articles` với mỗi locale).
- `kb_search_articles("honeypot")` trả về đủ bài.
- Không compose file nào bind `0.0.0.0` — grep khẳng định.

## Risks

| Rủi ro | Xử lý |
|---|---|
| Người dùng phơi honeypot ra internet rồi gặp rắc rối | Cảnh báo ở bài 1 và ở đầu mỗi bài có dịch vụ mạng, không giấu ở cuối |
| Dionaea kích hoạt antivirus doanh nghiệp | Nói trước, ngay trong bài |
| Phase phình thành "tính năng honeypot" có UI riêng | Ràng buộc cứng: **không module Rust mới**. Cần code mới = đã đi sai |
| Image honeypot lỗi thời/không maintain | Ghi ngày kiểm chứng cuối trong bài; ưu tiên image còn được maintain |

## Success Criteria

- Người dùng mở Knowledge Bank, đọc, copy compose, chạy được trên Colima, thấy log.
- Không có module Rust nào được tạo cho phase này.
- Mọi bài có cảnh báo dùng sai, đủ 4 ngôn ngữ.

---

## Đã thực thi — 2026-08-12

**Kết quả: 4 bài × 4 locale = 16 file. Không tạo module Rust nào.**

### Hai sai lệch so với spec, đều có lý do đo được

**1. Bỏ bài Dionaea.** Kiểm `docker manifest inspect` trên aarch64:

| Image | Kết quả |
|---|---|
| `cowrie/cowrie:latest` | ✅ manifest list, amd64 + arm64 |
| `thinkst/opencanary:latest` | ✅ manifest list, amd64 + arm64 |
| `dinotools/dionaea:latest` | ⚠️ manifest hợp lệ nhưng **chỉ amd64**, không có arm64 |

Máy dev là aarch64, nên Dionaea sẽ phải chạy qua emulation — chậm và thêm một
biến số cho một bài chỉ mang tính minh hoạ. Bỏ nó cũng gỡ luôn rủi ro antivirus
doanh nghiệp mà chính spec cảnh báo (Dionaea lưu malware thật xuống disk).
Còn 4 bài — vẫn trong khoảng "3–5 bài" spec cho phép.

> Ghi nhận sai sót: lần ghi đầu của mục này nói Dionaea "không có manifest".
> Sai — lệnh inspect thành công. Lý do đúng là thiếu arm64. Kết luận bỏ không
> đổi, nhưng căn cứ thì khác.

**2. YAML nằm trong markdown, không tách file `.yml` riêng.**
Spec yêu cầu `resources/compose-examples/honeypot/*.yml`. Bài viết mới là thứ
người dùng copy; một file `.yml` song song là bản sao thứ hai, tự do trôi lệch
khỏi bản người ta thật sự dùng. Một nguồn sự thật, và
`scripts/validate-kb-compose.sh` trích block YAML ra để kiểm thay vì kiểm file.

### File đã đụng

- Tạo: `src-tauri/resources/kb/{en,vi,ja,zh}/honeypot-{overview,cowrie,opencanary,logs}.md`
- Sửa: `src-tauri/src/commands/kb_articles.rs` — `ARTICLE_VERSION` 2→3, +4 `ARTICLE_META`, +16 `ARTICLE_BODIES`
- Tạo: `scripts/validate-kb-compose.sh`

### Code review đã tìm ra 7 lỗi — đã sửa hết

Reviewer chạy thật container thay vì chỉ đọc, và bắt được các lỗi nằm ngay trên
happy path. Tất cả đã sửa và kiểm lại:

| # | Lỗi | Sửa |
|---|---|---|
| 1 | Volume mount trỏ `/cowrie/var`, nhưng `WorkingDir=/cowrie/cowrie-git`. Mount tạo thư mục rỗng honeypot không bao giờ ghi → mất sạch session ở `down -v`, trong khi bài lại nói "bỏ `-v` để giữ lại" | Đổi sang `/cowrie/cowrie-git/{var,etc}`, kèm comment giải thích, 4 locale |
| 2 | `exec cowrie cat …` — image **distroless**, không có `cat` lẫn shell. Lệnh luôn fail | Đổi sang `docker compose cp`, giải thích vì sao, 4 locale |
| 3 | Đường dẫn `cowrie.json` sai | `/cowrie/cowrie-git/var/log/cowrie/cowrie.json`, 4 locale |
| 4 | Validator **false PASS** với `ports: ["0.0.0.0:2222:2222"]` (flow style) và với `network_mode: host` | Viết lại: assert trên `docker compose config --format json` thay vì grep dòng |
| 5 | Validator **false FAIL** với long syntax `host_ip:` | Hết, do dùng dạng đã chuẩn hoá |
| 6 | Mở cổng 2223 nhưng Telnet tắt mặc định trong `cowrie.cfg.dist` | Bỏ cổng, nói rõ Telnet tắt sẵn, 4 locale |
| 7 | `honeypot-logs.md` thiếu cảnh báo dùng sai (vi phạm tiêu chí phase) | Thêm mục "trước khi hành động": IP nguồn hay bị giả mạo/đi mượn, file tải về là file thù địch, đừng công bố log thô. 4 locale |

Cộng thêm một mục reviewer nêu: bind-mount `./opencanary.conf` ngoài thư mục
Colima chia sẻ sẽ khiến Docker tạo **thư mục** thay vì mount file → OpenCanary
không thấy config, chạy 0 dịch vụ, `restart: unless-stopped` lặp im lặng. Đã
thêm cảnh báo, 4 locale.

**Đã revert `ARTICLE_VERSION` về 2.** Reviewer đúng: slug mới không xung đột
`(slug, locale)` nên insert bất kể version; bump lên 3 khiến 24 dòng bài cũ bị
ghi đè và reindex FTS dù nội dung không đổi, phản lại chính doc comment và
invariant mà test `reseeding_does_not_rewrite_rows_at_the_same_version` bảo vệ.

### Kiểm chứng sau khi sửa

| Kiểm | Kết quả |
|---|---|
| `scripts/validate-kb-compose.sh` | ✅ 8 block hợp lệ, không gì lộ ra ngoài máy |
| Validator vs 4 ca tấn công của reviewer | ✅ bắt flow-style `0.0.0.0`, bắt `network_mode: host`, cho qua long-syntax loopback |
| `cargo test --lib` toàn bộ | ✅ 240/240, 0 fail |
| grep đường dẫn/lệnh cũ còn sót | ✅ chỉ còn trong comment giải thích, cố ý |

### Chưa chạy

Chưa `docker compose up` thật hai honeypot — cần pull image trên máy chủ dự án.
Compose **syntax** và **đường dẫn trong image** đã kiểm (qua `docker inspect` và
qua reviewer chạy thật); **vòng đời runtime đầy đủ** thì chưa tự tay chạy lại sau
khi sửa. Bước 2 của spec còn nợ phần này.
