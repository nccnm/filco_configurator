# Giao việc: ứng dụng desktop chỉnh layer bàn phím FILCO (kiểu VIA), Rust + Tauri

Bạn là session "planner/implementer". Một session khác (session "reviewer", ở pane bên cạnh trong cùng tab herdr) giao
việc này và sẽ review plan của bạn. Người dùng đã đi ngủ và trao quyền cho hai session làm tới khi có phần mềm build
xong; sáng mai người dùng tự kiểm tra. Trả lời và viết tài liệu bằng tiếng Việt (code, tên biến, commit message theo
quy ước của repo mới; mặc định tiếng Anh cho code và commit).

## Mục tiêu

Một ứng dụng desktop Windows (Rust + Tauri, bản ổn định mới nhất của cả Rust và Tauri: tự tra phiên bản hiện tại trên
web, đừng dựa vào trí nhớ) để xem và sửa cấu hình layer của bàn phím FILCO dùng giao thức của FILCO Assist, giống
usevia.app về cấu trúc: chọn layer, bấm vào phím trên hình bàn phím, chọn hành động (phím cơ bản, modifier, media,
chuột, macro...), xem khác biệt, lưu. Người dùng muốn chia sẻ phần mềm này cho người khác dùng.

## Bối cảnh và nguồn kiến thức (đọc trước khi lập plan)

Repo phân tích: `D:\AI\rea_filco` (private, có phần mềm/firmware/manual bản quyền Diatec). Đọc:

- `D:\AI\rea_filco\CLAUDE.md` và `README.md`: toàn cảnh, nguyên tắc an toàn với bàn phím thật.
- `D:\AI\rea_filco\docs\FAQ.md`: đặc biệt câu 6 (lưu trữ, ghi layer), 7 (lệnh HID), 8 (giới hạn ô layer), 9 (loại ô
  8-15 kiểu QMK, hướng mở rộng tương lai), 11 (trình chỉnh sửa giống VIA: kiến trúc, style, bố cục, lý do không dùng
  WebHID).
- `D:\AI\rea_filco\firmware\tools\fnmap.py`: bản cài đặt giao thức đã chạy đúng trên thiết bị thật (tìm interface,
  0x10 đọc, 0x11 ghi 8 gói, 0x02 kết thúc + checksum, định dạng sao lưu). Đây là nguồn chuẩn để đối chiếu từng byte.
- `D:\AI\rea_filco\firmware\FN_ANALYSIS.md` mục 2-3 (định dạng ô, loại ô), `docs\FilcoAssistUI_CODE_MAP.md` (giao thức
  phía FILCO Assist: 0x15 bảng macro, 0x16/0x17 Game Mode, 0x05/0x06 basic info; danh sách model mà FILCO Assist hỗ
  trợ), `decomp/` nếu cần chi tiết.
- `D:\AI\rea_filco\docs\gen_layers.py` và artifact https://claude.ai/artifact/Sx5Z6PDKyHbvAuumKHotiv ("M10SP layer
  setup"): style giao diện phải theo (giấy/mực, chế độ tối, màu nhấn theo layer trùng màu đèn, IBM Plex, bàn phím vẽ
  nét). Hình bố cục phím M10SP có trong gen_layers.py.
