# Task hoãn: dựng ảnh firmware tùy biến trong ứng dụng

## Trạng thái (2026-11-01): phần A-D xong, chưa tích hợp vào giao diện

Nhánh `feat/firmware-build`. Bốn phần A-D đã port sang Rust và đạt tiêu chí so với golden. Việc dựng firmware giờ
chạy hoàn toàn trong Rust, không cần Python, Keystone hay Unicorn:

| Phần | Crate | Kết quả so với golden |
|---|---|---|
| A | `filco-firmware` (HEX, bảng V72, cấu hình TOML, nhật ký vá) | 5/5 bản: `image.bin`, `.hex` (tên và nội dung, tức tag), `PATCH_DIFF.txt` giống từng byte |
| B | `thumb` (assembler + decoder ARMv6-M) + `filco-firmware::taphold` / `info` | Code tap-hold và code lệnh 0x30 ra đúng từng byte như Keystone; 4 bản có code đạt như trên |
| C | `m0emu` (trình thông dịch Cortex-M0) + `filco-fwtest` (kịch bản, máy HID giả, độ sâu stack) | 5/5 bản: `TEST_REPORT.txt` giống **từng dòng** báo cáo Unicorn; mỗi bản dưới 4 giây (bản debug có tối ưu cho trình giả lập), 3,4 giây cả pipeline ở release |
| D | `filco-updater` (ZIP trong RT_RCDATA 4011) | Xem ghi chú dưới |
| Pipeline | `filco-fwbuild` (`build.py` `main()`) + `filco-cli firmware-build` | Cả `TEST_REPORT.txt` (gồm các dòng của trình cập nhật) giống golden, trừ các dòng in đường dẫn máy |

Ghi chú phần D: trình cập nhật mà `isp_exe.py` dựng nén lại **mọi** mục của ZIP bằng deflate trong bản Python đang
dùng (Python 3.14 kèm zlib-ng). Byte nén vì thế phụ thuộc thư viện nén, không phụ thuộc dữ liệu. Bản Rust giữ nguyên
byte nén gốc của các mục không đổi (chép thẳng từ trình cập nhật chính hãng), và chỉ nén hai mục thay đổi (HEX và
`UISettings.ini`, deflate mức 6 qua zlib-rs). Hai mục này ra giống từng byte bản golden. Test chứng minh:

