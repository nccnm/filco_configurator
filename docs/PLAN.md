# Plan: Keyboard Layer Editor (repo `filco_configurator`), trình chỉnh layer bàn phím FILCO kiểu VIA (Rust + Tauri, Windows)

Trạng thái: bản nháp chờ duyệt. Ngày 2026-10-31. Bản giao việc gốc: `docs/BRIEF.md`.

Nguồn đã đọc: `D:\AI\rea_filco\` (`CLAUDE.md`, `README.md`, `SETUP.md`, `docs/FAQ.md` câu 6-11,
`firmware/tools/fnmap.py`, `firmware/FN_ANALYSIS.md` mục 1-5, `docs/FilcoAssistUI_CODE_MAP.md`, `docs/gen_layers.py`,
`firmware/flash_tool/v72.py`, `firmware/flash_tool/emu.py`, các trang trong `firmware/backups/` và `firmware/configs/`),
cộng pseudocode FILCO Assist trong `decomp/annotated/` cho phần nhận dạng model và mã hóa ô.

## Ghi chú khi duyệt (PLAN APPROVED)

- Khoảng chờ đọc tối ưu (nếu đo được) chỉ áp cho model đã đo và nằm trong file model đó (`protocol.delay_ms.read`);
  mọi model khác giữ 100 ms.
- Nếu dùng `cargo-xwin`, README ghi rõ cách build lại trên máy có Visual Studio / Build Tools bình thường (target
  MSVC chuẩn, không cần xwin), để người khác build được.

## Thay đổi sau review 1

- Mục 4.5 viết lại: trên thiết bị thật được chạy phiên đọc đầy đủ như `fnmap.py` (0x10 + 8 GetFeature + 0x02). Bỏ
  hẳn chế độ "đọc không 0x02". Vẫn cấm 0x11, 0x15, 0x16, 0x17, 0x06 và mọi lệnh khác ngoài 0x10/0x02. Đo khoảng
  chờ có giới hạn số phiên và điều kiện dừng.
- M0 bắt đầu bằng kiểm quyền admin và MSVC. Đã kiểm lúc sửa plan: phiên **không** có quyền admin, máy chưa có Visual
  Studio hay Build Tools. Thêm các đường không cần admin (mục 1.1).
- Thứ tự mốc: M0-M6, rồi M8, M7 làm cuối nếu còn thời gian; mỗi mốc kết thúc bằng commit qua `check.ps1`; thứ tự ưu
  tiên khi thiếu thời gian (mục 10).
- Thêm bộ ảnh chụp mọi màn hình vào `docs/screenshots/`, nhúng vào `STATUS.md`, kèm bảng tự so với trang "M10SP
  layer setup" (mục 6.5).
- Tên sản phẩm trung tính "Keyboard Layer Editor", đổi ở một chỗ; giấy phép và chữ ký số: mục 11.
- Góp ý nhỏ: kiểm fixtures không còn đường dẫn máy, tên người dùng, instance path; `default_usage` là mã HID do mình
  viết, không nhúng bảng keymap thô; ngân sách khởi động không nới âm thầm; commit khởi tạo trên `main` rồi làm trên
  `feat/initial-app`.

## Thay đổi 2026-11-01: giai đoạn 2, firmware trong ứng dụng

Người dùng quyết định (chi tiết ở mục 12):

- Repo độc lập: mọi phần làm bằng Rust trong repo này, không gọi hay phụ thuộc code Python của `rea_filco` nữa.
  `rea_filco` chỉ còn là nguồn tham khảo; dữ liệu golden lấy từ đó một lần và commit như dữ liệu.
- Người dùng có license firmware V72: firmware gốc và trình cập nhật chính hãng được commit vào repo và đi kèm bộ cài.
  Thay quy định cũ "repo không chứa firmware". Cài trên máy khác là chạy được, không cần gì thêm.
- Ứng dụng đọc cấu hình firmware từ bàn phím (lệnh HID 0x30, khối thông tin), hiển thị tap-hold, cho sửa tap-hold /
  Fn + phím / keymap mặc định, tự vá từ V72 gốc, kiểm trên trình giả lập Cortex-M0 viết bằng Rust, rồi nạp bằng trình
  cập nhật chính hãng (người dùng kiểm CheckSum và bấm Start).

## 0. Tóm tắt

- Ứng dụng desktop Windows, Rust 1.99.1 + Tauri 2.12, frontend Svelte 5 + TypeScript + Vite, không framework UI
  ngoài. Toàn bộ logic (mã hóa ô, giao thức, ghi an toàn) nằm trong các crate Rust thuần, test được không cần Tauri,
  không cần bàn phím.
- Model bàn phím là dữ liệu: file JSON trong `models/`, có JSON Schema sinh từ kiểu Rust và bộ kiểm tra ngữ nghĩa.
  Thêm model mới là thêm một file, không sửa lõi.
- HID qua crate `windows` (Win32 trực tiếp), làm đúng như `fnmap.py`: lọc interface theo HIDP_CAPS, mở với quyền
  truy cập 0, `HidD_SetFeature`/`HidD_GetFeature` 65 byte.
- Mọi lần ghi: đọc mới, so với bản người dùng đã sửa trên đó, sao lưu ra đĩa, xem khác biệt, xác nhận, kiểm FILCO
  Assist, ghi từng layer, đọc lại so từng byte, nhật ký ghi để phục hồi nếu đứt giữa chừng.
- Test nhiều tầng: codec (vét cạn + property), golden bytes sinh từ chính `fnmap.py`, thiết bị giả có tiêm lỗi,
  trang thật, schema, UI tự động bằng tauri-driver + WebDriver, ảnh chụp so pixel, benchmark có ngân sách.
- Đêm nay trên bàn phím thật chỉ có phiên đọc 0x10 + 0x02 như `fnmap.py`; mọi phiên ghi chỉ chạy trên thiết bị giả
  (mục 4.5).

## 1. Phiên bản và môi trường (tra ngày 2026-10-31)

| Thành phần | Phiên bản | Nguồn |
|---|---|---|
| Rust stable | 1.99.1 (2026-10-13) | `https://static.rust-lang.org/dist/channel-rust-stable.toml` (date 2026-10-15) |
| tauri / tauri-build / tauri-cli | 2.12.5 / 2.7.4 / 2.12.4 | crates.io API |
| @tauri-apps/api, @tauri-apps/cli | 2.12.4 | npm registry |
| windows (crate) | 0.62.2 | crates.io |
| Svelte / Vite | 5.57.3 / 8.3.5 | npm registry |
| proptest / criterion | 1.12.0 / 0.8.2 | crates.io |
| serde / schemars / jsonschema | 1.0.229 / 1.2.3 / 0.60.2 | crates.io |
| Node / npm (đã có trên máy) | 24.22.1 / 11.19.1 | máy |
| WebView2 Runtime (đã có) | 156.0.4314.50 | registry |

`rust-toolchain.toml` ghim `1.99.1` để build lặp lại được. TypeScript: npm `latest` là 7.0.2 (bản biên dịch
native); nếu `svelte-check` chưa hỗ trợ 7.x thì ghim 6.x và ghi vào `docs/DECISIONS.md`.

### 1.1 Toolchain khi không có quyền admin

Đã kiểm: phiên không có quyền admin, chưa có Visual Studio / Build Tools, chưa có Rust. Đêm nay không ai bấm UAC. Bước
đầu tiên của M0, theo thứ tự, dừng ở đường đầu tiên chạy được:

1. Kiểm lại quyền admin và `vswhere` / thư mục Visual Studio (phòng khi có bản cài ở chỗ khác).
2. Cài `rustup` theo user (`winget install Rustlang.Rustup --scope user`, hoặc `rustup-init.exe` tải từ
   `static.rust-lang.org`, không cần admin), toolchain 1.99.1.
3. Thử cài Build Tools theo cách không nâng quyền (`winget ... --scope user`). Gần như chắc sẽ cần UAC; nếu installer
   hiện UAC hoặc trả lỗi quyền thì bỏ qua, không thử lặp.
4. Đường thay thế thứ nhất, target MSVC không cần Visual Studio: `cargo-xwin` (dùng `lld-link` và `clang-cl` của LLVM
   bản zip, tải CRT và Windows SDK của Microsoft về thư mục user; xwin yêu cầu chấp nhận giấy phép của Microsoft cho
   các gói này, ghi vào `DECISIONS.md`). Tauri, `windows` crate và WebView2 đều chạy trên target
   `x86_64-pc-windows-msvc` này.
5. Đường thay thế thứ hai: target `x86_64-pc-windows-gnullvm` với llvm-mingw bản zip (không cần admin). Tauri không
   chính thức hỗ trợ target GNU trên Windows; chỉ dùng nếu đường 4 hỏng, và chạy đủ E2E trên bản build đó.
6. Kiểm linker ngay bằng một crate "hello" và một app Tauri trống trước khi viết code khác. Nếu cả 4 và 5 đều hỏng: ghi
   chặn vào `STATUS.md`, báo reviewer, và vẫn viết + kiểm tra những phần không cần link (frontend, `cargo check` các
   crate Rust thuần nếu được).

