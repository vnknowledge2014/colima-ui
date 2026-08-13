# ColimaUI — Benchmark OrbStack & Định hướng Free / Pro / Teams / Enterprise

- Ngày: 2026-08-10
- Loại: Research + Product strategy (report-only, không thay đổi code)
- Phạm vi: audit hiện trạng repo → benchmark OrbStack → gap analysis → bổ sung roadmap → mô hình gói & lộ trình

---

## 1. Hiện trạng ColimaUI (audit repo)

| Mảng | Đã có |
|---|---|
| Nền tảng | Tauri 2 + Svelte 5 (runes) + Rust/Axum, dual-mode Desktop (IPC) & Browser (HTTP :11420 + SSE) |
| Docker | Containers, Images, Volumes, Networks, Compose (qua adapter `docker`/`nerdctl`/`compose`) |
| VM/Orchestrator | Colima instances, Lima VMs, Kind clusters, Kubernetes browser, X-Ray, Cluster Topology |
| AI | Agent 5-tool loop, sandbox 3 tier, Knowledge Bank SQLite (22+ solution, feedback 👍/👎), 9 provider LLM, Ollama models |
| Khác | Terminal xterm.js, Setup Wizard, Getting Started Tour, Resource Saver, Toast container, Bearer-token auth cho HTTP |
| Version | 0.1.10 — MIT License |

**Nhận định:** phần "làm được việc" đã khá đầy đủ, thậm chí vượt OrbStack ở AI và Kubernetes diagnostics. Thiếu chủ yếu là **độ đánh bóng (polish), networking DX, và toàn bộ hạ tầng thương mại hoá**.

---

## 2. Benchmark OrbStack (2026)

**Giá:** Free = chỉ dùng cá nhân/phi thương mại (đủ tính năng, trừ Debug Shell) · Pro **$8/tháng (~$80/năm)** cho commercial + priority support · Business/Enterprise: SAML SSO, invoice/PO, contact sales.

**Điểm mạnh cốt lõi:**
1. **Tốc độ** — khởi động ~2s, CPU/disk thấp, x86 emulation nhanh trên Apple Silicon.
2. **Container domain names + HTTPS zero-setup** — `service.project.orb.local`, tự sinh & cài cert, reverse proxy. Đây là tính năng "ghiền" nhất, gần như không đối thủ nào có.
3. **Grouping theo Compose project** ngay trong domain và UI.
4. **File sharing** — thư mục `~/OrbStack` truy cập trực tiếp container/volume/machine từ Finder.
5. **Kubernetes tích hợp** — `*.k8s.orb.local` cho LoadBalancer/Ingress, GUI + network integration.
6. UI mới định vị là "container IDE".

**Điểm yếu khai thác được:** **chỉ macOS**, closed-source, không có AI, không có Linux/Windows, VM engine độc quyền (khoá vendor).

---

## 3. Gap analysis — ColimaUI vs OrbStack

| Hạng mục | OrbStack | ColimaUI | Ưu tiên |
|---|---|---|---|
| Container domains + auto HTTPS | ✅ | ❌ | **P0 — gap lớn nhất** |
| Grouping theo Compose project | ✅ | ❌ (có trong plan) | P0 |
| File browser container/volume | ✅ | ❌ | P1 |
| Tray icon / menu bar | ✅ | ❌ (có trong plan) | P0 |
| Tốc độ khởi động | ~2s | phụ thuộc Colima | P1 (benchmark & tuning) |
| AI assistant / auto-fix | ❌ | ✅ **lợi thế** | giữ & mở rộng |
| K8s X-Ray / topology | một phần | ✅ **lợi thế** | giữ |
| Linux support | ❌ | ✅ **lợi thế** | marketing |
| Windows (WSL2) | ❌ | ❌ | P2 — cơ hội chiếm thị trường |
| Account / license / billing | ✅ | ❌ | P1 — chặn monetize |
| SSO/SAML, audit, policy | ✅ (Ent) | ❌ | P3 |

---

## 4. Rà soát plan của bạn + bổ sung

### 4.1 Các mục bạn đã liệt kê — đánh giá nhanh

