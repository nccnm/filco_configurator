# Thiết kế trang Firmware (đề xuất, chờ duyệt)

Đây là phần "Tích hợp vào ứng dụng" của `DEFERRED_FIRMWARE_BUILD.md`: sửa cấu hình firmware ngay trong ứng dụng,
dựng và kiểm thử trên giả lập, rồi dẫn người dùng nạp bằng trình cập nhật chính hãng. Phần lõi (dựng, kiểm thử,
dựng trình cập nhật) đã xong ở các crate `filco-firmware`, `filco-fwtest`, `filco-updater`, `filco-fwbuild`.

Mockup có thể bấm: `docs/firmware-ui-mockup.html` (7 màn hình, sáng / tối). Nó dùng đúng token màu, phông và cấu
trúc phím của ứng dụng, cùng dữ liệu thật: bố cục M10SP, bảng Fn của V72, cấu hình bản `HJKL_CapsSpace2_Info` đang
trên bàn phím. CheckSum 0x2A7C trên mockup là số giả định.

## 1. Vào chế độ Firmware

- Thẻ **Firmware** nằm sau Layer 1-4, tách bằng một vạch đứng. Thẻ có màu nhấn riêng (xanh da trời, token mới
  `--c-fw`: `#0a85bd` sáng, `#4fc1f0` tối). Số trên thẻ là số thay đổi chưa dựng.
- Để màu xanh chỉ thuộc về chế độ Firmware, màu nhấn của Layer 2 (`--c-l2`) đổi từ xanh dương sang mận (`#a3367c`
  sáng, `#e48cc4` tối). Tránh đỏ, cam, vàng để không giống lỗi hay cảnh báo. Chấm trên thẻ layer giữ màu đèn LED
  thật lấy từ file model (Layer 2 vẫn xanh dương), vì nó cho biết đèn trên bàn phím sẽ sáng màu gì.
- Khi ở chế độ này:
  - một dòng dưới thanh tiêu đề cho biết firmware trên bàn phím (tên bản build và CheckSum, đọc từ khối thông tin)
    và nhắc rằng thay đổi chỉ có tác dụng sau khi dựng và nạp;
  - nút chính đổi từ "Ghi" thành **Dựng và kiểm thử**;
  - Hoàn tác, Làm lại và Bỏ mọi thay đổi áp dụng cho bản nháp firmware.
- Bản nháp bắt đầu từ cấu hình đọc được trong khối thông tin. Firmware gốc hoặc bản vá cũ không có khối thì bắt đầu
  từ V72 gốc, kèm một dòng cảnh báo cho trường hợp bản vá cũ. Bản nháp được lưu tự động, đóng ứng dụng không mất.

## 2. Bàn phím

- Mọi phím được đọc theo nghĩa "Fn + phím này":
  - Chữ to ở giữa là chức năng Fn. Màu mực nếu như V72 gốc, màu nhấn nếu đã đổi. Ví dụ phím H hiện "←" to.
  - Chữ nhỏ phía trên là tên phím (mã mặc định trong keymap).
