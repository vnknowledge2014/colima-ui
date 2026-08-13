# Bước 0 phase 2 — rà giấy phép nguồn rule

**Ngày:** 2026-08-12 · **Phase:** `plans/260812-1013-security-posture/phase-02-rule-pack-scoring-engine.md`
**Phán quyết: GO, nhưng chặt hơn plan viết.** Hai nguồn bị plan đánh giá nhẹ tay.

Mọi kết luận dưới đây kiểm từ nguồn gốc trong hôm nay, không phải từ trí nhớ. URL
và ngày ghi kèm để rà lại được.

## Bối cảnh quyết định mọi thứ: ColimaUI là sản phẩm thương mại

Có tier trả phí qua Polar (`docs/pricing-rationale.md`). Nên mọi điều khoản
**NonCommercial** đều áp dụng cho ta — đây không phải dự án hobby.

Và: **repo hiện không có file `LICENSE` lẫn `NOTICE` ở gốc.** Cần có trước khi
dùng bất kỳ tư liệu Apache-2.0 nào (xem "Việc phải làm").

## Phán quyết theo nguồn

| Nguồn | Giấy phép thật | Bằng chứng | Được làm gì |
|---|---|---|---|
| **CIS Docker Benchmark** v1.8.0 | **CC BY-NC-SA 4.0** cho bản PDF miễn phí | `cisecurity.org/benchmark/docker`: *"freely available in PDF format for **non-commercial use**"*; trang điều khoản trỏ "CIS Benchmarks™ (Free PDF Use) – Creative Commons License" thẳng tới `creativecommons.org/licenses/by-nc-sa/4.0/legalcode` | **Chỉ số hiệu** (`CIS-4.1`). Không copy tiêu đề, mô tả, lý do, cách sửa. Không trích dẫn nguyên văn dù ngắn |
| **Docker Bench for Security** | **Apache-2.0** (xác nhận qua metadata repo + `LICENSE.md`, repo còn hoạt động, commit gần nhất 2026-06) | `api.github.com/repos/docker/docker-bench-security` → `spdx_id: Apache-2.0` | Đọc **cách kiểm tra** (kiểm cái gì, ở field nào) và tự viết lại. Nếu dùng lại đoạn logic nào thì bắt buộc có `NOTICE` |
| **OWASP Docker Top 10** | **CC BY-NC-SA 4.0** — **plan ghi sai là CC-BY-SA** | `raw.githubusercontent.com/OWASP/Docker-Security/main/License.md` nguyên văn: *"CC-BY-NC-SA 4.0 International"* | Chỉ tham chiếu **mã mục** (`D01`…`D10`) và tên nhóm rủi ro. Không phái sinh văn bản. Điều khoản NC loại bỏ hẳn khả năng "dùng có ghi nguồn" trong sản phẩm bán tiền |
| **OWASP CSVS** | **CC BY-SA 4.0** (plan ghi đúng) — nhưng **repo đã archived, không đổi từ 2019-08** | `en/0x01-Frontispiece.md`: *"Copyright © 2019 OWASP Foundation. This document is released under the Creative Commons Attribution ShareAlike 4.0 license."* | Dùng **khái niệm L1/L2/L3 làm tên mức**, tự định nghĩa mỗi mức gồm rule nào. Không copy tiêu chí. ShareAlike lây sang tác phẩm phái sinh — tránh phái sinh là tránh luôn |
| **NIST SP 800-190** | Public domain (US Gov), có ngoại lệ | `nist.gov/oism/copyrights`: *"With the exception of material marked as copyrighted, information presented on NIST sites are considered public information and may be distributed or copied. Use of appropriate byline/photo/image credits is requested."* | **Trích dẫn được**, kể cả nguyên văn, kèm ghi nguồn. Là nguồn duy nhất trong bảng này được phép copy |
| **Trivy** (đang dùng, phase 1) | Apache-2.0, **detect-không-bundle** | Phase 1 không phân phối binary nào | Không nghĩa vụ phân phối lại. Giữ nguyên cách này |

## Hai đính chính cho plan