- bộ ghi ZIP giống hệt `zipfile` của Python: đọc ZIP golden rồi ghi lại ra đúng từng byte;
- ZIP dựng ra khớp golden ở từng mục: tên (kể cả cách `zipfile` đổi CP437 sang UTF-8 và `\` sang `/`), phương thức
  nén, ngày giờ, CRC, kích thước, dữ liệu sau giải nén;
- byte nén của HEX và `UISettings.ini` giống golden, còn byte nén của các mục khác giống bản chính hãng;
- các tài nguyên RCDATA khác (4007, 4008, 4009, 4012, 4013) giống golden.

Vì vậy file exe khác golden ở kích thước (1 933 824 so với 1 935 360 byte) nhưng chứa đúng cùng nội dung.

Kiểm thử âm tính: sửa một byte bảng Fn sau khi dựng, hoặc đặt sai kỳ vọng tap-hold, thì kiểm thử báo FAIL
(`crates/filco-fwtest/tests/broken.rs`). Lỗi cấu hình dừng trước khi ghi file nào.

Lệnh (offline, không mở bàn phím, không chạy trình cập nhật):

```powershell
cargo run --release -p filco-cli -- firmware-build <config.toml> [--stock firmware] [--out out]
```

Kết quả ở `out/<name>/`: HEX (tên mang tag), `image.bin`, `PATCH_DIFF.txt`, `TEST_REPORT.txt`, và
`M10SP_72US_V72_<name>.exe` chỉ khi mọi kiểm thử đạt.

Còn lại: mục "Tích hợp vào ứng dụng" ở cuối tài liệu (phiên dựng firmware trong `filco-service`, lệnh IPC, trang
"Firmware" trong UI, luồng an toàn khi nạp). Phần giao diện cần người dùng duyệt thiết kế trước.

---

Phần dưới là đặc tả gốc của task, giữ nguyên.

Tài liệu này mô tả một task **chưa làm**. Nó tách khỏi giai đoạn 2 phần "đọc + hiển thị" (PLAN.md mục 12.1-12.8).
Khi nào làm task này thì đọc kỹ cả tài liệu này lẫn các nguồn tham khảo được trỏ tới trước khi viết dòng code đầu tiên.

## Bối cảnh và lý do hoãn

Phần "đọc + hiển thị" cho phép ứng dụng đọc cấu hình của firmware đang nạp (qua lệnh HID 0x30) và vẽ tap-hold / Fn +
phím / keymap lên bàn phím. Nó không cần dựng hay sửa firmware.

Task này là bước còn lại: cho ứng dụng **tự dựng ảnh V72 tùy biến** rồi dựng trình cập nhật, thay cho `rea_filco`
(Python). Hiện `rea_filco` vẫn làm việc này; ứng dụng chỉ đọc kết quả. Task được hoãn để phần đọc + hiển thị (đúng
yêu cầu ban đầu của người dùng) hoàn thành trước và không bị chặn bởi phần phức tạp hơn.

Khi bắt đầu task, cân nhắc chia nhỏ và trao đổi trước với người dùng về cách triển khai, vì đây là phần tạo ra ảnh
nhị phân sẽ nạp lên phần cứng thật.

## Nguồn tham khảo (đã có, không cần dựng lại)

Tất cả logic cần port đã tồn tại và đã được kiểm chứng trên thiết bị thật trong repo `rea_filco`
(`D:\AI\rea_filco`, repo private, firmware có license). Dùng chúng làm đặc tả và làm nguồn golden:

| Nguồn | Vai trò |
|---|---|
| `firmware/FN_ANALYSIS.md` | Phân tích firmware V72: địa chỉ bảng, bố cục, các điểm nối, ngân sách stack. Mục 7-8 (bảng dữ liệu), 10-12 (vùng trống, state, stack), 14 (khối thông tin và lệnh 0x30). Đây là đặc tả quyền uy; không suy lại từ ảnh |
| `firmware/flash_tool/hexfile.py` | Đọc/ghi Intel HEX giữ đúng bố cục bản ghi, tổng word, tag CheckSum |
| `firmware/flash_tool/v72.py` | Mô tả ảnh gốc, bảng mã phím, các thao tác vá **bảng dữ liệu** (Fn + phím, keymap, vị trí Fn) có kiểm byte gốc |
| `firmware/flash_tool/taphold.py` | Vá **thêm code** tap-hold (giữ = phím, giữ = layer) |
| `firmware/flash_tool/fwinfo.py` | Khối thông tin và vá **thêm code** trả lời lệnh 0x30 (phần `parse()` đã port sang `filco-fwinfo` ở giai đoạn đọc) |
| `firmware/flash_tool/emu.py` | Giả lập firmware thật (Unicorn) + máy tính HID giả để kiểm |
| `firmware/flash_tool/build.py` | Luồng dựng và toàn bộ kịch bản kiểm thử; sinh `TEST_REPORT.txt` |
| `firmware/flash_tool/isp_exe.py` | Dựng trình cập nhật: thay HEX trong tài nguyên của trình cập nhật chính hãng |
| `firmware/tools/stackdepth.py` | Độ sâu stack tĩnh worst-case từng điểm vào |

Golden đã commit sẵn trong repo này tại `tests/fixtures/firmware/` (5 bản build: `HJKL`, `HJKL_CapsTapHold`,
`HJKL_CapsSpace`, `HJKL_CapsSpace2`, `HJKL_CapsSpace2_Info`), mỗi bản gồm `config.toml`, `image.bin`, `.hex`,
`PATCH_DIFF.txt`, `TEST_REPORT.txt`. Firmware gốc và trình cập nhật ở `firmware/`.

## Phạm vi và các phần việc

Chia làm bốn phần. Mỗi phần có tiêu chí "giống từng byte" rõ ràng so với golden, nên đúng/sai kiểm được tự động.

### A. Đọc/ghi HEX và mô tả ảnh V72 - `crates/filco-firmware`

- Ảnh 64 KB; đọc Intel HEX thành ảnh (byte không có bản ghi = 0xFF), ghi lại giữ đúng bố cục bản ghi, xuống dòng và
  tập địa chỉ được phủ như file gốc (port `hexfile.py`: `read_image`, `write_like`).
- Tổng word và tag CheckSum (port `word_sum`, `TAG_OFFSET`).
- Mô tả ảnh gốc: địa chỉ bảng Fn/keymap/bitmap, bảng mã phím, kiểm SHA-256 ảnh gốc, kiểm byte gốc trước mỗi lần sửa
  (port phần dữ liệu và kiểm cấu trúc của `v72.py`).
- Thao tác vá **bảng dữ liệu** (không thêm code): Fn + phím, keymap mặc định, vị trí Fn; ghi nhật ký từng byte
  (port các hàm sửa bảng của `v72.py`).
- `FirmwareConfig` (serde, đọc TOML như `build.py`) và mã hóa khối thông tin (port `fwinfo.block`; `parse` đã có ở
  `filco-fwinfo`).
- Kiểm chứng: dựng lại bản `HJKL` (chỉ sửa bảng dữ liệu) phải ra `image.bin`, `.hex`, tag và `PATCH_DIFF.txt` giống
  từng byte golden (bỏ qua các dòng đường dẫn Config/Stock).

### B. Sinh code chèn vào firmware - crate riêng (ví dụ `crates/thumb` + phần vá code của `filco-firmware`)

Đây là phần phức tạp nhất và nhạy nhất: sinh mã máy Cortex-M0 cho logic tap-hold và cho phần trả lời lệnh 0x30, rồi
đặt vào vùng flash trống của ảnh và nối vào các điểm đã phân tích.

- Logic, địa chỉ, điểm nối, bố cục state và ngân sách stack: **không thiết kế lại**, lấy nguyên từ `taphold.py`,
  `fwinfo.py` và `FN_ANALYSIS.md` (mục 10-12, 14). Chúng đã chạy đúng trên thiết bị thật.
- Cần một bộ sinh mã Thumb (ARMv6-M) tương đương Keystone để ra byte giống hệt. Lưu ý các mẫu mã hóa đã khảo sát
  (ví dụ `mov` dạng thanh ghi cao, `adds` 3 toán hạng khi vừa, chèn literal pool, đệm `.align` bằng 0).
- Kiểm chứng: 4 bản build golden còn lại (có tap-hold và/hoặc khối thông tin) phải ra `image.bin`, `.hex`, tag và
  `PATCH_DIFF.txt` giống từng byte. Đây là ràng buộc rất chặt: sai một byte mã lệnh là lệch tag và lệch toàn bộ.

### C. Giả lập và kiểm thử - `crates/m0emu` + `crates/filco-fwtest`

- `m0emu`: trình thông dịch Cortex-M0 (ARMv6-M Thumb) thuần Rust thay Unicorn; bộ nhớ flash/RAM/ngoại vi, hook theo
  địa chỉ, khởi động từ vector reset, mô hình ma trận phím, gọi ISR timer mỗi bước, máy tính HID giả (như `HidHost`
  trong `emu.py`). Mỗi bước = 1 tick 1 ms.
- `filco-fwtest`: port toàn bộ kịch bản của `build.py` (so từng phím thường và kèm Fn với bản gốc, Fn + Alt + số,
  tap-hold giữ = phím, giữ = layer với ~13 kịch bản, hai phím giữ = layer, khối thông tin qua HID, độ sâu stack tĩnh
  - port `stackdepth.py`).
- Kiểm chứng: chạy trên 5 ảnh golden phải cho kết quả **từng dòng** giống `TEST_REPORT.txt` (các báo cáo này do
  Unicorn sinh). Thêm test lệnh lẻ cho từng nhóm lệnh ARMv6-M (cờ, nhân, dịch, nạp/lưu, rẽ nhánh), đối chiếu ARM ARM.
- Trình giả lập phải dừng ngay ở lệnh chưa hỗ trợ thay vì đoán.

### D. Dựng trình cập nhật - `crates/filco-updater`

- Chép trình cập nhật chính hãng, thay kho ZIP ở tài nguyên RT_RCDATA 4011 (HEX mới và chuỗi tên) qua API tài nguyên
  Windows, đọc lại để kiểm (port `isp_exe.py`).
- Kiểm chứng: với cùng HEX, trình cập nhật dựng ra giống từng byte bản mà `isp_exe.py` đã dựng (các bản đã nạp thành
  công nằm ở `rea_filco/firmware/flash_tool/flashed/`). Có thể cần thêm một fixture golden của trình cập nhật đã dựng.

## Tích hợp vào ứng dụng (sau khi A-D xong)

- `filco-service` thêm phiên dựng firmware: đọc/sửa `FirmwareConfig`, dựng ảnh, chạy `filco-fwtest`, chỉ khi mọi
  kiểm thử đạt mới dựng trình cập nhật; theo dõi lần nạp.
- `src-tauri` thêm lệnh IPC; UI thêm trang "Firmware" cho sửa tap-hold / Fn + phím / keymap.
- An toàn khi nạp (PLAN.md 12.5): chỉ nạp ảnh vừa qua toàn bộ kiểm thử trong cùng phiên; hiện CheckSum để người dùng
  đối chiếu trong trình cập nhật; **không tự bấm Start, không tự động hóa cửa sổ trình cập nhật**; sao lưu 4 layer
  trước, đọc lại và so sau khi cắm lại; có đường lui nạp lại bản gần nhất hoặc V72 gốc.

## Ngân sách (khi task này nhập vào bộ cài)

Bộ cài đến 8 MB (trình cập nhật ~1,9 MB, HEX ~0,2 MB); một lần build + kiểm thử firmware dưới 15 giây; bộ nhớ trình
giả lập chỉ tốn khi build, không tính vào lúc rảnh.