| Mục | Đánh giá | Tier đề xuất |
|---|---|---|
| Suggestion docker image lite → write Dockerfile | Tốt, khác biệt; gắn AI | Free (basic) / Pro (AI optimize + multi-stage + so sánh size) |
| Tray icon macOS | Bắt buộc để ngang OrbStack | Free |
| Grouping containers | Bắt buộc | Free |
| Fix failed → toast, xem lỗi ở đâu | **Ưu tiên #1** — hiện lỗi im lặng là killer | Free |
| Tooltip, table resize | Polish cơ bản | Free |
| AI Assistant + Terminal inside | Đã có nền; cần gộp thành 1 khung "Copilot" | Free (BYOK, giới hạn) / Pro (hosted, không giới hạn) |
| Trạng thái khi chưa cài gì | **Ưu tiên cao** — quyết định tỉ lệ giữ chân người dùng mới | Free |
| Bộ knowledge hướng dẫn cài Colima/docker CLI | Vừa là DX vừa là SEO/content marketing | Free |
| Config Colima như Orb | Bắt buộc | Free (basic) / Pro (profile/preset, sync) |
| Personal account | Nền tảng monetize | hạ tầng |
| Graph view | Đẹp nhưng không phải nỗi đau → không nên là điểm bán chính | Pro |
| Activity Monitor | Nỗi đau thật (RAM/CPU trên Mac) | Free (basic) / Pro (history, alert) |
| File transfer + export image TAR | Ngang OrbStack | Free (export) / Pro (file browser 2 chiều) |
| Teams account | Cần định nghĩa rõ giá trị (xem §5.3) | Team |
| Cluster + transfer giữa cụm | Phức tạp, thị trường hẹp | Enterprise, sau cùng |
| Diagnostic log error / report bug | Có sẵn nền AI diagnostics | Free (local) / Pro (upload + tracking) |
| Self-healthy | Khác biệt mạnh nếu làm đúng | Pro |
| **Auto fix docker compose error → recommendation** | **Nỗi đau đúng nhất, moat rõ nhất** | Pro — nên là flagship |
| Review PR | Lệch trọng tâm sản phẩm (đó là việc của GitHub/Copilot) | **Đề xuất bỏ hoặc hoãn vô thời hạn** |

### 4.2 Bổ sung còn thiếu (quan trọng)

**Nhóm A — DX / cạnh tranh trực tiếp**
1. **Container domain names + auto HTTPS** (`app.project.colima.local`) — gap lớn nhất so với OrbStack, và là lý do người ta *ở lại*.
2. **Port & proxy manager** — bảng tất cả port đang mở, phát hiện xung đột port.
3. **File browser cho container/volume/VM** (đọc-ghi, drag & drop).
4. **Command palette ⌘K** + phím tắt toàn cục — chuẩn "IDE".
5. **Registry manager** — đăng nhập private registry, quản lý credential an toàn (Keychain).
6. **Buildx / multi-arch build UI** + build cache viewer.
7. **Devcontainer / dev environment** support.
8. **Backup–restore volume, snapshot VM**.
9. **Import/migrate từ Docker Desktop & OrbStack** — giảm ma sát chuyển đổi, cực kỳ đáng làm.
10. **Windows qua WSL2** — OrbStack không có; đây là cửa mở thị trường lớn nhất.

**Nhóm B — Hạ tầng sản phẩm (bắt buộc trước khi bán)**
11. Auto-update + release channel (stable/beta).
12. Crash reporting + telemetry **opt-in** (không có số liệu thì không biết ưu tiên cái gì).
13. Onboarding funnel & activation metric (định nghĩa "aha moment": chạy được container đầu tiên).
14. i18n hoàn chỉnh (đã có `src/locales`) — EN/VI trước.
15. Accessibility (keyboard nav, contrast) — bắt buộc cho bán vào doanh nghiệp.

**Nhóm C — Bảo mật & doanh nghiệp**
16. **Image vulnerability scanning** (Trivy/Grype) — món premium dễ bán nhất.
17. SBOM export, license compliance — Enterprise.
18. Policy engine: chặn `--privileged`, chặn image không trong allowlist, bắt buộc resource limit.
19. Audit log (ai làm gì, khi nào).
20. Secrets management cho compose/env.

**Nhóm D — Kinh doanh**
21. **Vấn đề license MIT — xem §6, đây là blocker số 1.**
22. Chọn Merchant of Record (Polar / Paddle / LemonSqueezy) — quan trọng vì pháp nhân VN bán ra quốc tế; tránh Stripe trực tiếp lúc đầu.
23. Kiến trúc cấp phép: license key ký khoá (Ed25519), **offline verification**, grace period, seat management, air-gapped license cho Enterprise.
24. Kinh tế đơn vị của AI: BYOK (miễn phí, người dùng trả LLM) vs hosted (Pro, ta trả) — phải có quota/rate-limit ngay từ ngày đầu nếu không biên lợi nhuận âm.

---

## 5. Mô hình gói đề xuất

### Nguyên tắc nền

1. **Không paywall thứ Colima CLI đã làm miễn phí.** Paywall `docker run` → người dùng quay lại terminal, mất luôn.
2. **Không copy mô hình "free = phi thương mại" của OrbStack.** Đó là điểm gây bực bội lớn nhất của OrbStack và là chỗ bạn nên đánh: **ColimaUI free cho cả thương mại**. Doanh nghiệp trả tiền vì *AI + team + compliance*, không phải vì "được phép dùng".
3. Chỉ tính phí ở 3 chỗ: (a) chi phí biên thực (AI hosted, sync), (b) giá trị đội nhóm (chia sẻ, quản trị), (c) yêu cầu doanh nghiệp (SSO, audit, policy).

