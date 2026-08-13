---
phase: 3
title: "Cắm ghi nhận vào các hành động"
status: completed
priority: P2
dependencies: [1, 2]
effort: "1-1.5 ngày"
---

# Phase 3: Cắm ghi nhận vào các hành động

## Overview

Gọi `record()` từ những chỗ thật sự làm gì đó. Đây là phase dễ làm ẩu nhất: cắm
30 chỗ theo kiểu chép-dán, quên vài chỗ, và không ai biết là đã quên.

## Chốt thiết kế: ghi ở lệnh, không ghi ở chokepoint

Phase 1 dựng `runtime::run` và có vẻ hiển nhiên là nên ghi ở đó — một chỗ, không
thể quên. **Không làm vậy**, vì ba lý do:

1. `runtime::run` chỉ thấy argv. `docker rm -f abc123` không cho biết container
   tên gì, hay lệnh này là người dùng bấm hay `self_heal` tự chạy.
2. Nó chạy cho **cả lệnh đọc**. `docker ps` chạy mỗi vài giây — lọc ra bằng danh
   sách verb là một danh sách nữa để quên cập nhật, đặt ở chỗ không ai nghĩ tới.
3. Colima/Lima không đi qua nó. Vẫn phải cắm tay ở `adapters/` — nên "một chỗ
   duy nhất" là ảo tưởng ngay từ đầu.

Nên: cắm ở tầng lệnh, nơi có ngữ nghĩa. Phase 1 vẫn cần — nó là điều kiện để
`duration_ms` đo được ở một chỗ thay vì ba mươi chỗ.

**Chống quên** bằng test, không bằng kỷ luật: một test liệt kê các lệnh mutating
đã biết và khẳng định mỗi lệnh có một `record` tương ứng. Xem phần Tests.

## Requirements

- Ghi cả bốn nhóm: `destructive`, `lifecycle`, `task`, `config`.
- Ghi **cả thất bại**, kèm lý do đã redact.
- `actor: app` cho hành động do self-heal / watcher khởi xướng.
- Không lệnh nào đổi hành vi hay chữ ký công khai.

## Architecture

```
mỗi lệnh mutating:
    let started = Instant::now();
    let result = <việc thật>;
    activity::record(ActivityEntry {
        kind, verb, target_kind, target, target_name,
        actor: Actor::User,
        outcome: (&result).into(),      // Ok → ok, Err → failed
        detail: <câu ngắn>,
        duration_ms: (chỉ nhóm task),
    });
    result
```

### Nhóm và nơi cắm

| Nhóm | Lệnh | Tệp |
|---|---|---|
| `destructive` | `remove_container`, `remove_image`, `remove_volume`, `remove_network`, `prune_images`, `prune_volumes`, `prune_networks`, `system_prune`, `delete_instance` | `containers.rs`, `volumes.rs`, `networks.rs`, `colima.rs` |
| `lifecycle` | `start/stop/restart/pause/unpause_container`, `run_container`, `rename_container`, `start/stop_instance` | `containers.rs`, `colima.rs` |
| `task` | `pull_image`, `start_image_save/load`, `copy_to/from_container`, `security_scan_image`, `export_sbom` | `containers.rs`, `file_transfer.rs`, `security_scan.rs` |
| `config` | `apply_colima_config`, `alerts_save/delete_rule`, `security_policy_save/delete`, `heal` rules, `security_watch_set_enabled` | `colima_config.rs`, `alerts.rs`, `security_policy.rs`, `self_heal.rs`, `security_watch.rs` |

### `prune` phải ghi con số

`docker image prune` trả về danh sách đã xoá và dung lượng thu hồi. Một bản ghi
"prune ok" không có số là vô dụng — chính con số mới là thứ người ta quay lại tìm.
Trích từ stdout, đưa vào `detail`.

### Tác vụ nền ghi khi kết thúc, không ghi lúc bắt đầu

`start_image_save` trả job id ngay rồi chạy nền. Bản ghi phát sinh khi job xong,
với `outcome` thật và `duration_ms` thật — một bản ghi "đã bắt đầu" không bao giờ
có phần kết là rác.

### `actor: app` — chốt 2026-08-13: có ghi

<!-- Updated: Validation Session 1 - Unresolved #2 closed: ghi cả hành động do app khởi xướng -->

`self_heal` restart container, `security_watch` rescan image. Cả hai đi qua đúng
những lệnh trên. Truyền `actor` xuống, hoặc dùng biến task-local. **Không** đoán
theo thread name.

Lưu ý trùng lặp: `self_heal` đã ghi `heal_log` và `security_watch` đã ghi
`scan_runs`. Ghi thêm vào `activity_log` là **cố ý** — hai câu hỏi khác nhau
("luật self-heal chạy ra sao" vs "cái gì đã xảy ra với máy tôi"). Phase 4 khử
trùng khi hiển thị.

## Related Code Files

- Modify: `commands/containers.rs`, `volumes.rs`, `networks.rs`, `compose.rs`
- Modify: `commands/colima.rs`, `colima_config.rs`, `file_transfer.rs`
- Modify: `commands/security_scan.rs`, `alerts.rs`, `security_policy.rs`,
  `security_watch.rs`, `self_heal.rs`
- Create: `src-tauri/tests/` hoặc test trong `activity.rs` cho phần chống quên

## Implementation Steps

1. Cắm nhóm `destructive` trước — giá trị cao nhất, ít lệnh nhất. Ship được ngay
   cả khi ba nhóm sau chưa xong.
2. Test chống quên (xem dưới), chạy đỏ trước khi cắm tiếp.
3. Nhóm `lifecycle`.
4. Nhóm `task` — cẩn thận điểm ghi của tác vụ nền.
5. Nhóm `config`.
6. `actor: app` cho self-heal và watcher.

