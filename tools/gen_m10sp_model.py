r"""Write models/m10sp-us.json, the descriptor of the FILCO Majestouch Xacro M10SP US.

Usage (repo root, Python 3.11+, stdlib only):  python -I tools\gen_m10sp_model.py

Sources (all facts, written here by hand):
- Key rows and widths: the real split layout, as drawn by docs/gen_layers.py of the analysis repository
  (M1-M5 and M6-M10 form two columns between the halves, with a 0.9 unit gap after M1-M5).
- Matrix indexes: key names of the V72 default keymap (index -> HID usage), cross-checked on the real
  keyboard for P 27, M10 70, Fn 73, Space right 76, Space left 77, left Alt 83.
- default_usage: standard HID keyboard usage of the printed legend (US layout, all DIP switches off).
The script keeps the descriptor reviewable as data; the JSON file is what the application reads.
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
GAP = 0.9

# (id, width, matrix index, legend, default HID usage or None, tags)
ROWS = [
    [("Esc", 1, 0, "Esc", 0x29), ("1", 1, 8, "1", 0x1E), ("2", 1, 16, "2", 0x1F), ("3", 1, 24, "3", 0x20),
     ("4", 1, 32, "4", 0x21), ("5", 1, 40, "5", 0x22), ("6", 1, 48, "6", 0x23), ("M1", 1, 56, "M1", None),
     ("M6", 1, 64, "M6", None), ("7", 1, 1, "7", 0x24), ("8", 1, 9, "8", 0x25), ("9", 1, 17, "9", 0x26),
     ("0", 1, 25, "0", 0x27), ("-", 1, 33, "-", 0x2D), ("=", 1, 41, "=", 0x2E), ("Bksp", 2, 49, "Backspace", 0x2A)],
    [("Tab", 1.5, 2, "Tab", 0x2B), ("Q", 1, 10, "Q", 0x14), ("W", 1, 18, "W", 0x1A), ("E", 1, 26, "E", 0x08),
     ("R", 1, 34, "R", 0x15), ("T", 1, 42, "T", 0x17), ("M2", 1, 57, "M2", None), ("M7", 1, 65, "M7", None),
     ("Y", 1, 50, "Y", 0x1C), ("U", 1, 3, "U", 0x18), ("I", 1, 11, "I", 0x0C), ("O", 1, 19, "O", 0x12),
     ("P", 1, 27, "P", 0x13), ("[", 1, 35, "[", 0x2F), ("]", 1, 43, "]", 0x30), ("\\", 1.5, 51, "\\", 0x31)],
    [("Caps", 1.75, 4, "Caps Lock", 0x39), ("A", 1, 12, "A", 0x04), ("S", 1, 20, "S", 0x16),
     ("D", 1, 28, "D", 0x07), ("F", 1, 36, "F", 0x09), ("G", 1, 44, "G", 0x0A), ("M3", 1, 58, "M3", None),
     ("M8", 1, 66, "M8", None), ("H", 1, 52, "H", 0x0B), ("J", 1, 5, "J", 0x0D), ("K", 1, 13, "K", 0x0E),
     ("L", 1, 21, "L", 0x0F), (";", 1, 29, ";", 0x33), ("'", 1, 37, "'", 0x34), ("Enter", 2.25, 45, "Enter", 0x28)],
    [("LShift", 2.25, 6, "Shift", 0xE1), ("Z", 1, 14, "Z", 0x1D), ("X", 1, 22, "X", 0x1B), ("C", 1, 30, "C", 0x06),
     ("V", 1, 38, "V", 0x19), ("B", 1, 46, "B", 0x05), ("M4", 1, 60, "M4", None), ("M9", 1, 68, "M9", None),
     ("N", 1, 54, "N", 0x11), ("M", 1, 7, "M", 0x10), (",", 1, 15, ",", 0x36), (".", 1, 23, ".", 0x37),
     ("/", 1, 31, "/", 0x38), ("RShift", 2.75, 39, "Shift", 0xE5)],
    [("LCtrl", 1.25, 71, "Ctrl", 0xE0), ("LWin", 1.25, 82, "Win", 0xE3), ("LAlt", 1.25, 83, "Alt", 0xE2),
     ("SpaceL", 3.125, 77, "Space", 0x2C), ("M5", 1, 62, "M5", None), ("M10", 1, 70, "M10", None),
     ("SpaceR", 3.125, 76, "Space", 0x2C), ("RAlt", 1.25, 78, "Alt", 0xE6), ("RWin", 1.25, 79, "Win", 0xE7),
     ("Fn", 1.25, 73, "Fn", 0xA4), ("RCtrl", 1.25, 69, "Ctrl", 0xE4)],
]

NOTES = {
    "RAlt": "Chỉ số 78 suy từ keymap mặc định (keymap còn có Alt phải ở 87, có lẽ của bản JP); chưa kiểm trên bàn "
            "phím thật.",
    "LAlt": "DIP ③ có thể biến phím này thành Fn2; ô layer vẫn thắng khi không giữ Fn (đã kiểm trên máy thật).",
    "Fn": "Ô của phím Fn bị bỏ qua khi đang giữ Fn. Bộ ghi macro phần cứng nhận Fn theo vị trí này.",
}


def keys():
    out = []
    for y, row in enumerate(ROWS):
        x = 0.0
        for kid, w, index, legend, usage in row:
            k = {"id": kid, "index": index, "x": round(x, 4), "y": y}
            if w != 1:
                k["w"] = w
            k["legend"] = legend
            if usage is not None:
                k["default_usage"] = f"0x{usage:02X}"
            tags = []
            if kid.startswith("M") and kid[1:].isdigit():
                tags.append("m-key")
            if kid == "RAlt":
                tags.append("unverified")
            if tags:
                k["tags"] = tags
            if kid in NOTES:
                k["note"] = NOTES[kid]
            out.append(k)
            x += w + (GAP if kid in ("M1", "M2", "M3", "M4", "M5") else 0)
    return out


MODEL = {
    "$schema": "./schema/model.schema.json",
    "schema_version": 1,
    "id": "filco-m10sp-us",
    "name": "Majestouch Xacro M10SP US",
    "vendor": "FILCO (Diatec)",
    "support": "experimental",
    "match": {
        "vendor_id": "0x2EFD",
        "product_ids": ["0x1972"],
        "hid_caps": {"usage_page": 1, "input_len": 9, "output_len": 2, "feature_len": 65},
    },
    "protocol": {
        "id": "costar-v1",
        "report_len": 65,
        "page_len": 512,
        "chunk_len": 64,
        "slot_addressing": "costar-interleaved-8",
        "delay_ms": {"read": 15, "write": 100},
    },
    "layers": [
        {"label": "1", "led": "#ffffff", "led_name": ["trắng", "white"]},
        {"label": "2", "led": "#2f63b0", "led_name": ["xanh dương", "blue"]},
        {"label": "3", "led": "#3bb26b", "led_name": ["xanh lá", "green"]},
        {"label": "4", "led": "#8f63d6", "led_name": ["tím", "purple"]},
    ],
    "slot_types": {"editable": [0, 1, 2, 3, 4], "view_only": [5, 6, 7]},
    "layout": {"keys": None},
    "notes": [
        "Ô trống làm theo keymap mặc định của firmware; công tắc DIP có thể đổi keymap đó, nên chữ in trên phím "
        "không phải lúc nào cũng đúng.",
        "Khi đang giữ Fn, firmware bỏ qua ô layer: lớp Fn cố định trong firmware.",
        "Firmware đã vá có thể giữ riêng một số phím (ví dụ tap-hold); ô của các phím đó có thể không có tác dụng.",
    ],
}


def main():
    MODEL["layout"]["keys"] = keys()
    text = json.dumps(MODEL, ensure_ascii=False, indent=2) + "\n"
    out = ROOT / "models" / "m10sp-us.json"
    out.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {out} ({len(MODEL['layout']['keys'])} keys)")


if __name__ == "__main__":
    main()
