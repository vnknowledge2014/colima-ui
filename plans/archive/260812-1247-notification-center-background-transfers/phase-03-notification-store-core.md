---
phase: 3
title: Notification store core
status: completed
priority: P1
dependencies: []
effort: M
---

# Phase 3: Notification store core

## Overview

**Mở rộng** `src/store/errorLog.svelte.ts` thành một notification store, thay vì
thêm store thứ hai cùng hình dạng. Bản plan đầu định tạo
`src/store/notifications.svelte.ts` mới — red team chỉ ra đó là trạng thái trùng lặp
dưới tên khác.

## Vì sao mở rộng chứ không tạo mới (finding #6)

`errorReporter.ts:32-33` đã làm đúng việc này cho mọi lỗi:
```ts
recordError(err, ctx.action);
globalToast("error", text, { error: err, hint: errorHint(err) });
```
`errorLogState` đã là: session-scoped, không persist, có cap (`MAX_ENTRIES = 50`),
có `isOpen`, có panel hiển thị (`ErrorDetailPanel.svelte`). Một store mới sẽ là bản
sao có thêm hai field.

Thứ còn thiếu chỉ là: entry không-phải-lỗi, entry có vòng đời (job đang chạy), và
cờ đã đọc. Đó là mở rộng, không phải hệ thống mới.

**Đổi tên file:** `errorLog.svelte.ts` → `notifications.svelte.ts`, giữ
`recordError()` như một hàm bọc mỏng để 100% call site hiện tại không phải đụng.
Đây là đổi tên có ý nghĩa: file không còn chỉ chứa lỗi.

## Requirements

**Functional**
- Entry mô tả được cả "đang chạy có progress" lẫn "đã kết thúc".
- Đánh dấu đã đọc / đọc tất cả; đếm chưa đọc (hàm đọc từ entries, không phải bộ
  đếm riêng — bộ đếm sẽ lệch khi cap cắt bớt).
