# Test fixtures

## `pages/`

Layer pages (512 bytes each, `layer0.bin` .. `layer3.bin`) read from the project owner's own FILCO Majestouch Xacro
M10SP US (VID 0x2EFD, PID 0x1972, firmware REV 0x0072) on 2026-10-31 with `fnmap.py` of the analysis repository.
They hold only the owner's layer configuration (key slots they assigned), not Diatec software or firmware data.
Manifests use the `fnmap.py` backup format; the device path of the original manifests was removed.

| Folder | Source in the analysis repository | Content |
|---|---|---|
| `2026-10-31_0737_original` | `firmware/backups/20261031-073738` | First backup, before any change: M10 = Right Alt (layer 2), Left Alt (layer 3), M9 = `b` (layer 1) |
| `2026-10-31_LAlt83` | `firmware/configs/2026-10-31_LAlt83` | M10 = Fn and slot 83 = Left Alt on all layers, layer 2 macros M1-M9 |
| `2026-10-31_SpaceNav` | `firmware/configs/2026-10-31_SpaceNav` | + layer 3 navigation (arrows, Home/End, F1-F12, mouse click and scroll) |
| `2026-10-31_Layer1M` | `firmware/configs/2026-10-31_Layer1M` | + layer 1 M1-M10 copied from layer 2 |
| `2026-10-31_Layer4Apps` | `firmware/configs/2026-10-31_Layer4Apps` | + layer 4 Ctrl+Shift+Alt+letter on 15 letters |
| `2026-10-31_Layer4Snip` | `firmware/configs/2026-10-31_Layer4Snip` | + layer 4 P = Win+Shift+S; current configuration of the keyboard |

## `golden/`

Byte-exact feature report transcripts produced by running `fnmap.py`'s own `read_page` / `write_page` against a
simulated device (`tools/golden/gen_fnmap_golden.py`). See that script for how they are made.
