# Firmware gốc và trình cập nhật (FILCO Majestouch Xacro M10SP US, V72)

Thư mục này chứa firmware V72 chính hãng và trình cập nhật chính hãng của Diatec/FILCO, đi kèm bộ cài để người dùng
nạp firmware lên bàn phím của chính mình.

## License

Firmware và trình cập nhật trong thư mục này là phần mềm có bản quyền của Diatec Corporation / FILCO, không thuộc giấy
phép MIT của repo.

## File và SHA-256

Kiểm lúc chạy (ứng dụng và task dựng firmware đều so SHA-256 trước khi dùng):

| File | SHA-256 |
|---|---|
| `CT_F72US[0xF448]_20240125_0v72_PID1972.hex` | `4d0a2cfb537416235ebea7aec056108aa77aebbe0ad8cda6ea85291ef77bb837` |
| `M10SP_72US_V72_stock.exe` | `b3a5f385d5da36f9a9bcc730b286ad6dc82b6e8340cd29e1fcf90fc1eeb641b2` |

Ảnh 64 KB giải mã từ file HEX trên có SHA-256 `5b09f9c1652286bd4dd648455d3f91e86114b7d2c3119c550e23ccedfdcb2cfa`
(đây là hằng số `STOCK_IMAGE_SHA256` mà mọi bước dựng firmware kiểm trước khi vá).

## Dùng để làm gì

- Phần "đọc + hiển thị" của ứng dụng không cần các file này để chạy; chúng đi kèm bộ cài để sẵn sàng nạp.
- Khi nạp một ảnh tùy biến, trình cập nhật chính hãng ở đây là cái người dùng chạy và bấm Start (ứng dụng không tự
  bấm Start, không tự động hóa cửa sổ của nó).
- Task dựng ảnh firmware tùy biến trong ứng dụng (xem `../docs/DEFERRED_FIRMWARE_BUILD.md`) dùng file HEX này làm
  điểm xuất phát và trình cập nhật này làm khuôn để dựng trình cập nhật mới.

`.gitattributes` đặt các file này ở chế độ nhị phân / `-text` để lưu đúng từng byte; SHA-256 ở trên phụ thuộc vào đó.