**1. CIS không phải "Terms of Use hạn chế" chung chung — nó là CC BY-NC-SA 4.0.**
Cụ thể hơn và tệ hơn cho ta: NC cấm dùng trong sản phẩm thương mại, SA buộc tác
phẩm phái sinh phải cùng giấy phép. Kết luận thực hành của plan ("chỉ tham chiếu
số hiệu, tự viết văn bản") **vẫn đúng** — nhưng lý do bây giờ cứng hơn: không
phải "nên tự viết cho lành", mà là **phái sinh văn bản CIS trong ColimaUI là vi
phạm giấy phép**.

**2. OWASP Docker Top 10 là CC BY-**NC**-SA, không phải CC-BY-SA.**
Plan xếp nó chung với CSVS và nói "được, nhưng nên tự viết". Sai ở chữ "được":
điều khoản NonCommercial nghĩa là kể cả ghi nguồn đầy đủ ta cũng **không** được
phái sinh trong sản phẩm bán tiền. Nó rơi vào cùng ô với CIS, không cùng ô với
CSVS.

Hệ quả gộp lại: trong 5 nguồn, **1 nguồn duy nhất (NIST) copy được**. Bốn nguồn
còn lại chỉ dùng được ở mức "số hiệu và ý tưởng kiểm tra".

## Ranh giới thực hành: cái gì là dữ kiện, cái gì là tác phẩm

Đây là phần quyết định rule pack v1 viết thế nào.

**Dữ kiện — dùng được:**
- Số hiệu rule: `CIS-4.1`, `CIS-5.12`, `D02`, `V03.1`.
- Tên chuẩn và phiên bản: "CIS Docker Benchmark v1.8.0".
- **Sự thật kỹ thuật**: Dockerfile không có `USER` thì container chạy root;
  `latest` không cố định nội dung; `ADD` giải nén và tải URL còn `COPY` thì không.
  Không ai sở hữu những điều này.

**Tác phẩm — không dùng được:**
- Tiêu đề rule nguyên văn của CIS/OWASP.
- Đoạn "Rationale"/"Description"/"Remediation" của họ, kể cả viết lại gần sát.
- Bảng điểm, trọng số, cách phân loại của họ.
- Cấu trúc tuyển chọn: bê nguyên thứ tự và tập rule của §4 CIS rồi đổi chữ vẫn là
  phái sinh.

**Quy trình viết rule pack v1 (bắt buộc):**
1. Đọc nguồn → ghi ra **hành vi cần kiểm** bằng một câu của mình, đóng nguồn lại.
2. Viết `title`/`rationale`/`remediation` từ hành vi đó, **không mở lại nguồn**.
3. Điền `standard_refs` bằng số hiệu — thêm sau khi văn bản đã viết xong, để
   tham chiếu là chú thích chứ không phải khuôn để chép theo.
4. Kiểm chứng bằng lệnh thật (`docker inspect`, `docker history`) — nếu không tự
   kiểm chứng được thì rule đó chưa hiểu đủ để tự viết.

## Việc phải làm trước khi viết rule đầu tiên

| # | Việc | Vì sao |
|---|---|---|
| 1 | Thêm `LICENSE` ở gốc repo | Chưa có. Mọi câu hỏi "chúng ta được phép làm gì" đều bắt đầu từ giấy phép của chính mình |
| 2 | Thêm `NOTICE` với mục Apache-2.0 (Docker Bench nếu dùng logic, Trivy nếu sau này bundle) | Apache-2.0 §4 yêu cầu; rẻ khi làm sớm |
| 3 | Trường `standard_refs` chỉ chứa `{ standard, id }` — **không có trường `text`** | Schema không cho chỗ dán văn bản thì không ai dán nhầm |
| 4 | Cấm chuỗi UI: "CIS Certified", "CIS compliant", "OWASP compliant", logo CIS/OWASP | "CIS Benchmarks™" là nhãn hiệu của họ; ngụ ý chứng nhận là vấn đề nhãn hiệu, độc lập với bản quyền. Nói được: *"tham chiếu CIS Docker Benchmark v1.8.0"* |
| 5 | Câu miễn trừ trong Security UI (phase 3) | Ta chấm điểm theo rule của mình, không phải điểm CIS. Không nói hay ngụ ý ngược lại |

## Rà chép — cách kiểm, không phải lời hứa

Success criteria của phase 2 nói "không dòng nào copy — rà thủ công". Rà thủ công
không kiểm chứng lại được. Đề xuất thay bằng thứ chạy được:

- Rule pack là JSON, nên viết một test đọc `v1.json` và **fail nếu bất kỳ chuỗi
  nào trong `title`/`rationale`/`remediation` khớp n-gram (n=8) với một corpus
  tham chiếu** để cạnh repo, không commit.
- Rẻ hơn và đủ cho v1: mỗi rule có `authored_by` + ngày, và PR thêm rule bắt buộc
  trả lời "văn bản này viết từ đâu". Ghi vào `CONTRIBUTING` khi có.

Chọn cách nào là việc của phase 2 bước 2; bước 0 chỉ nói rằng "rà thủ công" một
lần rồi tick không phải bằng chứng.

## Rủi ro còn lại

| Rủi ro | Đánh giá |
|---|---|
| Số hiệu rule CIS thay đổi giữa các phiên bản benchmark | Thật. `standard_refs` phải kèm **phiên bản** (`CIS Docker Benchmark v1.8.0 §4.1`), không phải `CIS-4.1` trần |
| CSVS chết từ 2019, tham chiếu tới một chuẩn không ai bảo trì | Chấp nhận: ta chỉ mượn **ý tưởng ba mức nghiêm ngặt**, không mượn nội dung. Nếu bỏ hẳn tên CSVS mà gọi L1/L2/L3 là mức của chính mình thì còn sạch hơn — đề xuất làm vậy |
| Ai đó sau này "bổ sung cho đầy đủ" bằng cách dán mô tả CIS vào | Việc #3 (schema không có trường text) là cái chặn thật. Bảng này thì không |

## Unresolved

1. **Giấy phép của chính ColimaUI chưa chốt.** Plan commercial-foundation nói
   "core-MIT" nhưng repo không có `LICENSE`. Rule pack là artifact dữ liệu trong
   repo đó — nó thừa hưởng giấy phép nào? Cần chốt trước khi rule pack v1 được
   commit, không phải sau.
2. **Có nên nêu tên CIS/OWASP trong UI không?** Nêu thì tăng độ tin nhưng kéo
   theo rủi ro nhãn hiệu và câu hỏi "sao điểm của các anh khác điểm CIS-CAT?".
   Không nêu thì rule pack tự đứng bằng lý lẽ của nó. Nghiêng về **chỉ nêu trong
   chi tiết từng rule, không nêu ở nhãn điểm tổng**.
3. **Chưa rà `docker-bench-security` ở mức mã nguồn.** Mới xác nhận giấy phép.
   Nếu bước 2 định đọc script của họ để biết field nào cần kiểm thì phải quyết
   trước: đọc-rồi-tự-viết (không cần NOTICE) hay port logic (cần NOTICE).