NSIS do Tauri bundler tự tải vào thư mục cache của user; msedgedriver là file zip, không cần admin.

## 2. Kiến trúc

### 2.1 Tổng thể

```
┌──────────────────────── Tauri app (src-tauri) ────────────────────────┐
│ WebView2: Svelte UI  ──invoke/event (JSON, kiểu sinh bằng ts-rs)──►   │
│                                    commands.rs (mỏng, không logic)     │
│                                          │                             │
│   filco-service: EditorSession, SafeWriter, BackupStore, Journal,      │
│                  FilcoAssistGuard, DeviceWatcher                       │
│          │                     │                        │              │
│   filco-core              filco-protocol          filco-hid-windows    │
│   (ô, trang, diff,        (gói, checksum,         (SetupDi, HidD_*,    │
│    model, file)            phiên, Transport)       CM_Register_Notif.) │
│                                │                                       │
│                           filco-sim (thiết bị giả, tiêm lỗi, đồng hồ ảo)│
└────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Workspace

| Crate / thư mục | Nội dung | Phụ thuộc |
|---|---|---|
| `crates/filco-core` | `Slot` (giải mã/mã hóa không mất byte), `Page` (512 byte), `LayerSet`, diff, `ModelDescriptor` + kiểm hợp lệ, định dạng sao lưu (tương thích `fnmap.py`) và file xuất/nhập. Không I/O thiết bị, `#![forbid(unsafe_code)]` | serde, schemars, sha2, thiserror |
| `crates/filco-protocol` | Gói feature report, ACK, checksum, phiên đọc/ghi trang, trait `Transport` (set/get feature) và `Clock` (để test không ngủ thật). Đồng bộ, không async | filco-core |
| `crates/filco-hid-windows` | `Transport` thật: liệt kê, lọc theo caps, mở quyền 0, đọc VID/PID/REV, theo dõi cắm/rút. `unsafe` chỉ nằm ở đây, mỗi khối có chú thích an toàn | windows |
| `crates/filco-sim` | Thiết bị giả theo hành vi firmware (mục 6.3), kịch bản lỗi, đồng hồ ảo | filco-protocol |
| `crates/filco-service` | Phiên chỉnh sửa (bản gốc, bản đang sửa, undo/redo), luồng ghi an toàn, kho sao lưu, nhật ký ghi, phát hiện FILCO Assist, đọc profile FILCO Assist (chỉ đọc) | 4 crate trên |
| `crates/filco-cli` | Công cụ dòng lệnh cho dev và test: `list`, `read --compare <thư mục>`, `decode`, `validate-model`, `bench`. Chỉ có lệnh ghi khi bật cờ biên dịch `write` và chỉ với thiết bị giả trong bản đêm nay | filco-service |
| `src-tauri` | Ứng dụng Tauri: lệnh IPC mỏng, trạng thái, sự kiện tiến độ, cửa sổ, bộ cài | filco-service |
| `ui/` | Svelte 5 + TS + Vite | @tauri-apps/api |
| `models/` | Mô tả model JSON; `models/schema/model.schema.json` sinh tự động | |
| `tests/fixtures/` | Trang layer thật của người dùng, golden transcript sinh từ `fnmap.py` | |
| `tools/golden/` | Script Python sinh golden từ `fnmap.py` (đọc `fnmap.py` theo đường dẫn ngoài repo, không chép) | |
| `tools/fwsim/` | (Tùy chọn, mốc M7) cầu nối giả lập firmware thật | |

Ranh giới Rust và frontend: Rust là nguồn sự thật duy nhất cho trạng thái thiết bị, trang đang sửa, undo/redo, diff
và mọi mã hóa byte. Frontend chỉ hiển thị và gửi ý định (`assign(layer, key, action)`, `undo`, `save`...). Lý do:
một codec duy nhất (không có bản TS lệch với Rust), test được hết logic bằng `cargo test`, UI không thể tự ghép byte
sai. Một lần `invoke` trong Tauri 2 tốn cỡ dưới 1 ms, nằm trong ngân sách phản hồi (mục 8). Kiểu dữ liệu IPC sinh sang
TypeScript bằng `ts-rs`; script kiểm tra báo lỗi nếu file sinh ra cũ hơn mã Rust.

### 2.3 Chọn frontend: Svelte 5 (không SvelteKit)

- Biên dịch ra JS nhỏ (runtime vài KB), không virtual DOM: bàn phím SVG khoảng 70-100 phím cập nhật từng phím một,
  hợp với reactivity mịn của runes.
- Kiểm soát markup và CSS hoàn toàn, cần cho việc theo đúng style trang "M10SP layer setup" đến từng pixel.
- So sánh: React lớn hơn và cần thêm thư viện state; Solid tương đương về tốc độ nhưng hệ sinh thái test ít hơn;
  TS thuần thì phải tự viết lớp reactivity. Svelte cho bundle nhỏ nhất ở mức công sức bảo trì hợp lý.
- Không dùng thư viện component. Font IBM Plex Sans, Sans Condensed, Mono (giấy phép OFL) đóng gói cục bộ, không tải
  từ Google Fonts: ứng dụng chạy offline và CSP chặn mọi kết nối mạng.

## 3. Mô hình dữ liệu

### 3.1 Ô (slot)

Ô 4 byte `[b0, b1, b2, b3]`, loại = `b0 & 0x0f` (bảng nhảy firmware 0x76a0).

```rust
pub enum SlotKind {
    Empty,                                   // loại 0 và 8-15 (firmware coi như ô trống)
    Mouse(MouseAction),                      // loại 1
    Key { mods: ModMask, usage: u8 },        // loại 2: b1 = mặt nạ modifier (bit i -> 0xe0+i), b2 = mã HID
    Consumer(u16),                           // loại 3: media
    System(u8),                              // loại 4: ngủ, tắt máy, thức
    Macro { raw: [u8; 3] },                  // loại 6: chỉ hiển thị trong bản đầu
    Reserved { kind: u8, raw: [u8; 3] },     // loại 5, 7: chưa phân tích, chỉ hiển thị
    Extension { kind: u8, raw: [u8; 3] },    // loại 8-15: chừa cho bộ máy kiểu QMK (FAQ câu 9)
}
pub struct Slot { bytes: [u8; 4] }           // luôn giữ nguyên 4 byte gốc
```

- `Slot` luôn lưu đủ 4 byte; `SlotKind` là cách nhìn. Giải mã rồi mã hóa lại cho đúng 4 byte cũ với mọi giá trị
  (kể cả nibble cao của b0, b3 khác 0, mã HID lạ như 0xa4 Fn). Test vét cạn cho mọi tổ hợp b0 và property test cho
  phần còn lại.
- Sửa một ô chỉ sinh byte qua `encode(SlotKind)` có kiểm giới hạn; ô "Empty" luôn mã hóa thành `00 00 00 00`.
- Mã hóa loại 1, 3, 4 lấy theo firmware và FILCO Assist (mục 3.2). Loại 6 (macro, lệnh 0x15), 5, 7 và 8-15 đọc được,
  hiển thị nguyên byte, không sửa trong bản đầu; kiến trúc đã có chỗ: thêm biến thể `SlotKind`, thêm tab trong UI,
  thêm khả năng `slot_types` trong mô tả model.

### 3.2 Mã hóa ô loại 1, 3, 4

| Loại | Byte | Nguồn | Mức chắc chắn |
|---|---|---|---|
| 1 chuột, click | `01 01 nút 00`, nút ∈ {1 trái, 2 phải, 4 giữa, 8 lùi, 0x10 tiến} | firmware 0x90aa; bộ giải mã FILCO Assist 0x6ed810 | 1 và 2 đã chạy trên máy thật (layer 3); nhãn 4/8/0x10 suy theo quy ước HID |
| 1 chuột, cuộn | `01 03 lượng 00`, lượng có dấu 8 bit (01 lên, ff xuống) | như trên | đã chạy trên máy thật |
| 1 chuột, di | `01 04 x y`, x và y có dấu 8 bit | firmware 0x90aa | phân tích firmware, chưa thử máy thật |
| 3 media | `03 lo hi 00`, mã Consumer 16 bit little-endian ở byte 1-2 | firmware 0x8f92; FILCO Assist ghép `b1 \| b2 << 8` | bố cục byte chắc chắn. FILCO Assist chỉ đưa 7 mã: 0xB5, 0xB6, 0xB7, 0xCD, 0xE2, 0xE9, 0xEA |
| 4 hệ thống | chưa rõ byte nào chứa mã | firmware 0x8fcc (V60) | phải phân tích ở M1 |

- Media: UI đưa 7 mã của FILCO Assist làm nhóm "đã kiểm"; các mã Consumer phổ biến khác (trình duyệt, máy tính,
  độ sáng) vào nhóm "chưa kiểm trên thiết bị" có ghi chú; tab Nâng cao nhập được mã 16 bit bất kỳ.