## Tests

- **Chống quên** (chốt 2026-08-13: giữ cách quét mã nguồn): danh sách hằng các lệnh mutating; test khẳng định mỗi lệnh có
  một `record` tương ứng. Cách rẻ: quét mã nguồn tại thời điểm test bằng
  `include_str!` từng tệp và tìm `activity::record` trong thân từng hàm đã liệt
  kê. Thô, nhưng bắt được đúng lỗi cần bắt và không cần chạy Docker.
- `remove_container` với id không tồn tại → có bản ghi `outcome = failed`.
- `prune_images` → `detail` chứa số lượng và dung lượng.
- `run_container` với `-e SECRET=x` → không có `x` trong `activity.db`.
- self-heal restart → bản ghi `actor = app`; người dùng restart → `actor = user`.
- Tác vụ nền hỏng giữa chừng → đúng một bản ghi, `outcome = failed`.

## Success Criteria

- [ ] Cả bốn nhóm có bản ghi, kể cả khi thất bại.
- [ ] Test chống quên xanh và **đỏ** khi cố tình xoá một `record`.
- [ ] `actor` phân biệt đúng người dùng và app.
- [ ] Không lệnh nào đổi chữ ký công khai; `pnpm check` không có lỗi mới.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Quên một lệnh, không ai biết | Test chống quên; danh sách lệnh là dữ liệu, không phải trí nhớ |
| Test quét mã nguồn vỡ khi đổi tên hàm/định dạng | Chấp nhận có ý thức. Nó vỡ **ồn ào** lúc CI, không im lặng như việc quên — đó là đánh đổi đúng |
| Chép-dán sai `kind`/`verb` giữa 30 chỗ | Cắm theo nhóm, review từng nhóm một, không cắm cả 30 rồi mới xem |
| Ghi hai lần (lệnh + đường nền) cho cùng một việc | Điểm ghi của tác vụ nền là lúc **kết thúc**; có test |
| Trùng với `heal_log`/`scan_runs` gây rối | Cố ý; phase 4 khử trùng khi hiển thị |
| Phase phình vì ai cũng muốn thêm một lệnh nữa | Danh sách trong bảng trên là phạm vi đóng. Lệnh mới là plan sau |


## Đã dựng — 2026-08-13

29 lệnh có ghi nhận, chia 4 nhóm đúng như bảng trên. Phần lớn do một phiên song
song làm; phiên này làm 4 tác vụ nền trong `file_transfer.rs`, dựng test chống
quên, và sửa hai lỗi mô tả dưới đây.

### Ghi ở `spawn_job`, không ghi ở 4 lệnh `start_*`

Cả bốn transfer đi qua `spawn_job`. Đó là chỗ **duy nhất** thấy được kết cục
thật và đo được thời lượng: `start_image_save` trả job id sau vài mili giây,
trước khi có outcome hay duration nào để ghi. Một dòng "đã bắt đầu" không bao
giờ có phần kết là rác, nên bản ghi phát sinh ở bốn nhánh kết thúc của
`spawn_job` chứ không ở thân các lệnh.

`TransferSubject` mang tên đối tượng từ lúc `start_*` xuống tới lúc kết thúc —
image có thể đã bị xoá khỏi máy khi job xong, và dòng log là chỗ duy nhất tên nó
còn sống.

### Thêm `ActivityOutcome::Cancelled`

Huỷ giữa chừng không vừa với từ nào đang có. `Failed` đổ lỗi cho máy về một lựa
chọn của người dùng; `Denied` nghĩa là "chưa từng chạy", trong khi một bản export
bị huỷ **đã chạy**, đã tốn thời gian, và có thể để lại việc dở dang. Transfer là
thứ đầu tiên trong app có thể dừng giữa chừng, nên từ vựng thật sự thiếu một từ.

### Hai lỗi đã sửa

1. **4 chỗ `prune` xoá mất lý do thất bại.** Viết
   `.outcome_of(&result).detail(<summary>)`, mà `summary` rỗng khi lệnh hỏng —
   nên nó ghi đè lên thông điệp lỗi `outcome_of` vừa đặt. Dòng log nói "failed"
   mà không nói vì sao, đúng thứ duy nhất một dòng thất bại tồn tại để nói.
   Sửa bằng cách **đảo thứ tự** (`outcome_of` chỉ điền `detail` khi rỗng), kèm 2
   test hồi quy.
2. **`human_bytes` bị nhân bản.** Tôi thêm một bản trong lúc viết `spawn_job`,
   không thấy bản đã có ở `file_transfer.rs:317`. Đã xoá bản thừa.

### Test chống quên — có chứng minh nó đỏ được

`commands/activity_coverage.rs`: danh sách hằng 29 lệnh + `include_str!` mã
nguồn, khẳng định mỗi lệnh có `activity::record` trong thân (hoặc gọi
`spawn_job`, mà `spawn_job` được kiểm riêng là có ghi).

Đã kiểm chứng bằng cách xoá trọn một câu lệnh `record` khỏi `remove_volume`:
build **vẫn OK**, test **đỏ** với thông điệp nêu đúng tên lệnh, xanh lại sau khi
khôi phục. Đó là bằng chứng nó bắt được lỗi thật, không chỉ xanh cho có.

Cách này thô — đổi tên `activity::record` hay xuống dòng khác đi là nó vỡ. Đánh
đổi có ý thức: nó vỡ **ồn ào** lúc CI, còn lỗi nó canh thì im lặng hàng tháng
cho tới khi ai đó đi tìm lệnh prune đã xoá image của họ và không thấy gì.

### Cổng chất lượng

`cargo clippy -D warnings` sạch · `cargo test --lib` **456 passed** ·
`pnpm lint/typecheck/check` 0 lỗi.