- `settleJob` với `jobId` chưa biết **tạo entry mới**, không phải no-op (finding #3).
- Đối soát được với `transfer_list` từ Phase 2.

**Non-functional**
- `recordError()` và `globalToast()` giữ nguyên chữ ký — **130 call site** trên
  32 file không được đụng (bản plan đầu ghi "~100", sai).
- Không persist.

## Kiến trúc

```ts
export type NotificationKind = "job" | "message";
export type NotificationStatus =
  | "running" | "cancelling" | "success" | "error" | "cancelled"
  /** Đã kết thúc, nhưng client này không thấy kết thúc thế nào. */
  | "ended";

export interface NotificationEntry {
  id: number;
  kind: NotificationKind;
  status: NotificationStatus;
  title: string;
  detail?: string;
  timestamp: number;
  read: boolean;
  /** Chỉ với kind === "job". Không chứa closure — xem ghi chú dưới. */
  job?: { jobId: string; bytes: number; totalEstimate: number | null };
  error?: AppError;
  hint?: string;
  action?: string;   // giữ từ ErrorLogEntry
}
```

**Không lưu closure `cancel` trong state** (finding Medium): bản plan đầu định để
`cancel: () => void` trong `$state`. Nó bắt scope của dialog, không serialize được,
và trùng với `transferApi.cancel(jobId)` (`src/lib/api/transfer.ts:91`). Panel gọi
thẳng `transferApi.cancel(entry.job.jobId)`.

**`kind: "announcement"` bị bỏ** — plan cloud tách riêng sẽ tự thêm khi cần. Thêm
sớm là abstraction non.

## Chống ngập — cách cũ sai (finding #6)

Bản plan đầu: "toast bị collapse thì không push entry". Không chạy được, vì
`dismissToast` xoá khỏi `_active` khi hết TTL (`globalToast.ts:71-84`, TTL 4-9s ở
`:42`). Bất cứ nguồn nào lặp chậm hơn TTL đó sẽ không tìm thấy toast để gộp vào và
sinh entry mới mỗi chu kỳ — đúng thứ nó định chặn.

*(Bản trước của đoạn này viện dẫn `dataPoller.ts` làm ví dụ. Sai — module đó không
phát toast lỗi nào. Lập luận vẫn đứng, ví dụ thì không.)*

**Cách đúng:** gộp ở tầng store theo `(kind, status, title-đã-chuẩn-hoá)` với bộ đếm
`count` và `lastSeen`, không phụ thuộc vòng đời toast. Giống cách `globalToast` gộp,
nhưng cửa sổ dài hơn nhiều và độc lập với TTL hiển thị.

**Cap:** giữ `MAX_ENTRIES` nhưng **loại trừ job đang chạy khỏi việc bị đuổi**
(finding Medium: FIFO có thể đuổi mất job đang chạy, mâu thuẫn với Phase 5).

## Related Code Files

- Rename + Modify: `src/store/errorLog.svelte.ts` → `src/store/notifications.svelte.ts`
- Create: `src/store/notifications.test.ts`
- Modify: `src/lib/errorReporter.ts` — import path
- Modify: `src/components/ErrorDetailPanel.svelte` — import path, kiểu entry
- Modify: `src/lib/globalToast.ts` — push entry cho toast không đến từ `errorReporter`
  (tránh ghi hai lần cho cùng một lỗi)
- Modify: mọi file import `store/errorLog.svelte` (dùng grep, không đoán)

## Implementation Steps

1. Đổi tên file, mở rộng `ErrorLogEntry` → `NotificationEntry`, giữ `recordError`
   như wrapper.
2. Cập nhật mọi import (grep `store/errorLog`).
3. Thêm `pushNotification`, `updateJob`, `settleJob`, `markRead`, `markAllRead`,
   `unreadCount` (đọc từ entries, không đếm tay), `reconcileJobs(snapshots)`.
4. `settleJob` id lạ → tạo entry đã kết thúc, không no-op.
5. Gộp theo khoá store-level; cap loại trừ job đang chạy.
6. Trong `globalToast()`: chỉ push khi lỗi **không** đến từ `errorReporter`
   (nếu không sẽ có hai entry cho một lỗi).
7. Test: id lạ tạo entry; job đang chạy không bị cap đuổi; lỗi qua `errorReporter`
   chỉ sinh một entry; gộp hoạt động qua nhiều chu kỳ TTL.

## Success Criteria

- [x] Một lỗi qua `errorReporter` sinh đúng **một** entry (không phải hai).
      — cơ chế là `ToastOptions.record`, cờ tường minh chứ không suy đoán từ việc
      có `error` hay không. Có test.
- [x] Lỗi lặp liên tục không sinh quá vài entry. — gộp ở tầng store với cửa sổ 5
      phút, độc lập với TTL hiển thị của toast.
- [x] Job đang chạy không bị đuổi khỏi store dù có bao nhiêu entry mới.
- [x] `settleJob("khong-ton-tai", ...)` tạo entry đã kết thúc.
- [x] `ErrorDetailPanel` vẫn hoạt động sau khi đổi tên.
- [x] `pnpm test` xanh; không còn import nào trỏ `store/errorLog`.

## Kết quả

vitest 210/210 (20 test mới cho store) · `svelte-check` 0/0 · eslint sạch.

Blast radius nhỏ hơn dự đoán: chỉ **3 importer** trực tiếp (`errorReporter.ts`,
`ToastContainer.svelte`, `ErrorDetailPanel.svelte`). 130 call site kia đi qua
`globalToast`/`reportError` nên không phải đụng.

### Code review sửa được một lỗi thiết kế thật

Bản đầu của store settle job "backend không còn liệt kê" thành **`success`**. Sai:
registry chỉ giữ job đã kết thúc 60 giây, nên một client offline lâu hơn thế quay
lại sẽ thấy job vắng mặt — và một export **thất bại** bị báo là đã xong. Thêm trạng
thái `ended` ("đã kết thúc, client này không thấy kết thúc thế nào") thay vì đoán.

Năm sửa khác từ review:
- Reconcile không được hạ `cancelling` về `running` — registry vẫn báo `running`
  đúng trong cửa sổ đó, nên nút cancel tự bật lại.
- Chuyển sang trạng thái kết thúc qua reconcile phải bật lại `read = false`, nếu
  không job xong lúc SSE chết sẽ không có badge — đúng ca mà reconcile sinh ra để lo.
- `clearErrorLog` xoá nhầm transfer thất bại (chúng cũng mang `error`).
- Gộp repeat bump `timestamp` nhưng không đổi vị trí → entry hiện giờ mới hơn entry
  nằm trên nó.
- `trim` phân hoạch running-trước-rest làm job nhảy lên đầu khi có push không liên quan.

Hai chỗ tài liệu tôi viết sai, đã sửa: `dataPoller` **không** phát toast lỗi (lý do
tôi viện dẫn cho cửa sổ gộp là bịa), và `unreadCount` là hàm thường chứ không phải
`$derived`.

## Risk Assessment

- **Đổi tên file chạm nhiều import** — cơ học, grep bắt hết; rủi ro thấp nhưng phải
  làm trọn trong một commit.
- **Ghi hai lần** cho lỗi đi qua cả `recordError` lẫn `globalToast` — chính là bước 6;
  cần test riêng vì đây là lỗi im lặng, không vỡ gì.
