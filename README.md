# Keyboard Layer Editor

Trình chỉnh layer **không chính thức** cho bàn phím FILCO dùng FILCO Assist, kiểu VIA: chọn layer, bấm phím trên
hình, chọn hành động (phím, tổ hợp modifier, media, chuột, lệnh hệ thống), xem khác biệt, ghi xuống bàn phím. Viết bằng
Rust + Tauri 2 + Svelte 5, chạy trên Windows 10/11. Không liên quan và không được Diatec / FILCO bảo trợ.

*English summary: an unofficial, VIA-like layer editor for FILCO keyboards configured by FILCO Assist (currently the
Majestouch Xacro M10SP US). Every write shows a diff, needs a confirmation, is backed up first and read back byte for
byte. Windows only.*

Model hiện có: **Majestouch Xacro M10SP US** (VID 0x2EFD, PID 0x1972), mức `experimental`: đọc đã kiểm trên bàn phím
thật, ghi đã kiểm trên thiết bị giả và đang chờ kiểm trên bàn phím thật (xem `docs/STATUS.md`).

## Cài đặt

1. Tải `Keyboard Layer Editor_<phiên bản>_x64-setup.exe` và file `.sha256` đi kèm.
2. Kiểm mã băm trong PowerShell, hai chuỗi phải giống nhau:

   ```powershell
   (Get-FileHash '.\Keyboard Layer Editor_0.1.0_x64-setup.exe' -Algorithm SHA256).Hash.ToLower()
   Get-Content '.\Keyboard Layer Editor_0.1.0_x64-setup.exe.sha256'
   ```

3. Chạy bộ cài. Bộ cài **chưa ký số**, nên Windows SmartScreen có thể báo "Windows protected your PC": bấm *More info*
   rồi *Run anyway* (chỉ làm vậy khi mã băm đã khớp). Bộ cài cài cho người dùng hiện tại, không cần quyền admin, vào
   `%LOCALAPPDATA%\Keyboard Layer Editor`; gỡ trong *Settings > Apps*.

Cần WebView2 Runtime (có sẵn trên Windows 11; bộ cài tự tải nếu thiếu).

## Dùng

1. **Thoát FILCO Assist** (chuột phải biểu tượng ở khay hệ thống, *Exit*). FILCO Assist dùng cùng cổng cấu hình và có
   thể ghi đè layer; khi nó chạy, ứng dụng không đọc và không ghi.
2. Mở ứng dụng: nó tự tìm bàn phím và đọc 4 layer (khoảng 0,75 giây). Đọc không thay đổi gì trên bàn phím.
3. Chọn layer (tab hoặc phím 1-4), bấm một phím trên hình (hoặc đi bằng mũi tên), chọn hành động ở bảng dưới. "Áp cho"
   gán cùng hành động cho nhiều layer một lần. Delete đưa ô về mặc định. Ctrl+Z / Ctrl+Y hoàn tác / làm lại.
4. Bấm **Lưu N thay đổi** (Ctrl+S): ứng dụng đọc lại bàn phím, so với lúc bắt đầu sửa, sao lưu, rồi hiện bảng khác
   biệt từng ô (layer, phím, đang có, sẽ ghi, byte). Chỉ khi bấm **Ghi xuống bàn phím** mới ghi; nút Hủy có sẵn focus.
5. Mỗi layer ghi xong được đọc lại và so đủ 512 byte. Lỗi giữa chừng thì ứng dụng đưa mọi layer đã ghi về bản sao lưu
   và báo từng layer đang ở trạng thái nào.

Phím trên hình: viền màu layer là ô đã gán (khác ô trống), chấm màu ở góc là thay đổi chưa ghi, chữ mờ là ô trống
(phím làm theo keymap mặc định của firmware), viền nét đứt là ô loại ứng dụng chưa sửa được (macro...), giữ nguyên.

### Sao lưu, khôi phục, xuất / nhập

