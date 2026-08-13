---
title: "ColimaUI — Nền tảng thương mại (đã red-team)"
description: "Hạ tầng thương mại tối giản: validate nhu cầu trả tiền trước, dùng license key của MoR thay vì tự xây, danh mục Pro 9 mục nhưng launch với 3-4 do dữ liệu chọn"
status: in_progress
priority: P1
branch: "dev"
tags: [monetization, licensing, pro, billing]
# p0-polish-parity đã xong và archive (plans/archive/260810-2258-colimaui-p0-polish-parity/).
# Giữ nó trong blockedBy chỉ khiến `ck plan status` báo "not found" mãi.
blockedBy: []
blocks: [260811-2245-pro-tier-features]
created: "2026-08-10T16:26:43.536Z"
createdBy: "ck:plan"
source: skill
---

# ColimaUI — Nền tảng thương mại (đã red-team)

## Overview

**Phiên bản 2**, viết lại sau red-team. Bản v1 dự tính 7-9 tuần và, cộng với Plan 1, là **12-16 tuần trước khi biết có ai chịu trả tiền hay không** — trên một dự án mới 102 commit. Bản này đảo ngược thứ tự: **đo nhu cầu trước, xây sau**.

Nguồn: báo cáo chiến lược + 4 báo cáo red-team trong `plans/reports/`.

## Quyết định đã chốt (không mở lại)

| Quyết định | Lựa chọn |
|---|---|
| **License** | Core `colima-ui` giữ **MIT nguyên vẹn**; tính năng Pro ở repo private |
| **Kinh tế AI** | **BYOK ở mọi tier.** Pro mở khoá tính năng + bỏ giới hạn. Không có AI gateway backend |
| **Giá** | Pro $6/tháng, $60/năm (OrbStack $8) |
| **Quyền thương mại** | Free **được** dùng thương mại — khác biệt cốt lõi so với OrbStack |
| **Tài khoản** | **Native OAuth trong app (Google/GitHub) qua BaaS** — quyết định của chủ dự án, mở lại quyết định "không backend" cũ. Entitlement (đã trả tiền) **vẫn là license key MoR**, không phải account. Xem "Tài khoản & danh tính" |

## Tài khoản & danh tính

Mô hình chủ dự án chốt: đăng nhập native trong app (Google/GitHub, giống OrbStack)
tạo **tài khoản free**; free chưa mua vẫn đăng ký được; khi trả phí, tài khoản đó gắn
với **Pro chạy đa máy**.

**Chốt (ghi đè có ý thức — xem cảnh báo bên dưới):**

| Lớp | Ai sở hữu | Cách làm |
|---|---|---|
| Danh tính + OAuth Google/GitHub | **BaaS** (Supabase / Clerk / Auth0 / WorkOS) | App nhúng SDK; BaaS host callback + session + đăng ký app OAuth |
| Tài khoản free (chưa mua) | **BaaS** | Đăng ký/đăng nhập không cần mua |
| Subscription Pro | **MoR** | Checkout MoR như Phase 2 |
| **Entitlement (đã trả tiền)** | **License key MoR** | Vẫn là gốc sự thật; offline-first giữ nguyên |
| Đa máy | Giới hạn activation của key (MoR) | Kích hoạt máy mới bằng cùng key |

**Điểm mấu chốt của thiết kế:** account **chỉ là danh tính**. Pro vẫn quyết bởi
license key MoR — nên toàn bộ bài toán offline-first, chống version-skew, và cache
license state ở Phase 2/3 **không đổi**. Account down / mất mạng **không** hạ cấp
khách trả tiền, vì entitlement không đi qua account.

**Giá trị account mang lại (trung thực):** với entitlement nằm ở key và Free không
cần account, đăng nhập chủ yếu là **tiện lợi + cảm giác native như OrbStack**: nhớ
danh tính, và (nếu làm phần glue BaaS-user ↔ MoR-customer) tự lấy key sau khi đăng
nhập thay vì dán tay. Nó **không** mở khoá tính năng nào — tính năng vẫn theo key.