- Loại 4: ở M1 disassemble trình xử lý loại 4 trong ảnh V72 (chỉ đọc file ảnh có sẵn) để biết byte nào là mã System
  Control (0x81 tắt máy, 0x82 ngủ, 0x83 thức theo HID). Kết luận được thì cho sửa với 3 mã đó; không kết luận được thì
  loại 4 chỉ xem và sửa bằng hex, ghi vào `DECISIONS.md` và checklist sáng mai.
- FILCO Assist ghi "Hot Key" (loại nội bộ 8 của nó) xuống ô dưới dạng loại 2, nên ứng dụng không cần biết loại đó.

### 3.3 Trang và layer

- `Page([u8; 512])`, offset ô của chỉ số k do model quy định; với giao thức `costar-v1` là
  `0x40*(k&7) + 4*(k>>3)`. Mọi truy cập ô đi qua `SlotAddressing` của model, không có công thức ghi cứng ở chỗ khác.
- `LayerSet` = N trang (M10SP: 4). Diff giữa hai `LayerSet` cho danh sách `(layer, chỉ số, ô cũ, ô mới)`, kể cả ô của
  vị trí ma trận không có phím vật lý (ví dụ file nhập vào có byte lạ ở chỉ số 53): UI liệt kê riêng các ô này.

### 3.4 Mô tả model (dữ liệu, không phải code)

File `models/m10sp-us.json`, kiểm bằng `models/schema/model.schema.json` (sinh từ kiểu Rust bằng `schemars`, kiểm
trong test rằng file schema đang commit khớp với kiểu) và bộ kiểm ngữ nghĩa trong Rust. Ví dụ rút gọn:

```json
{
  "schema_version": 1,
  "id": "filco-m10sp-us",
  "name": "Majestouch Xacro M10SP US",
  "vendor": "FILCO (Diatec)",
  "support": "verified",
  "match": { "vendor_id": "0x2EFD", "product_ids": ["0x1972"],
             "hid_caps": { "usage_page": 1, "input_len": 9, "output_len": 2, "feature_len": 65 } },
  "protocol": { "id": "costar-v1", "report_len": 65, "page_len": 512, "chunk_len": 64,
                "slot_addressing": "costar-interleaved-8", "slot_count": 128,
                "delay_ms": { "read": 100, "write": 100 } },
  "layers": [ { "label": "1", "led": "#ffffff" }, { "label": "2", "led": "#2f63b0" },
              { "label": "3", "led": "#3bb26b" }, { "label": "4", "led": "#8f63d6" } ],
  "slot_types": { "editable": [0, 1, 2, 3, 4], "view_only": [5, 6, 7], "extension": [] },
  "layout": {
    "unit_px": 64, "split_after": ["M1", "M2", "M3", "M4", "M5"], "split_gap_u": 0.9,
    "keys": [ { "id": "Esc", "index": 0, "x": 0, "y": 0, "w": 1, "legend": "Esc", "default_usage": "0x29" } ]
  },
  "special_indexes": { "fn": [73] },
  "notes": [ "DIP switch đổi keymap mặc định lúc khởi động; ô trống làm theo keymap đó, không theo legend." ]
}
```

- `support`: `verified` (đã đọc và ghi trên thiết bị thật), `read_only` (chỉ đọc, nút Lưu bị tắt), `experimental`.
  Model mới thêm vào mặc định `read_only` tới khi có người kiểm ghi.
- Kiểm ngữ nghĩa: id phím và chỉ số duy nhất, chỉ số < `slot_count`, phím không chồng lên nhau, `slot_addressing`
  là song ánh vào các offset bội 4 trong trang, VID/PID không trùng giữa các model, màu hợp lệ, mã HID hợp lệ.
- Model nạp từ hai nơi: nhúng sẵn trong ứng dụng (`include_str!`, kiểm lúc build qua test) và thư mục người dùng
  `%APPDATA%\FilcoConfigurator\models\*.json` (cho cộng đồng thêm model). Model ngoài không hợp lệ thì bị bỏ qua và
  báo trong UI; model ngoài không được ghi đè model nhúng cùng id.
- Biến thể giao thức: `protocol.id` chọn một cài đặt trong `filco-protocol` qua trait `ConfigProtocol`. Hiện chỉ có
  `costar-v1`. Khác biệt giữa model (số ô, layer, cách tính offset, mã không được gán) đều là tham số trong file
  model.
- Model mà FILCO Assist 2.0.97 hỗ trợ, nhận theo VID 0x2EFD + PID (hàm 0x571260, không dùng lệnh 0x05):

  | PID | Mã nội bộ | Tên | Trạng thái trong ứng dụng |
  |---|---|---|---|
  | 0x1967 | F67 | Majestouch Xacro M3A US | chưa có bố cục; nhận diện được, báo "chưa hỗ trợ" |
  | 0x1970 | F70 | Majestouch Xacro M3A JP | như trên |
  | 0x1972 | F72 | Majestouch Xacro M10SP US | `verified` sau checklist sáng mai |
  | 0x1976 | F76 | Majestouch Xacro M10SP JP | như M3A |

  FILCO Assist dùng chung một lớp giao thức, một trang 512 byte, 4 layer và cùng offset phím M cho cả 4 model, nên
  `costar-v1` gần như chắc dùng được cho cả bốn; công thức offset trên firmware M3A chưa kiểm. Bảng PID này nằm trong
  dữ liệu (`models/known-devices.json`) để UI gọi đúng tên thiết bị chưa có bố cục, không nằm trong code.
- Bố cục M10SP dựng từ `ROWS` trong `gen_layers.py` (hình học) cộng bảng chỉ số ma trận đọc từ keymap mặc định của
  ảnh V72 (tên do mình viết, không chép dữ liệu nhị phân). Vị trí đã kiểm trên máy thật: 27 P, 70 M10, 73 Fn,
  76 Space phải, 77 Space trái, 83 Alt trái. Chưa chắc: Alt phải là 78 hay 87 (keymap có cả hai, một chỉ số là của
  bản JP); mục 11.

### 3.5 File sao lưu và file xuất