### 5.1 Bảng gói

| | **Free** | **Pro** — $6/tháng, $60/năm | **Team** — $12/user/tháng | **Enterprise** — liên hệ |
|---|---|---|---|---|
| Dùng thương mại | ✅ | ✅ | ✅ | ✅ |
| Toàn bộ Docker/K8s/VM/Compose | ✅ | ✅ | ✅ | ✅ |
| Tray icon, grouping, tooltip, resize, terminal | ✅ | ✅ | ✅ | ✅ |
| Container domains + HTTPS | ✅ | ✅ | ✅ | ✅ |
| Knowledge base cài đặt | ✅ | ✅ | ✅ | ✅ |
| AI assistant | BYOK, 20 msg/ngày | Hosted, không giới hạn hợp lý | + shared context | + model riêng / on-prem |
| **Compose auto-fix + recommendation** | gợi ý cơ bản | ✅ đầy đủ | ✅ | ✅ |
| Dockerfile optimizer / image lite | gợi ý | ✅ multi-stage, so sánh size | ✅ | ✅ |
| Self-healing (tự phát hiện & khắc phục) | ❌ | ✅ | ✅ | ✅ |
| Activity Monitor lịch sử + alert | realtime cơ bản | ✅ | ✅ | ✅ |
| Graph view / topology nâng cao | cơ bản | ✅ | ✅ | ✅ |
| File browser 2 chiều, transfer | export TAR | ✅ | ✅ | ✅ |
| Vulnerability scan | ❌ | ✅ local | ✅ + báo cáo | ✅ + SBOM/compliance |
| Sync settings/profile giữa máy | ❌ | ✅ | ✅ | ✅ |
| **Shared Knowledge Bank của team** | ❌ | ❌ | ✅ | ✅ |
| Shared compose/env template, preset Colima | ❌ | ❌ | ✅ | ✅ |
| Seat management, billing tập trung | ❌ | ❌ | ✅ | ✅ |
| Policy engine, audit log | ❌ | ❌ | cơ bản | ✅ đầy đủ |
| SSO/SAML, SCIM | ❌ | ❌ | ❌ | ✅ |
| Air-gapped license, on-prem AI | ❌ | ❌ | ❌ | ✅ |
| Cluster federation & transfer | ❌ | ❌ | ❌ | ✅ |
| Hỗ trợ | community | email | ưu tiên | SLA + onboarding |

**Vị thế giá:** Pro $6 < OrbStack $8, **và** free-tier cho phép thương mại → thông điệp bán hàng cực rõ: *"Tất cả những gì OrbStack Pro làm, miễn phí — trả tiền chỉ khi cần AI và làm việc nhóm."*

### 5.2 Đường nâng cấp Free → Pro
Kích hoạt tự nhiên: hết quota AI → chạm nút "Auto-fix compose" → thấy kết quả scan bị mờ → cần sync sang máy thứ 2. Tất cả đều là **giá trị đã trải nghiệm rồi mới trả tiền**, không phải chặn cứng.

### 5.3 Đường Pro → Team (phần bạn cần định nghĩa rõ nhất)
Team **không phải** "Pro nhân số ghế". Team bán 4 thứ mà cá nhân không tự có:
1. **Shared Knowledge Bank** — lỗi 1 người đã fix, cả team không gặp lại. Đây là moat mạnh nhất và tăng theo thời gian sử dụng (data network effect).
2. **Chuẩn hoá môi trường** — preset Colima + compose template dùng chung → chấm dứt "máy tao chạy được".
3. **Onboarding dev mới** — 1 click dựng đúng môi trường của dự án.
4. **Quản trị** — ai đang dùng gì, policy, chi phí tập trung.

### 5.4 Team → Enterprise
Kích hoạt bởi: bộ phận mua sắm & bảo mật (SSO, audit, SBOM, air-gap), quy mô >50 ghế, hoặc yêu cầu on-prem AI.

---

## 6. Rủi ro & chặn (đọc trước khi làm)