**UX lúc mở lần đầu:** **không** có màn login chặn. `SetupWizard` (kiểm Colima/Docker)
giữ nguyên; onboarding vẫn là "chạy container đầu tiên". Màn đăng nhập native **bỏ
qua được, không chặn** — Free chạy đầy đủ không cần đăng nhập, giữ điểm bán "không
bắt buộc tài khoản".

### Ghi đè quyết định — 2026-08-11 (Tài khoản)

| | Nội dung |
|---|---|
| **Quyết định cũ (đã chốt)** | "Không có backend." Trước đó cả BYOK lẫn tài khoản đều dựa nguyên tắc này |
| **Ghi đè của chủ dự án** | Native OAuth trong app như OrbStack. Chấp nhận một **hạ tầng auth phía sau** (chọn BaaS để không tự viết server) |
| **Cái phải trả** | Thêm 1 phụ thuộc BaaS + phí theo tháng; đăng ký OAuth app với Google + GitHub; luồng OAuth desktop (loopback/PKCE hoặc device code); thêm mặt tấn công; công sức đăng nhập native > link portal |
| **Cái được giữ** | Entitlement vẫn ở license key MoR → offline-first, chống skew, "Free không mất gì" **không bị ảnh hưởng**. BYOK-mọi-tier **vẫn nguyên** (auth ≠ AI gateway) |
| **Đã bác trong nhánh này** | **Tự viết + tự vận hành server auth** — cái giá cao nhất, không kiểm soát thêm gì so với BaaS |

**Lưu ý BYOK vẫn nguyên:** ghi đè này chỉ mở lại "không backend" cho *auth danh tính*.
Quyết định "BYOK ở mọi tier, không AI gateway" **không đổi** — không có server nào
đứng giữa người dùng và LLM.

**Team/Enterprise** (shared KB, seat management) vẫn **ngoài phạm vi** plan này; account
danh tính ở đây là nền cho việc đó về sau, nhưng phần sync/shared là plan riêng.

## Danh mục Pro

Danh mục là thứ **đăng lên trang giá**; nó không phải phạm vi launch. Phase 6 chỉ
mở bán với 3-4 mục, chọn theo dữ liệu waitlist Phase 1. Phần còn lại đi Phase 7.

Cột cuối là kỷ luật quan trọng nhất của bảng này: nguyên tắc 1 cấm paywall thứ
Colima/Docker CLI đã làm miễn phí. Vài ứng viên hấp dẫn nằm sát ranh giới đó và
được đánh dấu rõ thay vì lờ đi.

| # | Tính năng | Ước lượng | Vì sao Pro chứ không Free |
|---|---|---|---|
| 1 | **Compose auto-fix / chẩn đoán** (flagship) | Phase 5 | Docker báo lỗi, không sửa lỗi. Giá trị nằm ở KB + AI, không phải ở wrapper CLI |
| 2 | **Dockerfile optimizer / image lite** | 3-4 ngày | Sinh bản multi-stage + đo bằng build thật. Không CLI nào làm |
| 3 | **Vulnerability scan** (Trivy/Grype) | 4-6 ngày | ⚠️ **Sát ranh giới.** Trivy miễn phí. Chỉ hợp lệ nếu giá trị Pro là theo dõi theo thời gian + gắn KB + workflow, không phải cái nút chạy Trivy |
| 4 | **Self-healing** — tự phát hiện & khắc phục | 6-8 ngày | Không có tương đương CLI. Rủi ro cao: tự động hành động lên máy người dùng |
| 5 | **Activity Monitor lịch sử + alert** | 4-5 ngày | `docker stats` chỉ realtime, không lưu gì. Lưu trữ + ngưỡng cảnh báo là mới |
| 6 | **Registry manager + Keychain** | 3-4 ngày | ⚠️ **Sát ranh giới.** `docker login` miễn phí. Chỉ hợp lệ ở phần đa registry + lưu credential an toàn |
| 7 | **Backup/restore volume, snapshot VM** | 4-5 ngày | ⚠️ **Sát ranh giới.** Làm được thủ công bằng `docker run --rm -v … tar`. Giá trị là lịch trình + khôi phục có kiểm chứng |
| 8 | **Import từ Docker Desktop / OrbStack** | 5-7 ngày | Migration thật, không CLI nào làm. Giảm ma sát chuyển đổi — đáng làm nhất về mặt tăng trưởng |
| 9 | **Buildx multi-arch UI + build cache viewer** | 3-4 ngày | ⚠️ **Rủi ro cao nhất.** `docker buildx` đã miễn phí và đầy đủ. Nhiều khả năng vi phạm nguyên tắc 1 — cân nhắc loại |