- Sao lưu: đúng định dạng `fnmap.py` (thư mục `layer{N}.bin` + `manifest.json` có SHA-256 từng layer), thêm khóa
  `app` (phiên bản ứng dụng, id model, lý do sao lưu). `fnmap.py restore` đọc được bản sao lưu của ứng dụng và ngược
  lại; có test cả hai chiều. Lưu ở `%LOCALAPPDATA%\FilcoConfigurator\backups\<model>\<thời gian>\`, ghi file tạm rồi
  đổi tên, `fsync` trước khi cho phép ghi thiết bị.
- Xuất/nhập: một file `.filco-layers.json` (định dạng văn bản, dễ chia sẻ): `format_version`, id model, PID, thời
  gian, mỗi layer là hex 512 byte kèm SHA-256, cộng danh sách ô đã giải mã để người đọc hiểu (chỉ để đọc; khi nhập chỉ
  dùng hex). Nhập cũng nhận thư mục sao lưu kiểu `fnmap.py`. Nhập chỉ nạp vào bản đang sửa, không ghi thiết bị; ghi
  vẫn qua diff và xác nhận.

## 4. Giao thức HID trên Windows

### 4.1 Tìm và mở interface (đúng như `fnmap.py`)

1. `HidD_GetHidGuid`, `SetupDiGetClassDevsW(DIGCF_PRESENT | DIGCF_DEVICEINTERFACE)`, duyệt
   `SetupDiEnumDeviceInterfaces` + `SetupDiGetDeviceInterfaceDetailW`.
2. Mỗi đường dẫn: `CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL)`. Quyền
   truy cập 0 là bắt buộc: collection bàn phím bị Windows giữ độc quyền đọc/ghi, nhưng feature report vẫn đi được với
   handle quyền 0 (cách FILCO Assist và `fnmap.py` làm).
3. `HidD_GetAttributes` (VID, PID, VersionNumber = REV firmware, ví dụ 0x0072) và `HidD_GetPreparsedData` +
   `HidP_GetCaps`. Khớp khi VID và PID có trong một model, `UsagePage == 1`, `InputReportByteLength == 9`,
   `OutputReportByteLength == 2`, `FeatureReportByteLength == 65`. Trên M10SP đó là interface MI_00. Thiết bị có VID
   FILCO và caps khớp nhưng PID lạ: hiện "model chưa hỗ trợ", chỉ cho xem thông tin.
4. Hơn một interface khớp: không tự chọn; UI cho chọn thiết bị (fnmap.py thì từ chối).
5. Cắm/rút: `CM_Register_Notification` với `GUID_DEVINTERFACE_HID`, lọc theo đường dẫn; thay cho message window.

Vì sao không dùng crate `hidapi`: nó không trả độ dài report trong caps nên không lọc được như FILCO Assist, và
đường mở thiết bị thử quyền đọc/ghi trước. Crate `windows` cho gọi đúng từng API như `fnmap.py`, không kéo thêm C.

### 4.2 Gói và phiên (đối chiếu `fnmap.py` từng byte)

| Bước | Gói gửi (65 byte, byte 0 = report ID 0) | Kiểm phản hồi |
|---|---|---|
| Lệnh | `[1]=0x04, [2]=cmd, [9]=len, [0xb]=layer` (với 0x10/0x11) | GetFeature: `[1]==4, [2]==cmd, [4]==1, [9]==len` |
| Đọc trang (0x10, len 8) | sau ACK: 8 lần GetFeature, dữ liệu là byte 1..64 | checksum += tổng cả 65 byte mỗi gói |
| Ghi trang (0x11, len 8) | sau ACK: 8 lần SetFeature `[0] + 64 byte` | checksum += tổng 65 byte mỗi gói |
| Kết thúc (0x02, len 0) | gói lệnh thường | checksum thiết bị ở `[5..9]` little-endian phải bằng checksum máy |

- Mỗi SetFeature/GetFeature có khoảng chờ cấu hình được theo model (`delay_ms`, mặc định 100 ms như FILCO Assist).
- Ghi trang xong: chờ, đọc lại bằng 0x10 + 0x02 và so 512 byte.
- Lỗi được phân loại: `Disconnected`, `NoAck { cmd, got }`, `ChecksumMismatch { device, host }`,
  `ReadbackMismatch { layer, first_diff }`, `Io(win32 error)`. Không tự thử lại lệnh ghi; lệnh đọc thử lại tối đa 2
  lần.
- Lệnh 0x05/0x06 (basic info), 0x12-0x17 (macro, Game Mode) không có trong bản đầu. Riêng 0x06 ghi đè cả khối
  basic info, gồm cờ reset về mặc định ở byte 0x10 (FILCO Assist dùng nó để tắt ghi macro phần cứng và để reset), nên
  ứng dụng không bao giờ gửi 0x06. Giai đoạn sau có thể dùng 0x05 (chỉ đọc) để hiện layer đang dùng (byte 0x00). Enum lệnh trong
  `filco-protocol` chỉ có 0x02, 0x10, 0x11; thêm lệnh mới phải thêm có chủ đích.

### 4.3 Luồng I/O

Mọi I/O thiết bị chạy trên một luồng worker riêng (một handle, các lệnh xếp hàng), không bao giờ trên luồng UI. Tiến
độ (gói thứ mấy, layer nào) gửi lên UI qua event. Hủy chỉ được giữa hai layer, không giữa các gói của một layer.

### 4.4 FILCO Assist

- Phát hiện `FilcoAssistUI.exe` và `FilcoAssistTray.exe` qua `CreateToolhelp32Snapshot`. Đang chạy thì hiện cảnh báo
  thường trực, nút Lưu bị tắt kèm giải thích; kiểm lại ngay trước khi gửi gói ghi đầu tiên.
- Đọc (chỉ đọc) `%APPDATA%\FILCO\filco_assist_config.json`: layer nào gắn profile thì cảnh báo "FILCO Assist có thể
  ghi đè layer này khi lưu profile". Không sửa file này trong bản đầu (giai đoạn sau: tùy chọn đồng bộ `AssignedKeys`
  như repo phân tích từng làm tay).

### 4.5 Ràng buộc đêm nay với thiết bị thật

- Được phép trên thiết bị thật: liệt kê và mở interface (quyền 0, attributes, caps) và phiên đọc đầy đủ y như
  `fnmap.py show/backup`: 0x10 + 8 GetFeature + 0x02 (0x02 trong phiên đọc chỉ trả checksum, không ghi flash; đường
  này đã chạy nhiều lần trên chính bàn phím này). Trước mỗi lần chạy kiểm FILCO Assist đã đóng.
- Cấm trên thiết bị thật đêm nay: 0x11, 0x15, 0x16, 0x17, 0x06 và mọi lệnh khác ngoài 0x10/0x02. Phiên ghi
  (0x11 ... 0x02) chỉ chạy trên thiết bị giả.
- Chặn bằng code, không chỉ bằng kỷ luật: `filco-hid-windows` có `WritePolicy`. Chế độ `ReadOnly` từ chối trước khi
  gọi `HidD_SetFeature` mọi gói không phải gói lệnh 0x10 hay 0x02 (kể cả gói dữ liệu); có unit test cho chặn này.
  `filco-cli` và mọi test chạm thiết bị thật đêm nay chỉ tạo transport ở `ReadOnly`. Ứng dụng chỉ chuyển sang
  `ReadWrite` trong `SafeWriter`, ngay sau khi người dùng bấm xác nhận; đêm nay không bấm Lưu với thiết bị thật.
- Test tích hợp: đọc 4 layer, so SHA-256 với bản cấu hình hiện tại (`firmware/configs/2026-10-31_Layer4Snip`, khớp
  bản đọc lại `backups/20261031-165259`). Không khớp thì không phải lỗi của ứng dụng chừng nào checksum 0x02 đúng
  (người dùng có thể đã đổi layer); khi đó so với bản `fnmap.py backup` mới nhất thay vì tạo bản mới bằng `fnmap.py`.
- Đo khoảng chờ đọc nhỏ hơn 100 ms: các mức 50, 25, 15, 10 ms, mỗi mức tối đa 10 phiên đọc 4 layer, tổng không quá 50
  phiên đọc trang trên máy thật. Gặp checksum sai, NoAck hay dữ liệu khác bản 100 ms ở một mức thì dừng ngay, không
  thử mức nhỏ hơn. Bản phát hành chỉ dùng mức nhỏ hơn 100 ms nếu mức đó qua toàn bộ phép đo, và lấy mức lớn hơn kế
  tiếp làm biên an toàn; khoảng chờ ghi giữ 100 ms. Kết quả ghi vào `STATUS.md`.
- Ứng dụng phát hành dùng phiên đọc đầy đủ có 0x02 như FILCO Assist.

## 5. Luồng ghi an toàn

```
Bấm "Lưu N thay đổi"
  1. FILCO Assist đang chạy?            có ─► dừng, hướng dẫn đóng
  2. Đọc mới các layer sẽ ghi (0x10+0x02)
  3. So với bản gốc lúc bắt đầu sửa     khác ─► dừng: "thiết bị đã đổi từ lúc đọc", cho đọc lại hoặc gộp
  4. Sao lưu cả N layer ra đĩa, fsync, kiểm lại SHA-256 file vừa ghi
  5. Hộp thoại khác biệt: từng ô (layer, phím, cũ ─► mới, byte hex), ô ngoài bố cục tách riêng,
     cảnh báo profile FILCO Assist; nút "Ghi xuống bàn phím" phải bấm rõ ràng (không mặc định Enter)
  6. Kiểm lại FILCO Assist; ghi nhật ký ý định (journal) ra đĩa: bản sao lưu, layer, SHA-256 đích
  7. Với từng layer có thay đổi: 0x11 + 8 gói + 0x02 (checksum) ─► 0x10 đọc lại ─► so 512 byte
  8. Lỗi ở layer k: dừng; thử khôi phục các layer đã ghi và layer k từ bản sao lưu (cùng luồng 7);
     báo rõ từng layer: đã ghi / đã khôi phục / trạng thái không rõ
  9. Xong: xóa journal, bản gốc mới = trang đọc lại, lưu cấu hình đã ghi vào lịch sử
Khởi động hoặc cắm lại thấy journal còn ─► đọc thiết bị, so với bản sao lưu và đích,
  đề nghị khôi phục (vẫn qua diff + xác nhận)