- Dữ liệu thật để làm test fixture: `D:\AI\rea_filco\firmware\backups\` và `firmware\configs\` (trang layer 512 byte đọc
  từ thiết bị thật, kèm manifest).

## Yêu cầu bắt buộc

1. **Kiến trúc mở cho nhiều bàn phím FILCO.** Hiện chỉ có M10SP (VID 0x2EFD, PID 0x1972), nhưng thiết kế phải thêm được
   các model FILCO khác dùng FILCO Assist mà không sửa lõi: mô tả model (VID/PID, bố cục hình học, ánh xạ vị trí ma
   trận -> chỉ số ô, số layer, loại ô hỗ trợ, biến thể giao thức nếu có) là dữ liệu, có schema và kiểm tra hợp lệ.
2. **Testing kỹ trước khi đưa người dùng test thật.** Plan phải có chiến lược test nhiều tầng, ví dụ: unit test mã hóa
   và giải mã ô, gói HID và checksum với golden bytes đối chiếu `fnmap.py`; thiết bị giả (mock HID) cho mọi luồng đọc,
   ghi, lỗi giữa chừng, rút cáp, checksum sai; property test round-trip; test với các trang thật trong backups; test giao
   diện tự động (tauri-driver/WebDriver hoặc tương đương trên Windows); test kiểm định schema của mô tả model. Cân nhắc
   cả việc dùng giả lập firmware thật (`firmware/flash_tool/emu.py`, Unicorn) để chạy chính hàm xử lý lệnh HID của
   firmware làm thiết bị giả, nếu khả thi; nếu không, nói rõ vì sao.
3. **Performance là yêu cầu quan trọng.** Đặt ngân sách đo được (thời gian khởi động, thời gian đọc đủ 4 layer, phản hồi
   UI khi bấm phím, bộ nhớ, kích thước bộ cài) và có test/benchmark kiểm chúng. Firmware vốn đã nhanh; ứng dụng không
   được là nút thắt.
4. **An toàn với bàn phím thật** (luật của repo, bắt buộc):
   - Mọi lần ghi phải qua bước xem khác biệt và xác nhận của người dùng trong UI; tự sao lưu trước khi ghi, đọc lại so
     từng byte sau khi ghi; cảnh báo nếu FILCO Assist đang chạy (cùng giao thức, nó tự ghi profile).
   - **Trong đêm nay, session của bạn KHÔNG được ghi xuống bàn phím thật** (không 0x11, không 0x02, không 0x15...).
     Được phép các lệnh chỉ đọc trên thiết bị thật (tìm thiết bị, 0x10 đọc trang) để kiểm thử tích hợp, sau khi đóng
     FILCO Assist nếu nó đang chạy. Mọi test ghi chạy trên thiết bị giả. Viết sẵn một checklist test thủ công có ghi
     để người dùng làm sáng mai.
   - Không chạy trình cập nhật firmware; ứng dụng này không nạp firmware.
5. **Repo riêng, chia sẻ được.** Tạo repo git mới ở `D:\AI\filco_configurator` (hoặc tên tốt hơn nếu bạn có lý do; báo
   lại). Không chép vào đó firmware, bộ cài, manual, code decompile hay file nhị phân bản quyền Diatec; chỉ mô tả giao
   thức và bố cục ở dạng dữ liệu/tài liệu do mình viết. Trang layer thật dùng làm fixture: chỉ dùng nếu đó là cấu hình
   của người dùng (không phải dữ liệu Diatec), ghi rõ nguồn. Làm việc trên nhánh tính năng; được commit trên nhánh đó;
   không push, không tạo remote, không merge vào main.
6. Phạm vi bản đầu: xem 4 layer từ thiết bị, sửa ô loại 0-4 (trống, chuột, gõ phím + modifier, media, system), sao
   lưu/khôi phục, xuất/nhập file, khác biệt trước khi ghi, ghi + đọc lại. Macro (loại 6, lệnh 0x15) và loại ô 8-15 là
   giai đoạn sau, nhưng kiến trúc phải chừa chỗ. Plan nên nói rõ cái gì vào bản đầu.
7. Chất lượng: lint sạch (clippy với cảnh báo là lỗi, formatter), test không flaky, giao diện chăm chút từng pixel theo
   style đã nêu. Không dùng ký tự em dash "—" trong mọi văn bản, chỉ dùng "-".

## Quy trình

1. Đọc nguồn ở trên, tra phiên bản Rust/Tauri mới nhất, rồi viết plan vào
   `D:\AI\filco_configurator\docs\PLAN.md` (tạo repo nếu chưa có). Plan gồm: kiến trúc (crate/module, ranh giới Rust và
   frontend, chọn framework frontend và lý do), mô hình dữ liệu model bàn phím, giao thức HID trên Windows (crate nào,
   cách mở interface MI_00 đúng như fnmap.py), luồng ghi an toàn, UI, chiến lược test chi tiết, ngân sách performance,
   các mốc (milestone) có tiêu chí xong đo được, rủi ro và câu hỏi mở.
2. **Chưa implement khi plan chưa được duyệt.** Viết xong plan thì dừng và trả lời đúng một dòng:
   `PLAN READY: D:\AI\filco_configurator\docs\PLAN.md`. Session reviewer sẽ gửi nhận xét; sửa plan và trả lời lại cùng
   dòng đó. Lặp tới khi nhận được tin nhắn bắt đầu bằng `PLAN APPROVED`.
3. Khi nhận `PLAN APPROVED`: trước khi code, giữ `docs/PLAN.md` là bản đã duyệt, sinh thêm `docs/plan.html` (một trang
   HTML độc lập cùng style giao diện ở trên, nội dung đúng như PLAN.md) để session reviewer đăng thành artifact cho người
   dùng đọc. Trả lời một dòng `PLAN HTML: <đường dẫn>` rồi bắt đầu implement luôn, không chờ.
4. Implement: toàn bộ source Rust/Tauri nằm trong folder riêng `D:\AI\filco_configurator`, không đặt gì vào
   `D:\AI\rea_filco`. Implement theo các mốc, chạy test ở mỗi mốc, commit theo mốc. Không hỏi người dùng (họ đang ngủ);
   gặp quyết định thì chọn phương án tốt nhất theo plan và ghi vào `docs/DECISIONS.md`. Gặp chặn thật sự thì ghi vào
   `docs/STATUS.md` và báo reviewer.
5. Khi xong: build bộ cài Windows, chạy toàn bộ test và benchmark, viết `docs/STATUS.md` (đã làm gì, test nào chạy và
   kết quả, benchmark so với ngân sách, việc còn lại, checklist test thủ công cho sáng mai, đường dẫn bộ cài), rồi trả
   lời đúng một dòng: `DONE: D:\AI\filco_configurator\docs\STATUS.md`.