### Loại khỏi Pro, có lý do

| Tính năng | Vì sao không |
|---|---|
| Container domains + auto HTTPS | Gap lớn nhất so với OrbStack — nhưng phải **Free**. Đây là vũ khí cạnh tranh, không phải hàng bán |
| Command palette ⌘K, phím tắt | Paywall UX cơ bản là dark pattern. Vi phạm nguyên tắc "Free là lựa chọn hợp lệ" |
| Đồng bộ cấu hình giữa máy | Cần hệ thống tài khoản mà không phase nào xây. Red-team #11 |
| AI hosted không giới hạn | Mâu thuẫn quyết định đã chốt: **BYOK ở mọi tier**. Bảng gói §5.1 của báo cáo benchmark sai ở ô này |
| Review PR | Lệch trọng tâm sản phẩm |

**Lưu ý về báo cáo benchmark:** bảng §5.1 trong
`plans/reports/from-research-to-product-strategy-…-freemium-roadmap-report.md`
ghi Pro = "AI hosted, không giới hạn" và Free = "BYOK 20 msg/ngày". Ô đó **không
còn hiệu lực** — quyết định BYOK-mọi-tier được chốt sau báo cáo đó.

## Thay đổi so với v1 (theo red-team)

| v1 | Vấn đề | v2 |
|---|---|---|
| Phase 1 = License Boundary (sidecar + JSON-RPC + repo private + CI + ký riêng), làm đầu tiên | Xây hạ tầng cấp phép ở doanh thu $0, trước khi biết có ai mua | **Phase 1 = trang giá + waitlist trả trước.** Đo nhu cầu ở tuần 1 |
| Tự xây license server: cấp key, fingerprint máy, grace, refresh, hàng đợi webhook, email | Toàn bộ là tính năng có sẵn của MoR. Và chính plan v1 viết "license check không phải DRM" | **Dùng license key của MoR.** 10-13 ngày → 3-4 ngày |
| Compose auto-fix 3 tầng + corpus 30 file, tầng 1 là quy tắc tất định tự viết | `grep -c '"config"' src-tauri/src/commands/compose.rs` = **0** — `docker compose config` chưa từng được gọi. Đang định xây lại thứ Docker cho không | **Spike trước:** ship `compose config` + truy vấn KB trong ~2 ngày, đo, rồi mới quyết định có xây tầng quy tắc không |
| Patch giữ nguyên comment/format là cổng launch | Dependency YAML duy nhất là `serde_yml 0.0.12` — **không có span, không có comment**. Không xây được | Bỏ khỏi cổng launch; nếu cần thì phải thêm crate mới và đánh giá riêng |
| Launch với 5 tính năng Pro | Trì hoãn học hỏi | **Launch với 3-4**, do dữ liệu waitlist chọn. Danh mục đầy đủ 9 mục nằm ở trang giá, không nằm trong phạm vi launch |
| `src-tauri/src/pro/registry.rs` nằm trong core MIT nhưng chứa logic Pro | Tự mâu thuẫn với chính ranh giới license | Core chỉ giữ **client gọi capability**, không giữ registry logic |
| Sidecar phân phối ra sao = câu hỏi treo | Auto-update thay bundle, sidecar ship riêng → **version skew làm khách trả tiền bị báo "không phải khách hàng"** | **Đã chốt: bundle-kèm** (ký cùng app) → core+sidecar luôn cùng version; core cache license state đã ký làm lưới an toàn |

## Nguyên tắc

