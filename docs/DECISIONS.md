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
