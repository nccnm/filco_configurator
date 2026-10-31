# Quyết định trong lúc làm

Ghi lại các quyết định không có sẵn trong `docs/PLAN.md`, theo thứ tự thời gian. Người dùng cần xem lại những mục có
ghi "người dùng chọn".

## 2026-10-31

### D1. Toolchain không cần admin (M0)

- Phiên làm việc không có quyền admin, máy chưa có Visual Studio / Build Tools. `winget install
  Microsoft.VisualStudio.BuildTools --scope user` và `Rustlang.Rustup --scope user` đều trả "No applicable
  installer found".
- Rust 1.99.1 cài bằng `rustup-init.exe` tải từ `static.rust-lang.org` (SHA-256 khớp file `.sha256` chính thức),
  host `x86_64-pc-windows-msvc`, profile minimal + clippy + rustfmt.
- `cargo-xwin` 0.23.1 hỏng trên máy này: bước splat SDK tạo symlink, cần quyền admin hoặc Developer Mode (os error
  1314). Dùng thẳng `xwin` 0.10.0 (SHA-256 khớp bản phát hành) với `--accept-license --disable-symlinks
  --preserve-ms-arch-notation`: mọi file CRT + Windows SDK 10.0.26100 đã được giải nén vào
  `%LOCALAPPDATA%\xwin-msvc\splat`; chỉ bước symlink cuối cùng lỗi và không cần. Việc chấp nhận giấy phép của
  Microsoft cho CRT/SDK là chấp nhận thay người dùng theo plan đã duyệt (mục 1.1, đường 4).
- Linker: `rust-lld` có sẵn trong toolchain, flavor `lld-link`, cộng `/ignore:4099` (CRT của xwin không kèm PDB).
- Một build script của Tauri (`vswhom-sys`, qua `embed-resource`) cần trình biên dịch C++; dùng LLVM 23.1.4 bản
  portable (chỉ giải nén `clang-cl`, `lld-link`, `llvm-rc`, `llvm-lib`, `llvm-ar` và header của clang) vào
  `%LOCALAPPDATA%\llvm-23`. File nén `.tar.zst` dùng cửa sổ zstd lớn mà `tar` của Windows từ chối, nên giải nén bằng
  module `compression.zstd` của Python 3.14.
- Toàn bộ cấu hình này nằm ở `%USERPROFILE%\.cargo\config.toml` của máy, **không nằm trong repo**: trên máy có Visual
  Studio Build Tools bình thường, repo build như mọi dự án Tauri (README mục Build). Khi người dùng cài Build Tools,
  xóa file config đó là xong.
- Kết quả: app Tauri trống build được cả debug và release, bộ cài NSIS 1,06 MiB, cửa sổ mở được.

### D2. TypeScript 6.0.3 thay vì 7.0.2

`svelte-check` 4.7.7 khai báo peer `typescript ^5 || ^6`. Plan đã dự liệu; ghim 6.0.3.

### D3. Mọi component Svelte dùng `<script lang="ts">`

Trên máy này `svelte-check` báo "Could not find a declaration file for module './App.svelte'" khi file `.ts` import
một component không có khối `<script lang="ts">` (tái hiện được trong dự án tối giản, mọi phiên bản 4.6-4.7.7, cả TS
5.8). Component có `lang="ts"` thì được kiểm kiểu đầy đủ. Quy ước: mọi component có `<script lang="ts">`.

### D4. Cảnh báo `npm audit` mức thấp

`deepmerge` (qua `@sveltejs/vite-plugin-svelte`) có cảnh báo prototype poisoning mức thấp, chưa có bản sửa. Chỉ dùng
lúc build, không vào ứng dụng. Chấp nhận, theo dõi khi có bản mới.

### D5. `docs/BRIEF.md` được miễn kiểm em dash

Bản giao việc phải chép nguyên văn, và chính nó trích ký tự em dash trong quy tắc cấm. `scripts/check-text.ps1` bỏ qua
riêng file này.

### D6. Tên sản phẩm (người dùng chọn)

Tên làm việc "Keyboard Layer Editor", định danh `io.github.keyboard-layer-editor`. Đổi tên: `productName` và
`title` trong `src-tauri/tauri.conf.json`, hằng số `PRODUCT_NAME` trong UI.

### D7. Mã hóa ô loại 3 và 4 (M1)