1. Không paywall thứ Colima CLI đã làm miễn phí.
2. Không xây thứ MoR đã cung cấp.
3. Core MIT chạy đầy đủ và tử tế khi không có sidecar.
4. License là hàng rào lịch sự, không phải DRM. Không đầu tư chống crack.
5. **Mỗi phase phải trả lời được: điều này dạy ta gì về việc có ai chịu trả tiền không?**
6. **Hệ thiết kế component Pro** (badge PRO tím + preview blur + gating) theo
   `plans/reports/pro-ui-design-system-260811-1615-badges-blur-preview-gating-report.md`.
   Quy tắc vàng: **blur chỉ tease preview Pro-độc-quyền dựng sẵn, KHÔNG bao giờ che
   kết quả mà Free vốn tính được** (hệ quả trực tiếp của Nguyên tắc 1). Mọi tính năng
   Pro mới đọc report này trước khi thiết kế UI.

**Không thuộc phạm vi:** Teams, Enterprise, SSO, policy engine, cluster federation, Windows/WSL2.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Pricing & Paid Waitlist](./phase-01-pricing-paid-waitlist.md) | In Progress — phần in-app xong, website/MoR chưa |
| 2 | [Billing & License via MoR](./phase-02-billing-license-via-mor.md) | In Progress — license client+UI xong (Polar, core); cần org_id+key test để verify live |
| 3 | [Pro Boundary](./phase-03-pro-boundary.md) | In Progress — scaffolding core-MIT xong; cached_state/sidecar chờ MoR |
| 4 | [Telemetry & Auto-Update](./phase-04-telemetry-auto-update.md) | In Progress — telemetry/crash core xong; auto-update/ký hoãn (cần Apple Dev ID) |
| 5 | [Compose Auto-Fix Spike](./phase-05-compose-auto-fix-spike.md) | In Progress — Bước A code xong, chờ corpus để chạy cổng go/no-go |
| 6 | [Launch](./phase-06-launch.md) | Pending |
| 7 | [Pro Wave 2](./phase-07-pro-wave-2.md) | Pending |
| 8 | [Native Account (BaaS OAuth)](./phase-08-native-account-baas.md) | Pending — P2, không chặn launch |

## Ước lượng

| Phase | v1 | v2 | Ghi chú |
|---|---|---|---|
| 1 | — | **3-4 ngày** | Mới. Đo nhu cầu trước |
| 2 | 10-13 ngày (Ph2+3) | **4-6 ngày** | Dùng license key MoR |
| 3 | 5-8 ngày | **5-7 ngày** | Thêm xử lý version skew |
| 4 | 4-5 ngày | **4-5 ngày** | Thêm redaction cho crash report |
| 5 | 8-10 ngày | **2 ngày spike + 5-8 ngày có điều kiện** | Có cổng go/no-go |
| 6 | 8-10 ngày | **8-11 ngày** | 3-4 tính năng thay vì 2 — xem ghi chú bên dưới |
| 7 | — | **10-15 ngày** | Mới. Sau doanh thu, theo dữ liệu ProGate/churn |
| 8 | — | **5-8 ngày** | Mới. Native OAuth qua BaaS. **P2, không chặn launch** |
| **Tổng tới launch** | 7-9 tuần | **~5-6 tuần** | Phase 7-8 sau launch, không tính vào |

**Vì sao Phase 6 tăng từ 5-6 lên 8-11 ngày:** phạm vi launch đi từ 2 lên 3-4 tính
năng theo quyết định của chủ dự án (xem "Ghi đè quyết định" ở phần Red Team Review).

## Acceptance Criteria

- [ ] Có tín hiệu nhu cầu thật (Phase 1) **trước khi** xây hạ tầng cấp phép.
- [ ] `colima-ui` (MIT) build và chạy đầy đủ khi không có sidecar Pro.
- [ ] Không dòng mã nguồn Pro nào trong repo MIT — kể cả registry.
- [ ] Auto-update **không bao giờ** làm khách trả tiền bị nhận diện thành Free (chống version skew).
- [ ] Mua hàng end-to-end chạy thật ở production, có đường phục hồi khi khách trả tiền mà không nhận được key.
- [ ] Telemetry mặc định tắt; crash report đã qua redaction.
- [ ] Trang giá đăng **đủ danh mục Pro** với nhãn trạng thái trung thực; không mục nào đã-có bị ghi nhầm thành sắp-có hoặc ngược lại.
- [ ] 3-4 tính năng cổng launch **chọn từ dữ liệu waitlist**, không phải từ phỏng đoán.
- [ ] Không mục Pro nào paywall thứ Colima/Docker CLI đã làm miễn phí — 4 mục đánh dấu ⚠️ phải qua rà soát này trước khi xây.