```

- Chỉ ghi layer có thay đổi. Trang ghi xuống luôn là trang đọc mới ở bước 2 cộng các ô người dùng đã sửa, không dùng
  bản đọc cũ.
- Khôi phục từ bản sao lưu và nhập file đi cùng luồng này (bước 5 hiện diff so với thiết bị).
- `SafeWriter` nằm trong `filco-service`, test bằng thiết bị giả cho mọi điểm lỗi (mục 6.3).

## 6. Chiến lược test

### 6.1 Tầng và công cụ

| Tầng | Công cụ | Nội dung |
|---|---|---|
| Codec ô | `cargo test`, proptest | Vét cạn 16 giá trị loại x mẫu byte; property: `encode(decode(b)) == b` với mọi 4 byte; `decode(encode(k)) == k` với mọi `SlotKind` hợp lệ; ô ví dụ đã biết (`02 00 a4 00` Fn, `02 0a 16 00` Win+Shift+S, `01 03 ff 00` cuộn xuống) |
| Địa chỉ ô | `cargo test` | Song ánh 128 chỉ số vào 128 offset; bảng chỉ số đã kiểm (70 -> 0x1a0, 73 -> 0x064, 27 -> 0x0cc, 83 -> 0x0e8) |
| Gói và checksum | golden | Transcript từng byte của `fnmap.py` (mục 6.2) cho đọc và ghi các trang thật; Rust phải sinh đúng từng gói và cùng checksum |
| Phiên giao thức | `filco-sim` | Đọc, ghi, đọc lại; ACK sai từng trường; checksum sai; thiết bị trả dữ liệu khác khi đọc lại; rút cáp ở từng gói (1..8) của từng pha; timeout; thiết bị chậm |
| Luồng ghi an toàn | `filco-sim` + thư mục tạm | Mỗi bước 1-9 của mục 5 có test lỗi ở đúng bước đó; FILCO Assist giả (trait `ProcessProbe`); thiết bị bị đổi giữa lúc đọc và ghi; journal sau khi "mất điện" (dừng giữa chừng) rồi khởi động lại; đĩa đầy khi sao lưu |
| Trang thật | fixtures | Mọi trang trong fixtures giải mã không lỗi, mã hóa lại đúng byte, SHA-256 khớp manifest; diff giữa các mốc cấu hình khớp các README trong `configs/` (ví dụ Layer4Apps -> Layer4Snip chỉ khác ô P layer 4) |
| Định dạng file | `cargo test` + Python | Sao lưu ứng dụng đọc được bởi hàm `load_backup` của `fnmap.py` và ngược lại; file xuất round-trip; file hỏng, SHA-256 sai, model khác bị từ chối |
| Schema model | `cargo test` + `jsonschema` | Mọi file trong `models/` qua schema và kiểm ngữ nghĩa; bộ file hỏng có chủ đích (trùng chỉ số, chồng phím, PID trùng, offset trùng) đều bị bắt với thông báo đúng; schema commit khớp kiểu Rust |
| HID Windows | `filco-cli` trên máy thật, `ReadOnly` | Liệt kê và lọc caps; phiên đọc 0x10 + 0x02 đủ 4 layer so SHA-256; đo khoảng chờ theo giới hạn mục 4.5; unit test `WritePolicy::ReadOnly` chặn mọi gói khác |
| Frontend đơn vị | Vitest + @testing-library/svelte | Component bàn phím, inspector, bộ chọn hành động, hộp thoại diff với `invoke` giả |
| UI end-to-end | tauri-driver + msedgedriver (khớp WebView2 156) + WebdriverIO | Chạy bản build có feature `e2e-sim` (thiết bị giả): đọc 4 layer, chọn phím, gán phím + modifier, media, chuột, undo/redo, diff, xác nhận, ghi, đọc lại, lỗi giữa chừng hiện đúng thông báo và khôi phục, FILCO Assist đang chạy chặn ghi, rút cáp giữa lúc ghi, nhập/xuất, chế độ tối |
| Ảnh so pixel | WebDriver screenshot + pixelmatch | Cửa sổ cố định 1280x800 và 1024x700, sáng và tối; ảnh chuẩn commit trong repo; lệch quá ngưỡng thì fail và lưu ảnh khác biệt |
| Hiệu năng | criterion + đo trong E2E | Mục 8 |

Chống flaky: thiết bị giả dùng đồng hồ ảo nên test Rust không ngủ thật; proptest dùng seed cố định và lưu ca hỏng;
E2E chờ theo thuộc tính trạng thái (`data-state="ready"`) chứ không chờ thời gian; cuối mỗi mốc chạy E2E 3 lần liên
tiếp, một lần fail là sửa tận gốc chứ không thêm retry.

### 6.2 Golden bytes từ `fnmap.py`

`tools/golden/gen_fnmap_golden.py` nạp `fnmap.py` từ đường dẫn trong biến môi trường `FILCO_FNMAP` (không chép vào
repo), thay `hid.HidD_SetFeature`/`HidD_GetFeature` bằng thiết bị giả bằng Python (trả ACK, trả trang fixture, tính
checksum như firmware) và `time.sleep` bằng hàm rỗng, rồi chạy `Keyboard.read_page` và `Keyboard.write_page` trên các
trang thật. Kết quả là file JSON liệt kê từng gói theo thứ tự (chiều, 65 byte hex), commit vào
`tests/fixtures/golden/`. Test Rust chạy cùng kịch bản trên `filco-sim` có ghi transcript và so khớp từng byte. Như vậy
Rust được đối chiếu với chính code đã chạy đúng trên thiết bị thật, không qua bản dịch tay.

### 6.3 Thiết bị giả `filco-sim`

Mô hình hành vi theo `fnmap.py`, `FN_ANALYSIS.md` mục 2 và FAQ câu 6: bộ nhớ "flash" 4 trang, phiên lệnh (chờ ACK,
đếm gói, cộng checksum), 0x02 so checksum rồi mới "ghi flash" với phiên ghi, đọc trả nội dung flash. Tiêm lỗi theo
kịch bản khai báo: `fail_at(phase, packet, kind)`, với kind = rút cáp, ACK sai, checksum sai, đọc lại lệch, im lặng.
Có bộ đếm để test khẳng định số gói và thứ tự gói.

Hạn chế: đây là mô hình suy từ phân tích, không phải code firmware. Mốc M7 thử thay bằng firmware thật.

### 6.4 Giả lập firmware thật (mốc M7, có giới hạn thời gian)

Khả thi một phần. `emu.py` hiện chạy vòng lặp chính, quét ma trận và bộ điều phối phím, nhưng không mô phỏng USB và
không mô phỏng ghi flash (IAP). Cách làm:

1. Tìm trong ảnh V72 trình xử lý lệnh feature (V60 là 0x9fde, 0xa282, 0xa30e; V72 phải dò lại) và bộ đệm gói mà
   ngắt USB điền vào.
2. Trong `tools/fwsim/` (Python, Unicorn, code mới viết, nhận đường dẫn ảnh firmware của người dùng qua biến môi
   trường, không chép ảnh vào repo): đặt 64 byte gói vào bộ đệm, gọi thẳng trình xử lý, đọc gói trả lời; móc thanh ghi
   flash để ghi vào flash giả.
3. Cầu nối stdin/stdout giữa Rust (`Transport` mới tên `FwSimTransport`) và script; chạy lại toàn bộ test phiên giao
   thức và luồng ghi an toàn trên firmware thật.

Tiêu chí tiếp tục: trong 2 giờ, đọc 0x10 qua firmware giả lập ra đúng từng byte trang fixture và checksum 0x02 khớp.
Không đạt thì dừng, ghi lý do vào `DECISIONS.md` (ví dụ ngắt USB và trình xử lý quá dính vào thanh ghi phần cứng).
Test này không chạy trong bộ test mặc định vì cần ảnh firmware bản quyền nằm ngoài repo; chạy bằng
`scripts/fw-in-loop.ps1`. M7 chỉ làm sau M8, khi mọi thứ khác đã xong (mục 10).

### 6.5 Ảnh chụp để người dùng duyệt bằng mắt

Ảnh chuẩn do chính ứng dụng sinh chỉ bắt hồi quy, không chứng minh giao diện đúng. Vì vậy E2E còn xuất bộ ảnh chụp
mọi màn hình và trạng thái (không thiết bị, đang đọc, đã đọc 4 layer, chọn phím, từng tab hành động, phím đã sửa,
hộp thoại diff, đang ghi, lỗi giữa chừng, FILCO Assist đang chạy, sao lưu/khôi phục; mỗi cái ở sáng và tối) vào
`docs/screenshots/`, nhúng vào `STATUS.md`. Kèm một bảng tự so với trang "M10SP layer setup" (màu, font, nét phím,
legend, khoảng cách, chế độ tối), ghi rõ điểm nào còn khác và vì sao.

## 7. UI

### 7.1 Bố cục

```
┌────────────────────────────────────────────────────────────────────────────────┐
│ Majestouch Xacro M10SP US        ● đã kết nối · REV 0x0072   [Đọc lại] [⋯]    │
│ [● Layer 1] [● Layer 2] [● Layer 3] [● Layer 4]    [Sao lưu] [Lưu 3 thay đổi]  │
├────────────────────────────────────────────────────────────────────────────────┤
│  bàn phím vẽ nét theo model; phím đang chọn viền màu layer; phím đã sửa có     │
│  chấm màu; ô trống hiện legend mờ (làm theo keymap mặc định)                   │
├────────────────────────────────────────────────────────────────────────────────┤
│ Phím M10 · chỉ số 70 · ô 02 00 a4 00 = Fn        (đã sửa: Alt phải ─► Fn)      │
│ [Phím] [Modifier] [Media] [Chuột] [Hệ thống] [Nâng cao]   Macro: giai đoạn sau │
│  lưới phím chọn nhanh, ô tìm kiếm, ô nhập byte hex (tab Nâng cao)              │
└────────────────────────────────────────────────────────────────────────────────┘
```

- Theo đúng style trang "M10SP layer setup": nền giấy `#fbfbfa`, mực `#1d2126`, nét `#9aa1a8`, chế độ tối theo hệ
  thống kèm nút đổi, màu nhấn theo layer (layer 1 `#b4532a`, layer 2 `#2f63b0`, layer 3 `#2e7d4f`, layer 4
  `#6a4aa5`; bản tối như trong `gen_layers.py`), chấm LED đúng màu đèn thật, IBM Plex, phím vẽ hai lớp `outer`/
  `inner` bo 7/5 px, phím M nền `--soft`.