Disassemble trình xử lý trong ảnh V60 (chỉ đọc file, `firmware/tools/armdis.py`): loại 3 (0x8f92) chép b1 vào byte
thấp và b2 vào byte cao của báo cáo Consumer (0x2000009c/9d), nên mã 16 bit là little-endian ở b1-b2; loại 4
(0x8fcc) chép đúng một byte b1 vào báo cáo System Control (0x2000009b). Vì vậy ô loại 4 là `04 mã 00 00`. Mã nào
firmware chấp nhận (0x81 tắt máy, 0x82 ngủ, 0x83 thức theo HID, hay chỉ số 1..3) phụ thuộc report descriptor của
interface System Control; sẽ đọc descriptor đó (chỉ đọc, không gửi report) ở M3. Tới khi đó UI chỉ đưa 3 mã HID chuẩn
và ghi chú "chưa kiểm trên thiết bị".

### D8. Model M10SP US ở mức `experimental`

Đọc và ghi bằng `fnmap.py` đã chạy trên máy thật, nhưng ghi bằng ứng dụng này chưa. Model để `experimental` (cho ghi,
kèm cảnh báo) cho tới khi người dùng làm xong checklist sáng mai; khi đó đổi `support` thành `verified` trong
`tools/gen_m10sp_model.py` rồi sinh lại `models/m10sp-us.json`. Alt phải để chỉ số 78, gắn `unverified`.

### D9. File model do script sinh

`models/m10sp-us.json` được sinh từ `tools/gen_m10sp_model.py` (hàng phím, độ rộng, chỉ số ma trận viết tay ở một
chỗ, tính tọa độ x tự động). File JSON vẫn là thứ ứng dụng đọc và là dạng để người khác thêm model; prettier bỏ qua file
này để giữ đúng định dạng của script.

### D10. Ô loại 4 là mặt nạ bit (M3, đo trên máy thật)

`filco-cli caps` đọc report descriptor của collection System Control (MI_01, col02; chỉ đọc descriptor, không gửi
report): một byte input gồm các nút Variable cho usage 0x81..=0x83 (logical 0..1). Firmware chép b1 của ô loại 4
nguyên vẹn vào byte đó (D7), nên b1 là mặt nạ bit: 0x01 tắt máy, 0x02 ngủ, 0x04 thức, không phải mã usage 0x81-0x83.
Bảng `usage::SYSTEM` dùng mặt nạ này. Ô loại 4 chưa được gõ thử trên máy thật: có trong checklist sáng mai (chỉ thử
"ngủ" nếu người dùng muốn).

### D11. Khoảng chờ đọc 15 ms cho M10SP US (M3, đo trên máy thật)

Phiên đo 2026-11-01 bằng `filco-cli measure-delays` (chỉ 0x10 + 0x02, chặn ReadOnly, FILCO Assist đã đóng), 36 phiên
đọc trang trong giới hạn 50:

| Khoảng chờ | Lần 1 | Lần 2 | Kết quả |
|---|---|---|---|
| 100 ms (tham chiếu) | 4,828 s | | khớp SHA-256 `configs/2026-10-31_Layer4Snip` |
| 50 ms | 2,427 s | 2,427 s | giống từng byte bản 100 ms |
| 25 ms | 1,225 s | 1,228 s | giống |
| 15 ms | 0,748 s | 0,747 s | giống |
| 10 ms | 0,507 s | 0,507 s | giống |

Không có lỗi checksum, NoAck hay khác byte ở mức nào. Theo plan (mục 4.5), mức nhỏ nhất qua hết là 10 ms nên dùng mức
lớn hơn kế tiếp, 15 ms, làm khoảng chờ đọc của riêng model M10SP US (`protocol.delay_ms.read`). Khoảng chờ ghi giữ
100 ms. Model khác mặc định 100 ms. Đọc 4 layer ~0,75 s, trong ngân sách 1,0 s.

## 2026-11-01

### D12. Kiểu IPC sinh từ một tầng DTO riêng (M5)

`ts-rs` chỉ gắn vào các kiểu trong `src-tauri/src/view.rs` (ảnh chụp trạng thái, hành động, danh mục), không gắn vào
kiểu lõi của `filco-core`: lõi không phụ thuộc thư viện sinh TypeScript, và giao diện nhận đúng thứ nó hiển thị (nhãn
đã dịch, cờ "đã sửa"). Tất cả sinh vào một file `ui/src/bindings/ipc.ts`; test `bindings_are_current` báo lỗi khi file
cũ hơn mã Rust (`UPDATE_BINDINGS=1` để sinh lại).