- Trước mỗi lần ghi có một bản sao lưu tự động trong `%LOCALAPPDATA%\FilcoConfigurator\backups\<model>\<thời gian>\`,
  cùng định dạng thư mục với `fnmap.py` (`layer0.bin`...`layer3.bin` + `manifest.json` có SHA-256 từng layer).
- Menu **Thêm**: sao lưu ngay, khôi phục từ bản sao lưu (nạp vào trình sửa; ghi vẫn qua bảng khác biệt), mở thư mục
  sao lưu, mở một thư mục sao lưu khác (ví dụ của `fnmap.py`), nhập / xuất file `.filco-layers.json`, chép cả layer
  sang layer khác, giao diện sáng / tối, tiếng Việt / tiếng Anh.

### An toàn

- Ứng dụng không tự nạp firmware; không gửi lệnh nào ngoài đọc trang (0x10), đọc khối thông tin firmware (0x30), ghi
  trang (0x11) và kết thúc (0x02). Tầng truyền chặn ở mức code mọi gói ghi khi không ở trong luồng ghi đã xác nhận.
  Nạp firmware chỉ qua trình cập nhật chính hãng, mở khi người dùng bấm, và người dùng tự bấm Start.
- Nếu ứng dụng hay máy tắt giữa lúc ghi, lần mở sau nó đọc bàn phím, so với bản sao lưu và với nội dung định ghi, rồi
  đề nghị khôi phục.
- Layer gắn với profile của FILCO Assist (đọc `%APPDATA%\FILCO\filco_assist_config.json`, chỉ đọc) được cảnh báo:
  lưu profile đó trong FILCO Assist sẽ dựng lại cả layer và xóa thay đổi làm ở đây.
- Khi đang giữ Fn, firmware bỏ qua ô layer; lớp Fn nằm cố định trong firmware, ứng dụng này không đổi được.

### Tap-hold và cấu hình trong firmware

Nếu bàn phím đang chạy một bản firmware tùy biến có khối thông tin (lệnh HID 0x30), ứng dụng đọc và **hiển thị** các
thay đổi đó: phím tap-hold vẽ hành động khi giữ (nhỏ, phía trên) và khi gõ nhanh (nhãn chính), phím "giữ để vào
layer" được tô đậm trên layer đó, và inspector hiện chi tiết tap-hold cùng các thay đổi Fn + phím. Ô layer của phím
tap-hold bị khóa vì firmware quyết định. Firmware gốc (hay bản vá cũ không có khối) trả NACK và ứng dụng chỉ không
hiện phần này.

### Trang Firmware: sửa, dựng, kiểm thử, nạp

Tab **Firmware** (sau Layer 4) sửa phần nằm trong firmware: Fn + phím, keymap mặc định và tap-hold. Thiết kế:
`docs/FIRMWARE_UI_DESIGN.md`.

1. Bàn phím trên hình vẽ Fn + phím: chức năng Fn là nhãn lớn, tên phím nhỏ ở trên, phím không có chức năng Fn để trống.
   Nhấp chuột, di chuyển chuột, cuộn và âm lượng vẽ bằng icon như manual. Chọn một phím rồi sửa ở bảng dưới
   (Tap-hold, Fn + phím, Mặc định). Mỗi thay đổi được vá thử ngay trên firmware V72 gốc; thay đổi firmware không cho
   phép (hết 39 mục Fn, phím Fn, W/R/X/C phụ thuộc bố cục...) bị từ chối kèm lý do.
2. Bản nháp tự lưu (`%LOCALAPPDATA%\FilcoConfigurator\firmware_draft.toml`); Ctrl+Z / Ctrl+Y, xuất / nhập TOML ở menu
   **Thêm**. Bản nháp bắt đầu từ cấu hình đang chạy trên bàn phím (đọc từ khối thông tin), hoặc V72 gốc nếu không có.
3. **Dựng và kiểm thử**: đặt tên rồi dựng (3-4 giây). Firmware được chạy trên giả lập (so từng phím với bản gốc, Fn,
   đổi layer, tap-hold, lệnh 0x30, độ sâu stack); trình cập nhật chỉ được tạo khi mọi kiểm thử đạt. Kết quả ở
   `%LOCALAPPDATA%\FilcoConfigurator\firmware\<tên>\`. Khối thông tin luôn được thêm.
4. **Nạp lên bàn phím...**: ứng dụng sao lưu 4 layer, kiểm FILCO Assist đã thoát, rồi mở trình cập nhật chính hãng.
   Người dùng kiểm dòng `CheckSum` đúng số ứng dụng hiện, rồi tự bấm Start. Bàn phím khởi động lại; ứng dụng đọc khối
   thông tin, xác nhận đúng bản vừa dựng và báo layer nào bị đổi so với trước khi nạp.

Dựng không cần giao diện (offline, không mở bàn phím, không chạy trình cập nhật):

```powershell
cargo run --release -p filco-cli -- firmware-build <config.toml> [--stock firmware] [--out out]
```

Cấu hình là file TOML cùng định dạng `custom.toml` của repo phân tích (ví dụ trong `tests/fixtures/firmware/*/config.toml`).
Kết quả ở `out/<name>/`; trình cập nhật chỉ được dựng khi mọi kiểm thử trên giả lập đạt, và người dùng tự chạy nó,
kiểm dòng `CheckSum` đúng tag in ra rồi mới bấm Start. Chi tiết: `docs/DEFERRED_FIRMWARE_BUILD.md`.

Bộ cài kèm sẵn firmware V72 gốc và trình cập nhật chính hãng (thư mục `firmware/`, có license, xem `firmware/README.md`)
để nạp được trên máy khác mà không cần gì thêm.

## Thêm một model

Mô tả model là dữ liệu, không phải code: một file JSON trong `models/`, kiểm bằng `models/schema/model.schema.json`
(sinh từ kiểu Rust) và các kiểm tra ngữ nghĩa trong `filco-core`.

- `match`: VID, danh sách PID, caps HID của cổng cấu hình (FILCO Assist lọc theo usage page 1, input 9, output 2,
  feature 65).
- `protocol`: biến thể giao thức (`costar-v1`), độ dài gói, cách đặt ô trong trang (`costar-interleaved-8`), khoảng
  chờ đọc / ghi. Model mới để 100 ms cho cả hai tới khi đo được trên máy thật.
- `layers`: nhãn và màu đèn LED từng layer.
- `slot_types`: loại ô sửa được (0-4) và chỉ hiển thị.
- `layout.keys`: từng phím với `id`, chỉ số ma trận `index`, tọa độ `x`/`y` và kích thước `w`/`h` theo đơn vị phím,
  chữ in `legend`, `default_usage` (mã HID chuẩn của chữ in), `tags` (`m-key`, `unverified`), `note`.
- `support`: `read_only` (chỉ đọc), `experimental` (cho ghi kèm cảnh báo) hoặc `verified`.

Kiểm một file: `cargo run -p filco-cli -- validate-model models\my-model.json`. Thêm file vào danh sách
`builtin::models()` trong `crates/filco-core/src/model.rs` rồi chạy `pwsh scripts/check.ps1`. Model M10SP US được sinh
từ `tools/gen_m10sp_model.py` để hàng phím dễ đọc; viết JSON tay cũng được.

## Build từ mã nguồn

### Máy có Visual Studio (cách thường)

Cần: Visual Studio 2022 Build Tools với workload *Desktop development with C++* (MSVC + Windows SDK), Rust qua rustup
(phiên bản ghim trong `rust-toolchain.toml`), Node.js 22 trở lên.

```powershell
npm ci
npx tauri build            # bộ cài: target\release\bundle\nsis\
pwsh scripts/check.ps1     # format, lint, kiểu, toàn bộ test
```

Hoặc `pwsh scripts/release.ps1 [-InstallTest]`: build bộ cài, kiểm kích thước, ghi file `.sha256`, thử cài / gỡ im
lặng.

### Máy không có quyền admin và không có Visual Studio (cách repo này được build)

Không cần thay gì trong repo; chỉ cấu hình máy, trong `%USERPROFILE%\.cargo\config.toml` (xem `docs/DECISIONS.md` D1):

1. Rust: chạy `rustup-init.exe` (host `x86_64-pc-windows-msvc`).
2. CRT + Windows SDK của Microsoft bằng [`xwin`](https://github.com/Jake-Shadle/xwin):
   `xwin --accept-license splat --output %LOCALAPPDATA%\xwin-msvc\splat --disable-symlinks
   --preserve-ms-arch-notation` (chấp nhận giấy phép của Microsoft). `cargo-xwin` cũng làm được việc này trên máy cho
   phép tạo symlink (Developer Mode bật); trên máy không cho, dùng thẳng `xwin` như trên.
3. LLVM bản portable (cần `clang-cl`, `llvm-rc`, `llvm-lib`) cho build script của Tauri.
4. `config.toml`:

   ```toml
   [env]
   CC_x86_64_pc_windows_msvc = "C:\\...\\llvm\\bin\\clang-cl.exe"
   CXX_x86_64_pc_windows_msvc = "C:\\...\\llvm\\bin\\clang-cl.exe"
   AR_x86_64_pc_windows_msvc = "C:\\...\\llvm\\bin\\llvm-lib.exe"
   RC = "C:\\...\\llvm\\bin\\llvm-rc.exe"
   INCLUDE = "<splat>\\crt\\include;<splat>\\sdk\\include\\ucrt;<splat>\\sdk\\include\\um;<splat>\\sdk\\include\\shared"
   LIB = "<splat>\\crt\\lib\\x86_64;<splat>\\sdk\\lib\\um\\x86_64;<splat>\\sdk\\lib\\ucrt\\x86_64"

   [target.x86_64-pc-windows-msvc]
   linker = "C:\\Users\\<you>\\.rustup\\toolchains\\<toolchain>\\lib\\rustlib\\x86_64-pc-windows-msvc\\bin\\rust-lld.exe"
   rustflags = ["-Clinker-flavor=lld-link", "-Clink-arg=/ignore:4099",
                "-Lnative=<splat>\\crt\\lib\\x86_64", "-Lnative=<splat>\\sdk\\lib\\um\\x86_64",
                "-Lnative=<splat>\\sdk\\lib\\ucrt\\x86_64"]
   ```

Khi cài Build Tools bình thường, xóa các mục này là xong.

## Kiểm thử

| Lệnh | Nội dung |
|---|---|
| `pwsh scripts/check.ps1` | quy tắc văn bản, `cargo fmt`, `clippy -D warnings` (cả bản `e2e-sim`), mọi test Rust (codec vét cạn 2^32 ô ở chế độ release), `cargo deny` (giấy phép, nguồn), prettier, eslint, svelte-check, vitest |
| `pwsh scripts/e2e.ps1 [-Runs 3] [-Perf] [-Screenshots]` | build bản `e2e-sim` (bàn phím giả sau IPC thật) rồi chạy test E2E qua WebDriver, so ảnh chuẩn, đo hiệu năng, chụp mọi màn hình |
| `cargo bench -p filco-service` / `-p filco-hid-windows` | chi phí đọc 4 layer, liệt kê HID |
| `pwsh scripts/release.ps1 -InstallTest` | bộ cài, ngân sách kích thước, SHA-256, cài / gỡ |

`check.ps1` cần `cargo-deny` (`cargo install cargo-deny --locked`). E2E cần `tauri-driver` (`cargo install tauri-driver --locked`) và `msedgedriver` cùng phiên bản chính với WebView2
Runtime của máy (đặt đường dẫn vào biến `MSEDGEDRIVER`, hoặc giải nén vào `%LOCALAPPDATA%\kle-tools\edgedriver-<bản>`).

## Cấu trúc

| Thư mục | Nội dung |
|---|---|
| `crates/filco-core` | ô, trang, diff, mô tả model + schema, sao lưu kiểu `fnmap.py`, file xuất |
| `crates/filco-protocol` | gói feature report, checksum, phiên đọc / ghi trang, chặn ghi (`WritePolicy`) |
| `crates/filco-sim` | bàn phím giả theo giao thức, tiêm lỗi, khối thông tin cho 0x30 |
| `crates/filco-fwinfo` | giải mã khối thông tin firmware (lệnh 0x30), chỉ đọc |
| `crates/filco-hid-windows` | HID thật trên Windows (chỗ duy nhất có `unsafe`) |
| `crates/filco-service` | phiên sửa + undo, luồng ghi an toàn, kho sao lưu, journal, chặn FILCO Assist |
| `crates/filco-cli` | công cụ dòng lệnh cho dev: chỉ đọc bàn phím (`list`, `caps`, `read`, `decode`, `validate-model`); `firmware-build` dựng + kiểm firmware offline |
| `crates/thumb` | assembler và decoder Thumb ARMv6-M (Cortex-M0) cho code chèn vào firmware |
| `crates/filco-firmware` | dựng ảnh V72 tùy biến từ bản gốc: HEX, bảng Fn / keymap, code tap-hold, khối thông tin + lệnh 0x30 |
| `crates/m0emu` | trình thông dịch Cortex-M0 thuần Rust, dừng ở lệnh chưa mô hình hóa |
| `crates/filco-fwtest` | chạy firmware thật trong `m0emu`, so với bản gốc từng phím, tap-hold, 0x30 qua HID giả, độ sâu stack |
| `crates/filco-updater` | dựng trình cập nhật từ bản chính hãng (ZIP trong tài nguyên RT_RCDATA 4011) |
| `crates/filco-fwbuild` | luồng dựng: ảnh, HEX, kiểm thử, và chỉ khi mọi kiểm thử đạt mới dựng trình cập nhật |
| `src-tauri` | ứng dụng: trạng thái, lệnh IPC mỏng, kiểu IPC sinh sang TypeScript |
| `ui` | giao diện Svelte |
| `models` | mô tả model JSON và schema |
| `firmware` | firmware V72 gốc + trình cập nhật chính hãng (có license), kèm trong bộ cài |
| `tests` | fixtures (trang layer của chính người dùng, ghi nguồn, firmware golden), golden bytes từ `fnmap.py`, E2E |
| `docs` | bản giao việc, kế hoạch, quyết định, trạng thái, ảnh chụp |

## Giấy phép

Chưa chọn (người dùng quyết định trước khi chia sẻ). Font IBM Plex đi kèm theo giấy phép SIL Open Font License 1.1.

Repo **có chứa** firmware V72 gốc và trình cập nhật chính hãng của Diatec/FILCO trong `firmware/` (người dùng có
license; xem `firmware/README.md`), kèm trong bộ cài để nạp. Vì vậy repo này là private và không được chuyển sang
public khi còn các file đó. Ngoài firmware và trình cập nhật đó, phần còn lại không chứa code dịch ngược hay file nhị
phân nào của Diatec; giao thức và bố cục được mô tả lại ở dạng dữ liệu và tài liệu tự viết.