- Hàm vẽ bàn phím là component Svelte nhận mô tả model, cùng thông số hình học với `gen_layers.py` để hai trang trông
  giống nhau.
- Bàn phím dùng được hoàn toàn bằng bàn phím máy tính: mũi tên đi giữa các phím, 1-4 đổi layer, Ctrl+Z / Ctrl+Y,
  Ctrl+S mở hộp thoại lưu. Focus thấy rõ; nhãn ARIA cho từng phím.
- Ngôn ngữ: khung bảng chuỗi vi/en có từ đầu, mặc định theo ngôn ngữ hệ thống, vì sẽ chia sẻ cho người khác. Bắt
  buộc đủ hai thứ tiếng cho màn hình chính; các màn hình khác theo thứ tự ưu tiên ở mục 10.
- Trạng thái không có thiết bị, thiết bị chưa hỗ trợ, FILCO Assist đang chạy, đang đọc (tiến độ từng layer), lỗi:
  mỗi trạng thái có màn hình riêng, có ảnh chuẩn trong test pixel.
- Cửa sổ có viền chuẩn Windows (không tự vẽ titlebar), kích thước tối thiểu 1024x680.

### 7.2 Tính năng bản đầu

Có: xem 4 layer từ thiết bị; sửa ô loại 0-4 (trống, chuột, gõ phím + modifier, media, hệ thống); undo/redo; xem khác
biệt; ghi + đọc lại; sao lưu tự động và thủ công, danh sách bản sao lưu, khôi phục; xuất/nhập file; mở thư mục sao lưu
kiểu `fnmap.py`; chép một ô hay cả layer sang layer khác; cảnh báo FILCO Assist; vi/en; sáng/tối.

Chưa có (đã chừa chỗ): macro loại 6 và lệnh 0x15; loại 5, 7; ô 8-15 kiểu QMK (cần firmware vá); đồng bộ profile
FILCO Assist; Game Mode 0x16/0x17; tự cập nhật ứng dụng; model khác ngoài M10SP US.

## 8. Ngân sách hiệu năng

Đo trên máy này, bản release.

| Chỉ số | Ngân sách | Cách đo |
|---|---|---|
| Khởi động tới khung hình đầu | ≤ 400 ms | E2E: thời điểm spawn tới event `ui-ready` (performance mark đầu tiên sau khi Svelte mount), trung vị 10 lần |
| Khởi động tới tương tác được (không thiết bị) | ≤ 600 ms | như trên, tới `data-state="no-device"` |
| Liệt kê thiết bị | ≤ 50 ms | criterion trên máy thật (không gửi report) và `filco-cli bench enumerate` |
| Chi phí ứng dụng khi đọc 4 layer | ≤ 5 ms ngoài thời gian HID và khoảng chờ | criterion trên `filco-sim` với khoảng chờ 0 |
| Đọc đủ 4 layer, khoảng chờ 100 ms | ≤ 5,0 s (giới hạn giao thức: 48 khoảng chờ = 4,8 s) | E2E trên sim có độ trễ thật; máy thật ở test thủ công |
| Đọc đủ 4 layer, khoảng chờ đọc tối ưu | ≤ 1,0 s nếu thiết bị chịu được khoảng chờ ≤ 15 ms | đo trên máy thật theo mục 4.5, chọn khoảng chờ = 2 lần mức nhỏ nhất đạt 200 lần đọc liên tiếp không lỗi; khoảng chờ ghi giữ 100 ms tới khi người dùng kiểm |
| Bấm phím trên hình tới phím được chọn hiển thị | ≤ 16 ms (một khung hình) | E2E: event click tới `requestAnimationFrame` sau cập nhật, p95 trên 200 lần |
| Gán hành động tới phím vẽ lại (gồm IPC) | ≤ 50 ms p95 | như trên |
| Đổi layer | ≤ 16 ms p95 | như trên |
| Bộ nhớ tiến trình Rust | ≤ 40 MB private bytes | E2E đọc Process tree sau 4 layer + 100 thao tác |
| Bộ nhớ tổng gồm WebView2 | ≤ 220 MB | như trên, cộng các tiến trình msedgewebview2 con |
| Bộ cài NSIS | ≤ 6 MB | kiểm kích thước file trong script build |
| JS + CSS (không tính font) | ≤ 80 KB gzip | kiểm trong script build |

Mọi số đo ghi vào `docs/STATUS.md` cùng ngân sách. Vượt ngân sách là lỗi phải sửa hoặc ghi lý do và câu hỏi; không
nới ngân sách. Riêng khởi động 400 ms phụ thuộc thời gian WebView2 khởi tạo: nếu không đạt, ghi số đo thật, tách phần
của WebView2 (đo một cửa sổ Tauri trống làm mốc) và phần của ứng dụng, phần của ứng dụng phải ≤ 100 ms.

## 9. Chất lượng và quy ước

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` (cộng `clippy::pedantic` ở các crate
  lõi, có danh sách ngoại lệ ghi lý do), `#![forbid(unsafe_code)]` mọi crate trừ `filco-hid-windows`.
- `eslint`, `prettier --check`, `svelte-check` không cảnh báo.
- `cargo deny` kiểm giấy phép phụ thuộc (để chia sẻ được) và lỗ hổng đã biết.
- Script kiểm không có ký tự em dash trong mọi file văn bản của repo.
- `scripts/check.ps1` chạy tất cả ở trên cộng test; mỗi commit theo mốc phải qua script này.
- Tauri: CSP chặt (`default-src 'self'`), không bật plugin shell hay HTTP, quyền (capabilities) chỉ gồm lệnh của
  ứng dụng, hộp thoại file và mở thư mục sao lưu.