### D13. WebView2 chạy không tăng tốc GPU và không có renderer dự phòng (M6)

Đo sau 500 thao tác: tiến trình GPU của WebView2 chiếm 110 MB private, tổng cả cây 372 MB, vượt ngân sách 220 MB.
`additionalBrowserArgs` thêm `--disable-gpu` và tắt `SpareRendererForSitePerProcess` (giữ các cờ mặc định của Tauri):
tiến trình GPU còn 22 MB, tổng 207 MB. Giao diện là SVG nét mảnh và vài bảng; vẽ bằng CPU vẫn dưới 3 ms mỗi lần đổi
layer. Nếu sau này cần hoạt ảnh nặng, xem lại.

### D14. Đo khởi động không qua WebDriver; E2E dùng WebDriver Classic (M6)

Qua `tauri-driver` + `msedgedriver`, thời điểm "spawn" gồm cả việc driver tạo phiên, làm số khởi động phình ra (496 ms).
Bản `e2e-sim` có lệnh `e2e_startup`: khi biến `KLE_E2E_STARTUP_FILE` được đặt, giao diện báo thời điểm trang bắt đầu và
các mốc của nó, Rust ghi thêm lúc `main` chạy, rồi thoát. `tests/e2e/perf.mjs` chạy thẳng file exe 10 lần và lấy trung
vị. WebDriver BiDi mở thêm một trang "mapper" (thêm một renderer) nên E2E ép dùng WebDriver Classic; số bộ nhớ khi
đó khớp với lúc chạy app trực tiếp (một renderer).

### D15. Bàn phím giả nằm trong `filco-service` (M5)

`SimBackend` (bàn phím giả sau khóa dùng chung, điều khiển rút / cắm, tiêm lỗi, giả FILCO Assist) là một `Backend` như
HID thật, nên bản `e2e-sim` chạy đúng mã ứng dụng, IPC và giao diện của bản phát hành, chỉ khác nguồn thiết bị. Bản phát
hành không bật tính năng `e2e-sim`: lệnh điều khiển bàn phím giả không tồn tại trong đó.

### D16. Nhãn tổ hợp phím dài trên keycap (M5)

Ảnh chụp cho thấy "Ctrl+Shift+Alt+E" thu xuống cỡ chữ 5 px, không đọc được. Keycap vẽ hai dòng: dòng nhỏ là modifier
(viết tắt "C+S+A" khi tên đầy đủ dài quá 9 ký tự), dòng lớn là phím. Inspector và bảng khác biệt luôn ghi tên đầy đủ.

### D17. "data-state=starting" cho tới khi tìm bàn phím xong lần đầu (M6)

Trước lần tìm đầu tiên, giao diện từng báo sẵn trạng thái "chưa có bàn phím" và hiện màn hình đó trong khoảng 300 ms
rồi mới chuyển sang bàn phím: vừa nháy sai nội dung, vừa làm test đo đọc 4 layer thỉnh thoảng bắt đầu quá sớm (2 trên 4
lần thử). Nay vùng chính để trống và `data-state` là `starting` tới khi lần tìm đầu xong.

### D18. Bàn phím mất kết nối không còn hiện là "chưa hỗ trợ" (M6, test E2E tìm ra)

Khi bàn phím ngừng trả lời giữa lúc ghi, ứng dụng bỏ kết nối nhưng danh sách thiết bị còn cũ, nên hiện "Chưa hỗ trợ
bàn phím này" và mất hộp thoại báo kết quả ghi. Nay ứng dụng quét lại thiết bị ngay khi mất kết nối, chỉ báo "chưa hỗ
trợ" khi thật sự không có model nào khớp, và giữ mô tả model để hộp thoại kết quả vẫn ghi đúng tên layer.

### D19. Giấy phép phụ thuộc: cho phép thêm MPL-2.0 (M8)

`cargo deny check` (`deny.toml`): mọi crate là MIT, Apache-2.0, BSD-3-Clause, Zlib, Unicode-3.0, CC0-1.0, trừ vài
crate MPL-2.0 mà Tauri kéo vào (`cssparser`, `selectors`...). MPL-2.0 là copyleft theo từng file: dùng nguyên bản từ
crates.io thì chia sẻ ứng dụng dưới giấy phép nào cũng được. Không có advisory nào. Phía npm: chỉ cảnh báo mức thấp ở D4,
chỉ dùng lúc build.