- Phím không có chức năng Fn, hoặc đã bỏ chức năng Fn, để trống ở giữa, chỉ còn tên phím nhỏ phía trên.
- Ký hiệu một ký tự (mũi tên) vẽ to hơn chữ một chút cho dễ nhìn.
- Mã riêng của firmware và phím Menu vẽ bằng biểu tượng như sơ đồ lớp Fn trong manual (trang 10), không hiện mã số.
  Bảng chọn phím dùng cùng biểu tượng, tên đầy đủ ở tooltip và ở dòng tóm tắt của inspector:

  | Mã | Phím gốc | Biểu tượng | Tên đầy đủ |
  |---|---|---|---|
  | 0xF7 | W | chuột, nút trái tô đặc | Chuột: nhấp trái |
  | 0xF8 | R | chuột, nút phải tô đặc | Chuột: nhấp phải |
  | 0xFB | E | chuột + mũi tên lên | Chuột: di lên |
  | 0xFC | D | chuột + mũi tên xuống | Chuột: di xuống |
  | 0xF9 | S | mũi tên trái + chuột | Chuột: di sang trái |
  | 0xFA | F | chuột + mũi tên phải | Chuột: di sang phải |
  | 0xFD | X | chuột có bánh cuộn + cung lên | Chuột: cuộn lên |
  | 0xFE | C | chuột có bánh cuộn + cung xuống | Chuột: cuộn xuống |
  | 0xB5 | N | loa + dấu x | Tắt âm |
  | 0xBA | M | loa + một sóng | Giảm âm lượng |
  | 0xB9 | , | loa + hai sóng | Tăng âm lượng |
  | 0x65 | Win phải | khung có 3 gạch | Menu |

  Biểu tượng là SVG nét theo `currentColor` (màu mực, hoặc màu nhấn nếu đã đổi), vẽ trong khung 20 x 16. Tên chỉ
  dùng cho giao diện. `code_name` của `filco-firmware` vẫn in `0xNN` trong báo cáo, để báo cáo giống từng
  dòng bản Python. Tên chuột và âm lượng mới dựa vào manual, chưa thử trên máy thật; cần thử Fn + S (chuột sang
  trái) và Fn + N (tắt âm) trước khi làm. W, R, X, C chỉ có chức năng Fn trong bitmap 0xC634 (chế độ chuột, DIP ⑩
  OFF, mặc định); bitmap 0xC644 bỏ bốn phím này. Gán các mã này cho phím khác có chạy không thì phải kiểm trên giả
  lập trước khi cho chọn trong bảng phím.
- Phím tap-hold vẽ theo thiết kế đã duyệt (PLAN 12.3.1): hành động giữ ở trên, gõ nhanh ở giữa, "→ L3" khi giữ để
  vào layer.
- Phím có thay đổi nào so với V72 có viền màu nhấn. Chấm tròn đánh dấu thay đổi chưa dựng.

## 3. Inspector: ba mục cho phím đang chọn

| Mục | Nội dung | Ràng buộc hiện ngay trên giao diện |
|---|---|---|
| Tap-hold | Kiểu: Không / Giữ = phím / Giữ = layer; ô Gõ nhanh, ô Giữ (mã hoặc layer 1-4); Ngưỡng (thanh trượt + số) | Giữ = phím tối đa 1 phím, Giữ = layer tối đa 2 phím (hiện số đã dùng); ngưỡng 50-255 ms (giữ = phím) hoặc 50-2000 ms (giữ = layer); khóa kèm lý do nếu phím có chức năng Fn hoặc là phím Fn |
| Fn + phím | Mã cho Fn + phím, nút "Bỏ chức năng Fn" | Số ô Fn đã dùng / 39 |
| Mặc định | Mã mặc định của phím | Chế độ DIP có thể ghi đè (nhắc ở Alt / Win trái); đúng một phím phải là Fn |

- Bảng chọn phím dùng lại bảng của chế độ layer, nhưng chỉ có mã bàn phím (firmware lưu một mã HID, không có
  media / chuột / macro).
- Nút "Về như trên bàn phím" và "Về V72 gốc" cho phím đang chọn.

## 4. Dựng và kiểm thử (hộp thoại)

- Tên bản build gợi ý sẵn (tên bản trên bàn phím + hậu tố), sửa được (chữ, số, `_`, `-`, tối đa 32 ký tự).
- Khối thông tin luôn được thêm, vì ứng dụng cần nó để hiển thị và để kiểm sau khi nạp.
- Các bước hiện với số đếm thật lấy từ báo cáo: dựng ảnh, so từng phím, đổi layer, tap-hold, lệnh 0x30, stack,
  trình cập nhật. Khoảng 3-4 giây, chạy ở luồng nền, hủy được.
- **Đạt:**
  - CheckSum in to;
  - báo cáo đầy đủ mở được tại chỗ;
  - có nút Mở thư mục và **Nạp lên bàn phím...**.
- **Không đạt:** hiện các dòng FAIL. Không có trình cập nhật nên không có đường nạp.
- Kết quả lưu ở thư mục dữ liệu của ứng dụng (`firmware/<tên>/`), cùng bố cục với `out/<tên>/` của lệnh dòng lệnh.