- Repo: nhánh `feat/initial-app`, commit theo mốc, không push, không remote, không merge vào main. Không chép
  firmware, bộ cài, manual, decompile hay nhị phân Diatec. Fixtures chỉ là trang layer cấu hình của người dùng,
  ghi nguồn trong `tests/fixtures/README.md`; manifest và README fixture không chứa đường dẫn máy, tên người dùng hay
  instance path (có test quét fixtures tìm `\\?\`, `C:\Users`, `D:\`, tên người dùng của máy).
- `default_usage` trong model là mã HID chuẩn của legend (Esc = 0x29) do mình viết; không nhúng bảng keymap thô của
  firmware.
- Tên sản phẩm: tên làm việc "Keyboard Layer Editor", phụ đề "Không chính thức, cho bàn phím FILCO dùng FILCO
  Assist". Tên chỉ khai báo ở `productName` trong `tauri.conf.json` và một hằng số UI; tên crate và định danh dùng
  `filco_configurator` / `keyboard-layer-editor`. Người dùng chọn tên cuối (ghi trong `DECISIONS.md`).
- Git: commit khởi tạo tối thiểu trên `main` (README, .gitignore, docs), rồi mọi việc trên `feat/initial-app`.

## 10. Các mốc

| Mốc | Nội dung | Xong khi |
|---|---|---|
| M0 | Đầu tiên: kiểm quyền admin + MSVC, cài toolchain theo mục 1.1, kiểm linker bằng crate hello và app Tauri trống. Sau đó: commit khởi tạo trên `main`, nhánh `feat/initial-app`, khung workspace, Tauri + Svelte, `scripts/check.ps1` | `check.ps1` exit 0; `cargo tauri build` ra bộ cài NSIS; cửa sổ mở được. Nếu chặn toolchain: ghi `STATUS.md`, báo reviewer |
| M1 | `filco-core`: ô, trang, địa chỉ, diff, model + schema + kiểm ngữ nghĩa, model M10SP US, sao lưu và file xuất | Test codec vét cạn + property, fixtures thật, schema, file hỏng; 100% hàm công khai có test; `check.ps1` xanh |
| M2 | `filco-protocol` + `filco-sim` + golden từ `fnmap.py` | Transcript Rust khớp golden từng byte cho đọc và ghi 4 trang thật; mọi kịch bản lỗi của mục 6.3 cho đúng loại lỗi |
| M3 | `filco-hid-windows` + `filco-cli` | Trên máy thật: liệt kê đúng 1 interface MI_00 của M10SP, VID/PID/REV đúng, thời gian liệt kê trong ngân sách; phiên đọc 0x10 + 0x02 đủ 4 layer, checksum đúng, SHA-256 khớp bản cấu hình (mục 4.5); đo khoảng chờ trong giới hạn 50 phiên |
| M4 | `filco-service`: phiên sửa, undo, SafeWriter, sao lưu, journal, FILCO Assist | Mỗi điểm lỗi trong mục 5 có test và cho đúng trạng thái cuối; tương thích sao lưu hai chiều với `fnmap.py` |
| M5 | UI: bàn phím, layer, inspector, bộ chọn hành động, diff, sao lưu/khôi phục, nhập/xuất, vi/en, tối | Vitest xanh; tự kiểm bằng ảnh chụp từng màn hình ở cả hai chế độ, sửa mọi lệch pixel thấy được |
| M6 | E2E tauri-driver, ảnh chuẩn, ảnh chụp duyệt bằng mắt (mục 6.5), benchmark, bộ cài | E2E xanh 3 lần liên tiếp; mọi ngân sách mục 8 đạt hoặc có số đo thật kèm nguyên nhân; bộ cài cài và gỡ được trên máy này (cài theo user, không cần admin) |
| M8 | Tài liệu: README (cài, dùng, an toàn, thêm model, cảnh báo SmartScreen, cách kiểm SHA-256 bộ cài), `STATUS.md` (kèm ảnh chụp, việc người dùng cần chọn: tên, giấy phép), checklist test thủ công | `STATUS.md` đủ mục theo bản giao việc; bộ test và benchmark chạy lại lần cuối |
| M7 | (Cuối cùng, chỉ khi còn thời gian; giới hạn 2 giờ) giả lập firmware thật cho HID | Đạt tiêu chí mục 6.4 hoặc ghi rõ vì sao dừng; cập nhật `STATUS.md` |

Thứ tự làm: M0, M1, M2, M3, M4, M5, M6, M8, rồi M7. Mỗi mốc kết thúc bằng một commit đã qua `check.ps1`, để lúc nào
cũng có trạng thái chạy được gần nhất. Nếu sắp hết thời gian hay usage, ưu tiên theo thứ tự: (1) bộ cài chạy được,
(2) đọc 4 layer từ thiết bị, (3) sửa ô loại 0-4, (4) ghi + đọc lại trên thiết bị giả qua đủ luồng an toàn, (5)
`STATUS.md` đầy đủ; sau đó mới đến tính năng phụ (nhập/xuất, chép layer, chế độ tối hoàn chỉnh, tiếng Anh ngoài màn
hình chính, M7). Khung chuỗi i18n có từ đầu, nhưng chỉ bắt buộc đủ tiếng Việt và tiếng Anh cho màn hình chính.

Checklist test thủ công cho sáng mai (bản đầy đủ nằm trong `STATUS.md`), làm theo thứ tự, mỗi bước có cách khôi phục
bằng `fnmap.py restore firmware\configs\2026-10-31_Layer4Snip`:

1. Đóng FILCO Assist. Mở ứng dụng, đọc 4 layer; so với `SETUP.md` (M1-M10, layer 3, layer 4).
2. Kiểm vị trí Alt phải trên hình (câu hỏi 78/87).
3. Sửa một ô vô hại ở layer 3 (ví dụ M1 = F13), xem diff, ghi; kiểm sao lưu được tạo, đọc lại khớp; gõ thử.
4. Hoàn tác bằng Khôi phục từ bản sao lưu trong ứng dụng; so SHA-256 với `Layer4Snip`.
5. Thử media (Fn không cần), chuột, Win+Shift+S trên một ô của layer 3; gõ thử từng ô.
6. Mở FILCO Assist: nút Lưu phải bị chặn.
7. Rút cáp giữa lúc ghi một layer thử: ứng dụng phải báo và đề nghị khôi phục khi cắm lại.
8. Xuất file, nhập lại, diff phải rỗng.

## 11. Rủi ro và câu hỏi mở

Câu hỏi đã được reviewer trả lời (review 1):

1. 0x02 trên thiết bị thật: được trong phiên đọc đầy đủ như `fnmap.py`; mục 4.5.
2. Tên sản phẩm: không dùng "FILCO" hay "Xacro" (nhãn của Diatec) làm tên. Tên làm việc "Keyboard Layer Editor",
   phụ đề "Không chính thức, cho bàn phím FILCO dùng FILCO Assist", đổi ở một chỗ (mục 9); người dùng chọn tên cuối.
3. Giấy phép: chưa thêm file LICENSE; `STATUS.md` ghi là người dùng chọn (gợi ý MIT OR Apache-2.0).
4. Không ký số: chấp nhận cho bản đầu; README nói rõ cảnh báo SmartScreen và cách kiểm SHA-256 bộ cài
   (`Get-FileHash`), `STATUS.md` ghi SHA-256 của bộ cài đã build.

Câu hỏi còn mở cho người dùng (ghi trong `STATUS.md`): tên sản phẩm, giấy phép, vị trí Alt phải (78 hay 87).

Rủi ro:

| Rủi ro | Ảnh hưởng | Giảm thiểu |
|---|---|---|
| Alt phải là chỉ số 78 hay 87 chưa kiểm | Gán nhầm ô cho phím Alt phải | Ghi rõ trong model là chưa kiểm, UI hiện ghi chú; checklist sáng mai kiểm (ghi tạm một ký tự như repo phân tích từng làm) |
| Bố cục và chỉ số ma trận các model khác chưa có (PID thì đã có, mục 3.4) | Kiến trúc mở nhưng chỉ có một model dùng được | Model mới mặc định `read_only`; README hướng dẫn thêm model; thiết bị đã biết PID được gọi đúng tên |
| Mã hóa ô loại 4 chưa rõ | Không sửa được System control | Phân tích firmware ở M1; nếu không được thì chỉ cho sửa hex (mục 3.2) |
| Thiết bị giả lệch firmware thật | Test xanh mà thiết bị lỗi | Golden từ `fnmap.py` đã chạy đúng trên máy thật; M7 giả lập firmware; đọc thật theo mục 4.5; checklist ghi có kiểm |
| Khoảng chờ nhỏ làm đọc sai | Dữ liệu sai trên UI | Checksum 0x02 trong bản phát hành; đọc lại khi checksum sai; khoảng chờ ghi giữ 100 ms |
| FILCO Assist ghi đè layer gắn profile | Mất thay đổi | Cảnh báo theo `filco_assist_config.json`; đồng bộ profile để giai đoạn sau |
| Firmware vá của người dùng chiếm phím (CapsLock, hai Space tap-hold) | Ô của các phím đó không có tác dụng | Giai đoạn 2 (mục 12): đọc khối thông tin firmware qua lệnh 0x30, hiển thị tap-hold và khóa ô của các phím đó |
| DIP đổi keymap mặc định | Legend của ô trống sai | Ghi chú trong UI: ô trống làm theo keymap của firmware, có thể bị DIP đổi |
| tauri-driver trên Windows cần msedgedriver đúng bản WebView2 | E2E không chạy | Tải msedgedriver đúng 156.0.4314.50 trong script; khóa phiên bản trong script |
| TypeScript 7 chưa hợp `svelte-check` | Lint frontend hỏng | Ghim bản hợp, ghi `DECISIONS.md` |
| Không có quyền admin để cài Build Tools (đã xác nhận) | Chặn M0 | Mục 1.1: `cargo-xwin` rồi `gnullvm`; kiểm linker trước mọi việc khác; chặn thật thì ghi `STATUS.md` và báo reviewer |
| Khối lượng lớn cho một đêm | Không xong hết | Thứ tự ưu tiên ở mục 10, commit qua `check.ps1` cuối mỗi mốc |

## 12. Giai đoạn 2: firmware trong ứng dụng (2026-11-01)

### 12.1 Mục tiêu

1. Đọc: khi kết nối, ứng dụng gửi lệnh 0x30. Firmware có khối thông tin trả về tên bản build và danh sách thay đổi
   (tap-hold, Fn + phím, keymap mặc định); firmware gốc hay bản vá cũ trả NACK và ứng dụng báo "firmware không có
   thông tin cấu hình". Không suy đoán, không cần người dùng khai báo.
2. Hiển thị: CapsLock vẽ "Esc / giữ Ctrl", Space trái "Space / giữ L3"; phím giữ để vào layer tô đậm trên layer đó
   (như trang "M10SP layer setup"); ô layer của phím tap-hold bị khóa kèm lý do; Fn + phím đã đổi hiện trong
   inspector.
3. Sửa và nạp: trang "Firmware" cho sửa tap-hold (tối đa 1 giữ = phím, 2 giữ = layer), Fn + phím, keymap mặc định.
   Ứng dụng vá từ V72 gốc, chạy toàn bộ kiểm thử trên trình giả lập, dựng trình cập nhật, mở nó; người dùng kiểm
   CheckSum và bấm Start; cắm lại thì ứng dụng đọc khối thông tin để xác nhận đúng bản vừa nạp.
4. Độc lập: bộ cài chứa firmware gốc và trình cập nhật; mọi bước trên chạy trong ứng dụng, không cần Python, không cần
   repo `rea_filco`.

### 12.2 Khối thông tin và lệnh 0x30

Định dạng và cách nối vào firmware như `rea_filco/firmware/flash_tool/fwinfo.py` (thiết kế, phân tích và kiểm trên
giả lập ngày 2026-11-01; `FN_ANALYSIS.md` mục 14 của repo đó): khối 512 byte ở 0xe600, code 68 byte ở 0xe500, nối
vào 0xa50a và 0xa5a0. Lệnh 0x30 đi đúng đường của lệnh đọc layer 0x10: ACK, 8 gói 64 byte, 0x02 trả checksum. Khối:
chữ ký `KLEI`, phiên bản định dạng, tên build, phiên bản firmware gốc và tag gốc, rồi các mục `loại, độ dài, nội
dung` (1 tap-hold giữ = phím, 2 tap-hold giữ = layer, 3 Fn + phím, 4 keymap mặc định); bên đọc bỏ qua loại chưa biết.

Phía giao thức: `filco-protocol` thêm lệnh 0x30 vào phiên đọc và vào danh sách lệnh chỉ đọc được phép; `filco-sim`
có chế độ có khối / trả NACK; dịch vụ đọc khối một lần sau khi đọc 4 layer.

### 12.3 Crate mới

| Crate | Nội dung |
|---|---|
| `crates/filco-firmware` | Ảnh 64 KB, đọc/ghi Intel HEX giữ đúng bố cục bản ghi của bản gốc, tổng word và tag CheckSum; mô tả V72 (địa chỉ bảng Fn, keymap, bitmap, các điểm nối) kèm kiểm byte gốc trước mỗi lần sửa; các thao tác vá: Fn + phím, keymap, vị trí Fn, tap-hold, khối thông tin; nhật ký từng byte; `FirmwareConfig` (serde) và mã hóa / đọc khối thông tin |
| `crates/thumb` | Bộ mã hóa Thumb-1 (ARMv6-M) + BL, nhãn và literal pool, đủ cho code tap-hold và lệnh 0x30; từ chối lệnh 32 bit khác BL. Thay Keystone |
| `crates/m0emu` | Trình thông dịch Cortex-M0 (ARMv6-M Thumb) thuần Rust: bộ nhớ flash/RAM/ngoại vi, hook theo địa chỉ, khởi động từ vector reset; mô hình ma trận phím, bỏ qua vòng chờ 0xb090, gọi ISR timer sau mỗi bước, máy tính HID giả (như `HidHost`) |
| `crates/filco-fwtest` | Các kiểm thử của `build.py` viết lại: so từng phím thường và kèm Fn với bản gốc, Fn + Alt + số, tap-hold (giữ = phím), giữ = layer (13 kịch bản), hai phím giữ = layer, khối thông tin qua HID, độ sâu stack tĩnh (port `stackdepth.py`). Chạy trong ứng dụng trước khi cho nạp, và trong `cargo test` |
| `crates/filco-updater` | Dựng trình cập nhật: chép trình cập nhật chính hãng, thay kho ZIP ở tài nguyên RT_RCDATA 4011 (HEX mới, `FWName`, `ISPTitle`) qua API tài nguyên Windows, đọc lại để kiểm; mở trình cập nhật và chờ bàn phím cắm lại |
| `firmware/` | `CT_F72US[0xF448]_20240125_0v72_PID1972.hex`, `M10SP_72US_V72_stock.exe`, SHA-256 của cả hai (kiểm lúc chạy), ghi chú license |

`filco-service` thêm `FirmwareSession` (đọc khối, sửa cấu hình firmware, build, kiểm thử, dựng trình cập nhật, theo
dõi lần nạp); `src-tauri` thêm lệnh IPC; UI thêm trang Firmware và phần hiển thị tap-hold trên bàn phím.

### 12.4 Kiểm chứng

- Golden từ `rea_filco` (lấy một lần, commit vào `tests/fixtures/firmware/` như dữ liệu): cấu hình TOML, ảnh `.bin`,
  `.hex`, `PATCH_DIFF.txt`, `TEST_REPORT.txt` của 5 bản build (`HJKL`, `HJKL_CapsTapHold`, `HJKL_CapsSpace`,
  `HJKL_CapsSpace2`, `HJKL_CapsSpace2_Info`). Rust phải ra ảnh và HEX giống từng byte, cùng tag, cùng nhật ký byte.
- `thumb`: mã sinh ra giống từng byte mã Keystone trong các ảnh golden.
- `m0emu`: chạy bộ kiểm thử trên 5 ảnh golden phải cho cùng kết quả từng dòng với `TEST_REPORT.txt` (sinh bởi
  Unicorn). Thêm test lệnh lẻ cho từng nhóm lệnh ARMv6-M (cờ, nhân, dịch, nạp/lưu, rẽ nhánh) đối chiếu ARM ARM.
- Mô hình HID giả phải đọc đúng 4 trang layer trên firmware gốc (ACK, 8 gói, checksum), như đã kiểm bằng Unicorn.
- `filco-updater`: với cùng HEX, trình cập nhật dựng ra giống từng byte bản `isp_exe.py` đã dựng (các bản trong
  `rea_filco/firmware/flash_tool/flashed/` đã nạp thành công).
- Thiết bị thật: chỉ đọc (0x10, 0x02, 0x30) do ứng dụng tự chạy; mọi lần nạp do người dùng bấm Start. Bản đầu tiên
  có lệnh 0x30 (`HJKL_CapsSpace2_Info`, CheckSum 0x1321) do `rea_filco` dựng; người dùng nạp để kiểm 0x30 trên máy
  thật trước khi ứng dụng phụ thuộc vào nó.

### 12.5 An toàn khi nạp

- Chỉ nạp ảnh đã qua toàn bộ kiểm thử trong cùng phiên; ứng dụng hiện CheckSum phải thấy trong trình cập nhật.
- Trước khi mở trình cập nhật: đọc và sao lưu 4 layer (trình cập nhật không ghi vùng cấu hình, nhưng vẫn sao lưu);
  sau khi cắm lại: đọc lại 4 layer so từng byte, đọc khối thông tin so tên build.
- Đường lui có sẵn trong ứng dụng: nạp lại bản đã nạp thành công gần nhất (giữ trong thư mục dữ liệu) hoặc V72 gốc.
  Bàn phím kẹt ở chế độ nạp vẫn được trình cập nhật nhận qua bootloader (VID 0x0C45, PID 0x7040).
- Không tự bấm Start, không tự động hóa cửa sổ trình cập nhật.

### 12.6 Bỏ Python khỏi repo

`tools/gen_m10sp_model.py` (sinh `models/m10sp-us.json`), `tools/gen_plan_html.py` và
`tools/golden/gen_fnmap_golden.py` được thay bằng lệnh Rust (`filco-cli`) hoặc bỏ: file model JSON thành nguồn chính,
golden hiện có giữ như dữ liệu. `check.ps1` kiểm không còn file `.py` nào trong repo.

### 12.7 Mốc

| Mốc | Nội dung | Xong khi |
|---|---|---|
| F1 | `filco-firmware` + `thumb`, firmware gốc vào `firmware/`, golden vào `tests/fixtures/firmware/` | 5 bản build ra ảnh, HEX, tag và nhật ký byte giống golden; `check.ps1` xanh |
| F2 | `m0emu` + `filco-fwtest` | Kết quả kiểm thử trên 5 ảnh golden khớp `TEST_REPORT.txt` từng dòng; mô hình HID đọc đúng 4 layer trên bản gốc; một lần build + kiểm thử dưới 15 giây |
| F3 | Lệnh 0x30 trong giao thức, sim, dịch vụ; hiển thị tap-hold, khóa ô, phím giữ layer; E2E và ảnh chuẩn | Trên thiết bị giả đủ hai trường hợp (có khối / NACK); trên bàn phím thật đã nạp 0x1321: đọc đúng khối |
| F4 | `filco-updater` | Trình cập nhật dựng bằng Rust giống từng byte bản `isp_exe.py` với cùng HEX |
| F5 | Trang Firmware: sửa, build, kiểm thử, nạp, xác nhận sau nạp, đường lui | E2E trên thiết bị giả (không chạy trình cập nhật thật trong test); checklist nạp thủ công có kiểm trên máy thật |
| F6 | Bỏ Python (12.6), README, STATUS, bộ cài có firmware | Không còn `.py`; bộ cài cài trên máy sạch chạy được đủ luồng; kích thước bộ cài trong ngân sách mới |

Ngân sách: bộ cài đến 8 MB (thêm trình cập nhật 1,9 MB và HEX 0,2 MB); build + kiểm thử firmware dưới 15 giây; bộ
nhớ trình giả lập không tính vào ngân sách lúc rảnh (chỉ chạy khi build).

### 12.8 Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Trình thông dịch Cortex-M0 tự viết sai ở lệnh hiếm | Đối chiếu kết quả với `TEST_REPORT.txt` của Unicorn trên 5 ảnh; test từng nhóm lệnh; dừng ngay ở lệnh chưa hỗ trợ thay vì đoán |
| Bộ mã hóa Thumb khác Keystone | So từng byte với ảnh golden; từ chối mọi lệnh 32 bit khác BL |
| Lệnh 0x30 chưa chạy trên bàn phím thật | Người dùng nạp bản 0x1321 trước F3; kiểm 0x30 và đọc layer sau đó |
| Nạp sai bản | Chỉ nạp ảnh vừa qua kiểm thử; CheckSum hiện trước khi bấm Start; xác nhận tên build sau khi cắm lại |
| API tài nguyên Windows khác nhau giữa máy | Đọc lại tài nguyên sau khi ghi và so; test trên máy này, bộ cài trên máy sạch |
