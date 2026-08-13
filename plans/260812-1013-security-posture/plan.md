---
name: Security Posture — nhánh bảo mật container cho ColimaUI
date: 2026-08-12
status: in-progress — phase 1, 2, 4, 5, 6, 7, 8 xong; phase 3 xong phần UI (2026-08-12), kênh ký số chờ Unresolved #1
blockedBy:
  # Chỉ id trần — chú thích trong ngoặc khiến `ck plan status` không resolve được
  # ref và báo "not found" cho một plan vẫn đang tồn tại.
  #
  # 260811-2300-platform-prerequisites: XONG 2026-08-12, đã archive
  # (plans/archive/...) — P2 redact, P4 streaming, P7 entitlement đều sẵn sàng.
  #
  # 260811-2245-pro-tier-features: cần P1 (patch engine) và P2 (metrics+alerts).
  - 260811-2245-pro-tier-features
---

# Security Posture

Nhánh bảo mật container, dựng theo bốn quyết định chủ dự án đã chốt 2026-08-12
(xem `plans/reports/from-sequential-thinking-to-owner-260812-1005-security-branch-analysis.md`):

| # | Quyết định | Chốt |
|---|---|---|
| 1 | Scoring/recommendation | **Artifact tĩnh có ký số**, không dựng API server |
| 2 | Runtime detection | **Mặt tiền cho Falco**, không tự viết engine phát hiện |
| 3 | Red team / attack tools | **Hoãn**. Phục vụ Blue team trước |
| 4 | Tier | **Gộp vào boolean `paid`**, không mở SKU thứ ba |

## Overview

Ba lớp, ánh xạ thẳng vào entitlement boolean đang có. Không lớp nào cần backend của ta.

```
Lớp 1 — Security Posture (FREE)        phase 1, 2, 3
  scan image local → SBOM → điểm minh bạch → checklist → gợi ý image thay thế

Lớp 2 — Security Intelligence (PRO)    phase 4, 5
  AI diễn giải findings (BYOK) → hardening patch → lịch sử điểm → policy gate

Lớp 3 — Runtime & Sandbox (PRO, sau)   phase 6, 7
  detonation sandbox trong instance cô lập → mặt tiền Falco

Nội dung (mọi tier, gần như miễn phí)  phase 8
  honeypot pack trong Knowledge Bank
```

## Phases

| # | Tên | Tier | Chờ | File |
|---|---|---|---|---|
| 1 | Scanner capability + scan engine | Free | prereq P2, P4 | [phase-01](phase-01-scanner-capability-scan-engine.md) — **DONE 2026-08-12** |
| 2 | Rule pack + scoring engine | Free | phase 1 | [phase-02](phase-02-rule-pack-scoring-engine.md) — **DONE 2026-08-12** |
| 3 | Security UI + image catalog ký số | Free | phase 2 | [phase-03](phase-03-security-ui-signed-catalog.md) — **UI xong 2026-08-12; kênh ký chờ Unresolved #1** |
| 4 | AI diễn giải + hardening patch | Pro | phase 2, prereq P7, Pro P1 | [phase-04](phase-04-ai-triage-hardening-patch.md) — **XONG 2026-08-13** |
| 5 | Lịch sử điểm + policy gate + alert | Pro | phase 2, Pro P2 | [phase-05](phase-05-score-history-policy-gate.md) — **XONG 2026-08-13** |
| 6 | Detonation sandbox | Pro | phase 1, prereq P4 | [phase-06](phase-06-detonation-sandbox.md) — **XONG 2026-08-13** (v1 `--network none`) |
| 7 | Mặt tiền Falco | Pro | phase 5 | [phase-07](phase-07-falco-frontend.md) — **XONG 2026-08-13** |
| 8 | Honeypot pack (Knowledge Bank) | Free | — | [phase-08](phase-08-honeypot-kb-pack.md) — **DONE 2026-08-12** |

**Đường tới hạn:** 1 → 2 → 3 là thứ khiến người dùng chọn sản phẩm. Không được
hoãn để làm phase 6/7 trước, dù chúng thú vị hơn.

## Dependencies — ánh xạ chính xác, không phải chờ cả plan

| Phase ở đây | Chờ |
|---|---|
| 1 | prereq **P4** (`run_cmd_streaming` — scan chạy phút, không phải giây), prereq **P2** (redact mở rộng — SBOM lộ toàn bộ bill of materials) |
| 2 | phase 1 (cần shape findings đã chuẩn hoá) |
| 3 | phase 2 (điểm + rule pack) |
| 4 | phase 2; prereq **P7** (`ProGate` chế độ `paid`); Pro **P1** (`compose_autofix.rs` — patch engine dùng chung, khác rule pack) |
| 5 | phase 2; Pro **P2** (`alerts.rs` — security alert là **một loại** alert, cấm dựng đường ống thứ hai) |
| 6 | phase 1; prereq **P4** |
| 7 | phase 5 (dùng lại đường alert + kho sự kiện) |
| 8 | không chờ gì |