| # | Rủi ro | Mức | Xử lý |
|---|---|---|---|
| 1 | **License MIT** — không thể paywall bền vững, ai cũng fork bỏ license check | **Chặn cứng** | Chọn 1: (a) giữ core MIT, tính năng Pro nằm **repo đóng riêng**, nạp dưới dạng plugin/binary; (b) đổi core sang **BSL/FSL/Elastic v2** cho code *mới* (code cũ đã phát hành vẫn MIT vĩnh viễn); (c) open-core với CLA. **Khuyến nghị: (a) — sạch pháp lý nhất, giữ được thiện chí cộng đồng.** |
| 2 | Biên lợi nhuận AI hosted âm | Cao | Quota cứng, cache câu trả lời qua Knowledge Bank, ưu tiên model rẻ, mặc định BYOK ở Free |
| 3 | Colima/Lima upstream thay đổi | TB | Adapter layer đã có — giữ nguyên, thêm integration test theo version |
| 4 | Tốc độ thua OrbStack (do Colima) | TB | Benchmark công khai; tối ưu VZ + virtiofs; nếu vẫn thua thì đổi trục cạnh tranh sang AI + đa nền tảng |
| 5 | Sandbox AI bị vượt → chạy lệnh phá hoại | Cao | Đã có 3 tier; cần thêm audit lệnh + fuzz test cho tầng banned |
| 6 | Thu thanh toán quốc tế từ pháp nhân VN | TB | Dùng MoR (Polar/Paddle/LemonSqueezy) |
| 7 | Làm quá nhiều thứ cùng lúc | **Cao** | Phần lớn plan hiện tại là "advanced". Đề xuất **đóng băng Nhóm Advanced** cho đến khi Free đủ bóng bẩy và có 1.000 người dùng hoạt động |

---

## 7. Lộ trình đề xuất

**P0 — Polish & ngang bằng (0–8 tuần) — mục tiêu: giữ chân**
Toast/hiển thị lỗi rõ nguồn · empty state khi chưa cài gì · tray icon · grouping theo compose · tooltip + table resize · **container domains + HTTPS** · Colima config editor · knowledge base cài đặt · auto-update · telemetry opt-in.
→ *Cổng:* 1.000 MAU, activation >60%, crash-free >99%.

**P1 — Nền tảng thương mại + Pro v1 (8–18 tuần) — mục tiêu: doanh thu đầu tiên**
Account + license key + MoR billing · **Compose auto-fix (flagship)** · Dockerfile optimizer · self-healing · Activity Monitor lịch sử · sync settings · vulnerability scan · file browser + export TAR.
→ *Cổng:* tỉ lệ chuyển đổi Free→Pro ≥2%.

**P2 — Team (18–30 tuần)**
Workspace, shared Knowledge Bank, shared preset/template, seat management, audit cơ bản, policy cơ bản. Song song: **Windows/WSL2**.
→ *Cổng:* 10 team trả phí.

**P3 — Enterprise (30 tuần+)**
SSO/SAML + SCIM · audit đầy đủ · SBOM/compliance · air-gapped + on-prem AI · cluster federation & transfer.

**Hoãn/bỏ:** Review PR (lệch trọng tâm) · Graph view làm sau P1 (đẹp nhưng không phải nỗi đau).

---

## 8. Việc cần làm ngay (tuần này)

1. **Quyết định license** — mọi thứ khác phụ thuộc vào đây.
2. Bật telemetry opt-in + crash reporting → có số liệu để ưu tiên.
3. Sửa toàn bộ luồng lỗi im lặng (toast + "lỗi ở đâu") — đây là thứ khiến người dùng bỏ đi mà bạn không biết.
4. Viết trang so sánh public **ColimaUI vs OrbStack vs Docker Desktop** — nhấn: miễn phí cho thương mại, có Linux, có AI.

---

## Câu hỏi chưa giải quyết

1. Chọn hướng license nào (repo đóng cho Pro / đổi sang BSL / open-core + CLA)?
2. Pháp nhân bán hàng đặt ở đâu (VN hay nước ngoài)? Quyết định này chi phối lựa chọn MoR và thuế.
3. AI hosted ở Pro: ta trả tiền LLM, hay Pro chỉ mở khoá tính năng còn người dùng vẫn BYOK? (ảnh hưởng trực tiếp tới giá $6)
4. Có cam kết làm Windows/WSL2 không? Nếu có thì đó là bet lớn nhất và phải vào roadmap sớm hơn P2.
5. Mục tiêu thực tế: sản phẩm sinh lời độc lập, hay công cụ tạo uy tín cho mảng dịch vụ?

---

## Nguồn

- [OrbStack Pricing 2026 — Toolradar](https://toolradar.com/tools/orbstack/pricing)
- [Docker Desktop vs OrbStack (2026)](https://usedocker.com/vs/orbstack)
- [OrbStack vs Docker Desktop — Sliplane](https://sliplane.io/blog/orbstack-vs-docker)
- [Container domain names — OrbStack Docs](https://docs.orbstack.dev/docker/domains)
- [Features — OrbStack Docs](https://docs.orbstack.dev/features)
- [Kubernetes — OrbStack Docs](https://docs.orbstack.dev/kubernetes/)
- [OrbStack Deep Dive — The New Stack](https://thenewstack.io/orbstack-a-deep-dive-for-container-and-kubernetes-development/)