## 5. Nạp (hộp thoại 4 bước)

1. **Sao lưu 4 layer** (tự làm, dùng kho sao lưu sẵn có).
2. **FILCO Assist đã đóng** (kiểm như luồng ghi sẵn có).
3. **Chạy trình cập nhật.**
   - Hiện CheckSum to kèm câu "phải thấy CheckSum : 0x.... rồi mới bấm Start".
   - Nút "Mở trình cập nhật" chỉ chạy khi người dùng bấm. Ứng dụng không bấm Start và không điều khiển cửa sổ đó.
   - Có thêm nút "Mở thư mục".
   - Ứng dụng chờ bàn phím ngắt kết nối và cắm lại.
4. **Kiểm sau khi nạp** (tự chạy khi bàn phím cắm lại, chỉ đọc).
   - Đọc khối thông tin: tên và nội dung phải khớp bản vừa dựng.
   - Đọc 4 layer và so với bản sao lưu.
   - Khác thì cảnh báo và đưa nút khôi phục (layer) hoặc gợi ý nạp lại (firmware).

Nút "Nạp lên bàn phím..." chỉ bật cho bản vừa đạt kiểm thử trong phiên này, và khi bản nháp chưa đổi sau lần dựng
đó (PLAN 12.5). Trên bàn phím giả lập (bản `e2e-sim`), bước 3-4 được mô phỏng để chạy E2E.

## 6. Menu "Thêm"

- Bản firmware đã dựng...: danh sách gồm tên, CheckSum, ngày, đạt / không và đánh dấu đã nạp. Thao tác cho mỗi
  bản: mở thư mục, nạp lại, nạp lại V72 gốc (đường lui), nạp cấu hình vào bản nháp.
- Nhập / Xuất cấu hình firmware (TOML, cùng định dạng `custom.toml` của repo phân tích).

## 7. Phần không đổi

Chế độ layer, luồng ghi layer, sao lưu, journal giữ nguyên. Chế độ Firmware không gửi gì xuống bàn phím ngoài các
lệnh đọc (0x10, 0x02, 0x30) và không bao giờ tự nạp.

## 8. Cần bạn chọn

1. ~~Màu nhấn của chế độ Firmware~~: đã chọn xanh da trời, Layer 2 đổi sang mận (mục 1).
2. ~~Nút "Mở trình cập nhật"~~: đã chọn để ứng dụng chạy trình cập nhật khi người dùng bấm; Start vẫn do người
   dùng bấm. Tự nạp ngay trong ứng dụng (dịch ngược giao thức của bootloader Sonix VID 0x0C45 PID 0x7040, không cần
   trình cập nhật) để làm sau.
3. ~~Fn + phím trên bàn phím~~: đã chọn hiện cho mọi phím, chức năng Fn là chữ chính, tên phím là chữ nhỏ (mục 2).
4. ~~Khối thông tin~~: đã chọn luôn thêm, không có nút tắt.

## 9. Kế hoạch làm sau khi duyệt

1. `filco-service`: bản nháp firmware (sửa, hoàn tác, lưu tự động, nhập / xuất TOML), chạy pipeline ở luồng nền
   có tiến độ, danh sách bản đã dựng, máy trạng thái của luồng nạp.
2. `src-tauri`: lệnh IPC và kiểu TypeScript sinh tự động; theo dõi cắm / rút cho bước 4.
3. UI: thẻ Firmware, cách vẽ phím, ba mục inspector, hai hộp thoại, mục trong menu Thêm; tiếng Việt và tiếng Anh.
   Đổi `--c-l2` trong `app.css` và chụp lại các ảnh chuẩn của Layer 2.
4. Kiểm thử:
   - unit (bản nháp, ràng buộc, máy trạng thái);
   - E2E trên bàn phím giả: sửa, dựng thật bằng pipeline, nạp mô phỏng, kiểm sau khi nạp;
   - ảnh chuẩn sáng / tối cho mọi màn hình mới;
   - `check.ps1` và `e2e.ps1 -Runs 3`.
5. Cập nhật STATUS, README và ảnh chụp.