## Nền tảng tái dùng — đã grep-verify 2026-08-12

| Có sẵn, xác nhận đúng | Dùng ở |
|---|---|
| `commands/system_capabilities.rs::detect_all_blocking`, `capability()`, `install_hint()`, `doc_id()`, `invalidate()` | Phase 1 — thêm scanner vào đúng pattern detect hiện tại |
| `commands/system_capabilities.rs::CapabilityState` (4 trạng thái, snake_case wire) | Phase 1 |
| `redact.rs::redact:249` (sau prereq P2) | Phase 1, 4 |
| `commands/compose_diagnose.rs::error_signature:97`, `redact_compose:132` | Phase 4 |
| `commands/kb_articles.rs::Article:86`, `seed:140`, `kb_search_articles:256` | Phase 3, 8 |
| `pro/mod.rs::ProStatus`, `subscription/mod.rs` trường `paid` | Phase 4, 5, 6, 7 |
| `components/ProGate.svelte` (`variant="preview"`, `GATED_ENUM`) | Phase 4, 5, 6, 7 |
| `commands/kind.rs`, `instance_reader.rs`, `commands/shell_sandbox.rs` — tiền lệ sinh/huỷ môi trường tạm | Phase 6 |
| `routes/images.rs::api_list_images` + `routes/mod.rs` | Phase 1, 3 |

**Đã kiểm và KHÔNG có (đừng giả định là có):**
- Không có bất kỳ tích hợp scanner nào. `grep -ril 'trivy\|cve\|vulnerab'` chỉ khớp các file không liên quan (K8s health, path_util, api client).
- `routes/mod.rs` chưa có module `security`. Phase 1 sở hữu việc thêm.
- `pro/bridge.rs` vẫn là placeholder — **mọi gate ở đây dùng `paid`, không dùng capability id** (bài học prereq P7).

## Trạng thái phụ thuộc — đo thật 2026-08-12

Đo trực tiếp trên repo, thay cho phần suy đoán ban đầu:

Đo lại 2026-08-12 12:35, sau khi `260811-2300-platform-prerequisites` và
`260811-2241-free-tier-foundation` được archive:

| Phụ thuộc | Trạng thái thật | Bằng chứng |
|---|---|---|
| prereq **P4** streaming + path confinement | ✅ **XONG** | `streaming_cmd.rs`: `run_cmd_streaming:82`, `run_streaming:101`, `cancel_stream:155`, `kill_all_streams:173`, `active_stream_count:188`; `validation.rs assert_path_within` |
| prereq **P2** redact mở rộng | ✅ **XONG** | `redact.rs:262 redact`, 12 regex, test phủ AWS key, JWT, PEM, bearer, URL credential, key theo provider, token của chính app |
| prereq **P7** `ProGate` chế độ `paid` | ✅ **XONG** | `ProGate.svelte:64` — `capability?` giờ là **tuỳ chọn**; không truyền thì gate theo entitlement boolean. `pro.svelte.ts:174 isPaid`, `:123 onEntitlementChange` |
| Pro **P1** patch engine | ❌ Chưa | `commands/compose_autofix.rs` không tồn tại |
| Pro **P2** metrics store + alerts | ❌ Chưa | `commands/{alerts,metrics_store}.rs` không tồn tại. `metrics_collector.rs` có, nhưng đó là của Free P3 — chỉ là nguồn dữ liệu, chưa có kho lẫn đường alert |
| Scanner trên máy dev | ✅ **Đã cài** | `trivy 0.73.0`, `grype 0.117.0`. Bakeoff đã chạy → **chốt Trivy**, xem report 260812-1259 |

**Hệ quả:**

| Phase | Chạy được? |
|---|---|
| 1, 2, 3 | ✅ Không còn chặn về code. Phase 1 bước 0 cần cài scanner |
| 4 | ✅ Pro P1 xong 2026-08-13 (`compose_autofix.rs`) → phase 4 XONG |
| 5 | ✅ Pro P2 xong 2026-08-13 (`alerts.rs`, `metrics_store.rs`) → phase 5 XONG |
| 6 | ✅ Gate 0 đo xong 2026-08-13 — tạo instance 24s, tạo theo yêu cầu |
| 7 | ✅ XONG 2026-08-13 — Falco 0.44.1 aarch64 chạy trong Lima VM, có event thật |
| 8 | ✅ Xong 2026-08-12 |

