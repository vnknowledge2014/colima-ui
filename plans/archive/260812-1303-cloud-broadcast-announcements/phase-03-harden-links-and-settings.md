---
phase: 3
title: "Harden links and settings"
status: completed
priority: P3
dependencies: [2]
effort: "S"
---

# Phase 3: Harden links and settings

## Overview

Đóng finding Critical về `linkUrl`, ép nội dung là plain text, và thêm công tắc tắt
thật sự tắt.

## `linkUrl` → `openExternal` (finding #5, Critical)

`src/lib/external-links.ts:110-122`:

```ts
export async function openExternal(url: string): Promise<void> {
  if (isRunningInTauri()) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);      // ← không kiểm scheme, không kiểm host
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
```

`opener:default` đã được cấp (`capabilities/default.json`). Mọi URL đi qua hàm này
**hiện tại** đều là hằng số biên dịch sẵn (repo, pricing, Polar portal) — nên chưa
có lỗ hổng. Announcement là **giá trị từ mạng đầu tiên** đi vào đường này.

Kịch bản hỏng: feed bị chiếm → đẩy alert `critical` với `linkUrl` là một URI tuỳ ý;
người dùng click; URI được giao cho OS handler, hoàn toàn ngoài CSP của webview.

**Sửa — hai tầng, không phải một:**

1. **Allowlist tại điểm dùng.** Trước khi render link trong panel:
   - scheme phải là `https:` (parse bằng `new URL()`, không regex)
   - host phải nằm trong allowlist ngắn (`github.com`, `polar.sh`, domain vendor)
   - không hợp lệ → **không render nút link**, chứ không phải render rồi chặn lúc click
2. **Chốt chặn trong `openExternal`.** Thêm guard scheme ngay trong hàm đó, để đường
   này an toàn cho **mọi caller tương lai** chứ không chỉ caller hôm nay. Đây là sửa
   phòng thủ có lợi cho cả app, không riêng feature này — và là lý do nó đáng làm kể
   cả nếu phần cloud bị bỏ.

## Nội dung phải là plain text (Medium)

Repo đã có mẫu `{@html} renderMarkdownHTML` (`AiChatPanel.svelte:490`). Rất dễ có
người sau này áp mẫu đó cho `body` của announcement — lúc đó nội dung từ mạng thành
HTML thực thi trong webview.

**Sửa:** render `title`/`body` bằng text binding thường (`{value}`), **không**
`{@html}`, và ghi lý do ngay trong comment ở component để lần sửa sau không vô tình
đổi. Kèm giới hạn độ dài phía client.

## Công tắc

- Cờ trong `src/lib/settingsStore.svelte.ts`, mặc định **bật**.
- Khi tắt: **không gọi mạng chút nào** — kiểm cờ trước khi tạo request, không phải
  lọc kết quả sau.
- Đặt vào `NotificationSettings.svelte` đã có (Phase 6 của plan trước), thành mục
  thứ hai bên cạnh công tắc notification của OS. Không tạo section mới cho một
  checkbox.

## Telemetry phải nói thật

Kể cả khi không gửi gì lên, fetch một file đã lộ **IP + thời điểm + phiên bản app**
cho GitHub. `docs/telemetry.md` phải mô tả kênh này như một kết nối ra ngoài, không
được im lặng chỉ vì "chỉ là đọc".

## Related Code Files

- Modify: `src/lib/external-links.ts` — guard scheme trong `openExternal`
- Modify: `src/components/notifications/NotificationItem.svelte` — allowlist + text binding
- Modify: `src/lib/announcements.ts` — giới hạn độ dài
- Modify: `src/lib/settingsStore.svelte.ts` — cờ
- Modify: `src/components/settings/NotificationSettings.svelte` — công tắc thứ hai
- Modify: `docs/telemetry.md`

## Implementation Steps

1. Guard scheme trong `openExternal`; chạy test hiện có để chắc caller cũ không vỡ.
2. Allowlist host tại `NotificationItem`; URL không hợp lệ → không render link.
3. Text binding cho `title`/`body` + comment nêu lý do; giới hạn độ dài.
4. Cờ settings; kiểm **trước** khi gọi mạng.
5. `docs/telemetry.md`.
6. Test: `javascript:` và `file://` bị chặn; host lạ không render link; `body` chứa
   thẻ HTML hiện ra như text; tắt cờ → 0 request.

## Success Criteria

- [x] `linkUrl` scheme không phải `https:` → không render link.
- [x] Host ngoài allowlist → không render link.
- [x] `openExternal` từ chối scheme lạ **kể cả khi gọi từ chỗ khác**.
- [x] `body` chứa thẻ HTML → hiện ra như text, không thực thi.
- [x] Tắt công tắc → 0 request tới host feed.
- [x] `docs/telemetry.md` mô tả đúng chiều dữ liệu và những gì lộ ra khi fetch.

## Kết quả

**Công tắc đã làm ở Phase 2** (cơ chế), phase này chỉ thêm UI vào
`NotificationSettings.svelte` — mục thứ hai, không tạo section mới. Toggle nằm
ngoài nhánh `inApp` vì backend fetch ở cả browser mode.

**`settingsStore.svelte.ts` lại không phải sửa**: key/value chung là đủ.

**Doc từng nói sai một dòng.** Bản đầu của `docs/telemetry.md` liệt kê "phiên bản
app" trong danh sách thứ lộ ra khi fetch — nhưng `announcements.rs` dựng
`reqwest::Client` không đặt `.user_agent()`, và reqwest **không** tự gửi UA. Hai
hướng sửa: thêm UA cho doc thành đúng, hoặc sửa doc. Chọn sửa doc — thêm UA là
tự tăng thứ lộ ra để một câu văn khỏi phải sửa. Chuỗi i18n ở cả 4 ngôn ngữ cũng
sửa theo. Plan gốc (`plan.md`, "IP + thời điểm + phiên bản app") viết trước khi
có code; thực tế chỉ IP + thời điểm.

**Hai tầng chứ không một**, đúng như đặc tả: `openExternal` chặn mọi scheme không
phải `https:` cho **mọi** caller (hôm nay đều là hằng số, nên guard không đụng
gì — nó ở đó cho caller sau), và allowlist host chặn riêng link từ feed. Link
không hợp lệ thì **không vẽ nút**, chứ không vẽ rồi chặn lúc click.

Sau review thêm: từ chối cả `port` và userinfo trong allowlist —
`https://user:pw@github.com/` trao credential cho trình duyệt, `:1337` nói rằng
link không phải trang web thường; announcement thật không cần cái nào.

Đã thử 35 vector bypass (userinfo `github.com@evil.test`, suffix
`github.com.evil.test`, dấu chấm cuối, homograph Cyrillic, punycode,
`javascript://github.com/%0a…`) — không cái nào lọt.

## Risk Assessment

- **Allowlist quá chặt** chặn nhầm link hợp lệ của chính vendor → giữ allowlist ở
  một chỗ, dễ bổ sung; và announcement vẫn hiện nội dung dù link bị chặn.
- **Sửa `openExternal` ảnh hưởng caller có sẵn** — tất cả đều là URL `https://` hằng
  số, nên guard không đụng gì; vẫn phải chạy test hiện có để chắc.