## Rủi ro cấp plan

| # | Rủi ro | Mức | Xử lý |
|---|---|---|---|
| 1 | Không ai chịu trả tiền | Cao | Phase 1 phát hiện điều này trong tuần đầu thay vì tuần thứ 16 |
| 2 | **Version skew core/sidecar strand khách trả tiền** | Cao | **Đã chốt bundle-kèm** → core+sidecar luôn cùng version; thêm cache license state đã ký làm lưới an toàn |
| 3 | MoR từ chối/duyệt chậm (pháp nhân VN) | TB | Nộp hồ sơ ở Phase 1, song song với trang giá |
| 4 | Compose auto-fix không đủ giá trị | Cao | Cổng spike ở Phase 5 quyết định trước khi bỏ 8 ngày |
| 5 | Cộng đồng phản ứng | TB | Core MIT không mất gì; truyền thông trước; Free được dùng thương mại |
| 6 | Crash report rò rỉ PII | TB | Dùng chung `redact()` từ plan hotfix bảo mật |

## Dependencies

- **Blocked by:** `260810-2258-colimaui-p0-polish-parity` — ràng buộc ở **Phase 6 (launch)**. Phase 1-2 chạy song song được ngay.
- **Ngoại bộ:** tài khoản MoR, Apple Developer ID (ký + notarize — **hiện chưa có hạ tầng này**), domain + trang giá.

## Red Team Review

### Session — 2026-08-10
**Findings áp dụng cho plan này:** 8 (8 accepted) — phần thuộc plan thương mại trong tổng 15 finding sau dedupe
**Reviewers:** Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic

| # | Finding | Sev | Disposition | Áp dụng |
|---|---|---|---|---|
| 1 | 12-16 tuần trước khi có bất kỳ tín hiệu willingness-to-pay nào | Critical | Accept | **Phase 1 mới:** trang giá + waitlist ở tuần 1 |
| 2 | Version skew core/sidecar → khách trả tiền bị báo là Free; phân phối sidecar là câu hỏi treo | Critical | Accept | Phase 3 chặn cứng bởi câu hỏi phân phối; core cache license state đã ký |
| 3 | Tự xây license server là commodity — MoR đã có sẵn | High | Accept | Phase 2 dùng license key MoR: 10-13 ngày → 4-6 |
| 4 | Grace/refresh tự mâu thuẫn: "offline vô thời hạn" + refresh 7d + grace 14d = khoá sau 21 ngày | High | Accept | Phase 2 buộc chốt 1 mô hình thời hạn trước khi code |
| 5 | Trả tiền nhưng không nhận key → không có đường phục hồi | High | Accept | Phase 2 thêm trang lấy lại key + link trong app |
| 6 | `docker compose config` chưa từng được gọi — tier 1 xây lại thứ Docker cho không | High | Accept | Phase 5 đổi thành spike 2 ngày + cổng go/no-go |
| 7 | `serde_yml` không có span/comment → patch giữ format bất khả thi, mà lại là cổng launch | High | Accept | Phase 5 bỏ khỏi cổng launch, có 3 phương án thay thế |
| 8 | Crash report chứa PII/API key mà enum đóng không phủ; compose gửi LLM có secret | High | Accept | Phase 4 bắt buộc redact crash; Phase 5 lọc secret mặc định |
| 9 | `pro/registry.rs` trong core MIT chứa logic Pro — mâu thuẫn ranh giới | Medium | Accept | Phase 3 bỏ registry khỏi core; sidecar tự khai báo |
| 10 | Sidecar định vị qua env var = thực thi mã tuỳ ý | Medium | Accept | Phase 3: env override chỉ ở debug build |
| 11 | Launch 5 tính năng; config sync cần tài khoản mà không phase nào xây | Medium | Accept (phần "còn 2" **đã bị ghi đè** — xem Ghi đè quyết định) | Phase 6 còn 3-4, do dữ liệu chọn; config sync loại hẳn |