Gate 0 của phase 6 + 7: `plans/reports/from-cook-gate0-to-owner-260813-0823-detonation-and-falco-viability-report.md`

## Ba quyết định kỹ thuật đã chốt sẵn cho mọi phase

**1. Detect scanner, KHÔNG bundle.**
Trivy DB sau giải nén ~700MB — không thể vào bundle. Bundle binary còn kéo theo
nghĩa vụ ký + notarize + chạy theo kịp CVE feed. Repo đã có pattern detect đúng
(`system_capabilities.rs`), dùng nguyên. Thiếu scanner → UI nói cách cài, y hệt
cách app đang xử lý `colima`/`kubectl` thiếu.

**2. Điểm là hàm thuần, luôn hiển thị đủ ba đầu vào.**
`score = f(findings, rule_pack_version, db_snapshot_date)`. Không bao giờ hiện
số trần. Một điểm tụt vì CVE DB cập nhật, trong khi image không đổi, mà không
giải thích được, là cách nhanh nhất làm mất niềm tin. Điểm tổng chỉ để **sắp
xếp danh sách**, không phải kết luận.

**3. Rule pack là artifact dữ liệu có phiên bản, không phải prompt.**
Nếu để LLM tự nhớ ID rule CIS, nó sẽ bịa. Rule pack nhúng trong binary làm
baseline (offline luôn chạy), cập nhật qua kênh artifact ký số của phase 3.

## Cảnh báo pháp lý — ĐÃ RÀ 2026-08-12 (bước 0 phase 2)

Bảng dưới là **kết quả kiểm từ nguồn gốc**, thay cho bảng ước đoán ban đầu. Chi
tiết + URL:
`plans/reports/from-security-phase2-step0-to-owner-260812-2109-rule-source-licensing-verdict-report.md`

| Nguồn | Giấy phép thật | Hệ quả |
|---|---|---|
| **CIS Docker Benchmark** v1.8.0 | **CC BY-NC-SA 4.0** (bản PDF miễn phí) | NonCommercial → ColimaUI có tier trả phí nên **phái sinh văn bản CIS là vi phạm**, không chỉ là "nên tránh". Chỉ tham chiếu số hiệu kèm phiên bản |
| **Docker Bench for Security** | Apache-2.0 | Được. Đọc cách kiểm tra rồi tự viết; port logic thì cần NOTICE |
| **OWASP Docker Top 10** | **CC BY-NC-SA 4.0** — bản đầu của plan ghi nhầm là CC-BY-SA | Cùng ô với CIS, không cùng ô với CSVS. Chỉ mã mục `D01`–`D10` |
| **OWASP CSVS** | CC BY-SA 4.0 (repo archived từ 2019) | Mượn ý tưởng ba mức L1/L2/L3, tự định nghĩa nội dung mỗi mức |
| **NIST SP 800-190** | Public domain (US Gov) | Nguồn duy nhất trong bảng copy được, kèm ghi nguồn |
| **Trivy / Grype / Syft** | Apache-2.0 | Detect-không-bundle né sạch mọi nghĩa vụ phân phối lại |

Quyết định 1 (detect, không bundle) giải quyết phần rủi ro của scanner. Phần rule
thì không tự khỏi: **4/5 nguồn chỉ dùng được ở mức số hiệu và ý tưởng.**

**Chặn trước, không nhắc nhở:** `standard_refs` không có trường văn bản; cấm chuỗi
UI ngụ ý chứng nhận ("CIS Certified", "OWASP compliant", logo CIS/OWASP).

**Đã trả:** `LICENSE` (MIT) và `NOTICE` đã có ở gốc repo (2026-08-12), nên rule
pack v1 thừa hưởng một giấy phép có thật.

## Acceptance chung

- Mọi tính năng Free chạy **hoàn toàn offline** sau khi scanner + DB đã có sẵn trên máy. Không gọi ra ngoài trừ khi người dùng bấm cập nhật catalog.
- Mọi output scan đi qua `redact::redact` **trước** khi vào diagnostic bundle, telemetry, hay payload LLM.
- Mọi gate Pro dùng `paid`; mọi hành vi nền kiểm tra `paid` **trong executor**, không phải lúc đăng ký (quy ước prereq P7).
- Không tính năng nào ở plan này tự động chạy khi app khởi động. Scan là hành động người dùng chủ động.
- Không có chuỗi UI nào hứa mức đảm bảo cao hơn thực tế ("phân tích malware an toàn" là câu bị cấm — xem phase 6).

## Ngoài phạm vi — có chủ đích