### Ghi đè quyết định — 2026-08-11

**Chủ dự án ghi đè một phần finding #11.** Được ghi lại ở đây để về sau không ai
tưởng đây là sơ suất.

| | Nội dung |
|---|---|
| **Quyết định gốc của red-team** | Launch với đúng 2 tính năng Pro. Lý do: học nhanh hơn từ 20 khách thật với 2 tính năng, hơn là đoán với 5 |
| **Quyết định của chủ dự án** | Launch với 3-4. Đồng thời bổ sung danh mục Pro đầy đủ vào plan + Phase 7 |
| **Lập luận ủng hộ** | Red-team tối ưu **tốc độ học**, không tối ưu **tỉ lệ chuyển đổi tại lúc quyết định mua**. Nếu giỏ hàng mỏng dưới ngưỡng ai đó chịu trả tiền, kết quả thu được là "không ai mua" mà không phân biệt được *mỏng quá* hay *sai tính năng* — thí nghiệm tự vô hiệu hoá |
| **Cái phải trả** | Launch chậm ~3-5 ngày so với bản 2 tính năng |
| **Cái được giữ** | Tinh thần của finding #11 vẫn nguyên: launch **không** ship cả 9 mục danh mục, và 3-4 mục kia **do dữ liệu chọn**, không do đoán |

Danh mục dày và launch mỏng không mâu thuẫn nhau: trang giá Phase 1 đăng đủ 9 mục
có nhãn trạng thái, waitlist đo xem người ta muốn cái nào, Phase 6 xây đúng cái đó.

### Whole-Plan Consistency Sweep
Đã đọc lại `plan.md` + 6 phase file sau khi áp dụng. Kiểm tra:
- Không phase nào còn nhắc tới license server tự xây, Ed25519 tự ký, fingerprint máy, hay hàng đợi webhook — đã thay bằng MoR.
- `pro/registry.rs` không còn trong danh sách file của bất kỳ phase nào; Phase 3 ghi rõ "KHÔNG có registry.rs".
- Corpus compose đổi từ 30 xuống ≥15 nhất quán giữa plan.md và Phase 5.
- Ước lượng trong bảng plan.md khớp với frontmatter `effort` của từng phase.
- Phase 4 đổi từ "chạy song song" sang `dependencies: [3]` — phản ánh trong cả plan.md lẫn phase file.
- `DiffView.svelte`: Phase 5 ghi "Reuse" từ plan P0 — không tạo trùng.
- **Không còn mâu thuẫn chưa giải quyết.**

## Unresolved Questions

1. ~~Chọn MoR nào?~~ **ĐÃ CHỐT (2026-08-11): Polar.** License key + activation limit
   + customer portal sẵn. Cần: tài khoản test-mode + product/price + test API key
   (đưa qua `.env`, không dán vào chat). Mở khoá Phase 2.
2. Pháp nhân bán hàng đặt ở đâu? Ảnh hưởng thuế và MoR.
3. ~~Sidecar Pro phân phối thế nào?~~ **ĐÃ CHỐT (2026-08-11): bundle kèm app đã ký.** Auto-update thay cả bundle → core+sidecar luôn cùng version, triệt tiêu version skew tại gốc. Xem Phase 3.
4. Ngưỡng nào ở Phase 1 thì coi là "đủ nhu cầu để đi tiếp"? Cần con số cụ thể trước khi chạy Phase 1.
5. Có cấp license miễn phí cho maintainer OSS / sinh viên không?
6. ~~Chọn BaaS auth nào?~~ **ĐÃ CHỐT (2026-08-11): Supabase Auth.** OAuth Google+GitHub
   only (signup = login đầu), badge chỉ danh tính. Chi tiết UI (login/profile/badge) +
   luồng redirect desktop (deep-link vs loopback) ở Phase 8.