| Không làm | Vì sao |
|---|---|
| Attack tools / Metasploit / C2 / Kali images | Notarization của Apple + false positive từ EDR doanh nghiệp (chính là khách trả tiền). Chi phí trả bằng **toàn bộ** sản phẩm |
| Engine phát hiện rootkit tự viết | Rootkit sống dưới lớp Docker events. Cần eBPF + agent chạy root. Falco/Wazuh đã miễn phí và có đội ngũ lớn hơn ta nhiều lần |
| API server chấm điểm | Traffic tỉ lệ thuận với số image người dùng scan — đúng loại chi phí mà mô hình giá đang tránh |
| SKU "Security" riêng | Phá boolean entitlement, phá lý lẽ trong `docs/pricing-rationale.md` |
| Agent giám sát trên host remote do ta viết | Là phần mềm quyền root trên máy khách hàng. Nghĩa vụ "không được bỏ sót" biến công cụ thành dịch vụ |

## Còn lại để đóng plan — rà 2026-08-13

Plan **chưa xong**, nên chưa archive. Cụ thể còn:

| Việc | Vì sao chưa xong |
|---|---|
| Phase 3 — kênh catalog + rule pack ký số | Chưa có một dòng code nào: không `ed25519`, không verify chữ ký, không lệnh cập nhật catalog. Chặn bởi Unresolved #1 — **quyết định của chủ dự án**, không phải việc code |
| Unresolved #4 — ngưỡng điểm mặc định cho policy gate | Cần dữ liệu thật từ phase 2 |
| Unresolved #5 — kiểm offline bằng airplane mode | Là một trong các Acceptance chung; chưa ai ngắt mạng thử |
| Unresolved #6 — chi phí secret scanning của Trivy | Chưa đo; ảnh hưởng tốc độ và phạm vi dữ liệu chạm vào |
| Unresolved #7 — 2/23 image Trivy đọc hỏng | Chưa rõ nguyên nhân; nếu ~9% là tỉ lệ thật thì đường báo lỗi phase 1 quan trọng hơn dự tính |
| Chạy thật phase 6 + 7 | Cả hai chưa từng chạy đầu-cuối: phase 6 cần entitlement `paid` thật, phase 7 cần một máy có Falco đang chạy |

Phase 1, 2, 4, 5, 6, 7, 8 đã xong. Phase 3 xong nửa UI.

## Unresolved

1. **Ai ký catalog và rule pack, khoá riêng cất ở đâu?** Phase 3 cần khoá ed25519; quy trình ký (thủ công hay CI) chưa quyết. Ảnh hưởng: phase 3 không hoàn thành được nếu không có. **Đây là thứ duy nhất chặn việc đóng plan.**
2. ~~**Trivy hay Grype làm scanner mặc định?**~~ **ĐÃ CHỐT 2026-08-12: Trivy.**
   Bakeoff trên 23 image thật. Trivy nhanh hơn, DB nhỏ hơn (1.2GB vs 2.0GB), sinh
   SBOM native (bỏ được `syft` khỏi phase 1), đọc được daemon Colima mặc định.
   Grype không đọc docker context nên rơi xuống kéo image từ registry công cộng —
   làm rò tên image riêng tư. Xem `plans/reports/…-260812-1259-scanner-bakeoff-report.md`.
3. ~~**Có bundle Falco trong Lima VM không?**~~ **ĐÃ CHỐT 2026-08-13: người dùng tự cài.** Đo thật: cài mất 9 giây trong VM, bundle không thêm giá trị nào. Bài KB `install-falco` phụ trách phần hướng dẫn.
4. **Ngưỡng điểm mặc định cho policy gate ở phase 5** — cần dữ liệu thật từ phase 2 trước khi đặt số.
5. **Chưa kiểm offline thật.** Bakeoff mới chứng minh cờ `--skip-db-update` chạy
   được, chưa ngắt mạng. Phải kiểm bằng airplane mode **trước khi** UI hứa
   "hoạt động offline" — đó là một trong các Acceptance chung ở trên.
6. **Trivy bật secret scanning mặc định.** Nghĩa là nó đọc nội dung file bên trong
   image. Chưa đo chi phí thời gian, chưa quyết có tắt bằng `--scanners vuln`
   không. Ảnh hưởng cả tốc độ lẫn phạm vi dữ liệu chạm vào — liên quan trực tiếp
   tới cam kết riêng tư.
7. **Trivy đọc hỏng 2/23 image** (`not found in tar`). Chưa rõ bug của Trivy,
   của containerd, hay đặc thù image. Nếu ~9% là tỉ lệ thật với người dùng thì
   đường báo lỗi của phase 1 quan trọng hơn dự tính.
