r"""Produce byte-exact transcripts of fnmap.py's own read and write sessions, as golden test data.

Usage (repo root, Windows, Python 3.11+, stdlib only):
    set FILCO_FNMAP=D:\AI\rea_filco\firmware\tools\fnmap.py
    python -I tools\golden\gen_fnmap_golden.py

fnmap.py (from the analysis repository, not copied here) is loaded from FILCO_FNMAP. Its HID calls are redirected to
a small simulated keyboard defined below, and time.sleep to a no-op; then fnmap's `Keyboard.read_page` and
`Keyboard.write_page` run unchanged on the real layer pages in tests/fixtures/pages. Every SetFeature and
GetFeature (65 bytes) is written to tests/fixtures/golden/*.json in order. The Rust tests replay the same scenarios
against filco-protocol + filco-sim and require the identical transcript.

Nothing here touches a real device: the module-level `hid` object of fnmap is replaced before any call.
"""
import ctypes
import importlib.util
import json
import os
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PAGES = ROOT / "tests" / "fixtures" / "pages"
OUT = ROOT / "tests" / "fixtures" / "golden"
REPORT_LEN = 65


class FakeKeyboard:
    """Device model: ACK [1]=4 [2]=cmd [4]=1 [9]=len, finish ACK carries the session sum at [5..9]."""

    def __init__(self, pages):
        self.flash = [bytearray(p) for p in pages]
        self.pending = None
        self.cur = None
        self.log = []

    @staticmethod
    def ack(cmd, length, value=0):
        r = bytearray(REPORT_LEN)
        r[1], r[2], r[4], r[9] = 4, cmd, 1, length
        r[5:9] = value.to_bytes(4, "little")
        return bytes(r)

    def set_feature(self, data):
        self.log.append(("set", data.hex()))
        if data[1] == 4 and not any(data[i] for i in range(REPORT_LEN) if i not in (1, 2, 9, 0xB)):
            cmd, length, layer = data[2], data[9], data[0xB]
            if cmd == 0x02:
                cur, self.cur = self.cur, None
                if cur and cur["cmd"] == 0x11 and cur["chunk"] == 8:
                    self.flash[cur["layer"]] = bytearray(cur["buf"])
                self.pending = self.ack(0x02, length, cur["sum"] if cur else 0)
            else:
                self.cur = {"cmd": cmd, "layer": layer, "chunk": 0, "sum": 0, "buf": bytearray(512)}
                self.pending = self.ack(cmd, length)
            return
        cur = self.cur
        if cur and cur["cmd"] == 0x11 and cur["chunk"] < 8:
            off = cur["chunk"] * 64
            cur["buf"][off:off + 64] = data[1:]
            cur["sum"] += sum(data)
            cur["chunk"] += 1

    def get_feature(self):
        if self.pending is not None:
            r, self.pending = self.pending, None
        elif self.cur and self.cur["cmd"] == 0x10 and self.cur["chunk"] < 8:
            off = self.cur["chunk"] * 64
            r = bytes(1) + bytes(self.flash[self.cur["layer"]][off:off + 64])
            self.cur["sum"] += sum(r)
            self.cur["chunk"] += 1
        else:
            r = bytes(REPORT_LEN)
        self.log.append(("get", r.hex()))
        return r


class FakeHid:
    def __init__(self, device):
        self.device = device

    def HidD_SetFeature(self, handle, buf, length):  # noqa: N802 (Win32 name)
        assert length == REPORT_LEN
        self.device.set_feature(bytes(buf.raw[:REPORT_LEN]))
        return 1

    def HidD_GetFeature(self, handle, buf, length):  # noqa: N802
        assert length == REPORT_LEN
        ctypes.memmove(buf, self.device.get_feature(), REPORT_LEN)
        return 1


class NoSleep:
    @staticmethod
    def sleep(_):
        pass


def load_fnmap():
    path = os.environ.get("FILCO_FNMAP")
    if not path:
        sys.exit("Set FILCO_FNMAP to the path of fnmap.py")
    spec = importlib.util.spec_from_file_location("fnmap", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def keyboard(fnmap, device):
    fnmap.hid = FakeHid(device)
    fnmap.time = NoSleep
    kb = object.__new__(fnmap.Keyboard)   # skip __init__: it would enumerate real devices
    kb.h, kb.checksum = None, 0
    return kb


def pages(name):
    return [(PAGES / name / f"layer{i}.bin").read_bytes() for i in range(4)]


def main():
    fnmap = load_fnmap()
    OUT.mkdir(parents=True, exist_ok=True)
    scenarios = []

    # 1. Read all four layers of the current configuration.
    dev = FakeKeyboard(pages("2026-10-31_Layer4Snip"))
    kb = keyboard(fnmap, dev)
    got = [kb.read_page(l) for l in range(4)]
    assert got == pages("2026-10-31_Layer4Snip")
    scenarios.append(("read_layer4snip", "2026-10-31_Layer4Snip", None, dev.log))

    # 2. Write layer 4 (index 3) of Layer4Snip onto a keyboard holding Layer4Apps (adds P = Win+Shift+S),
    #    including fnmap's own read-back.
    dev = FakeKeyboard(pages("2026-10-31_Layer4Apps"))
    kb = keyboard(fnmap, dev)
    kb.write_page(3, pages("2026-10-31_Layer4Snip")[3])
    assert bytes(dev.flash[3]) == pages("2026-10-31_Layer4Snip")[3]
    scenarios.append(("write_layer3_snip_over_apps", "2026-10-31_Layer4Apps",
                      {"layer": 3, "page": "2026-10-31_Layer4Snip"}, dev.log))

    for name, device_pages, write, log in scenarios:
        doc = {
            "generator": "tools/golden/gen_fnmap_golden.py (fnmap.py Keyboard.read_page / write_page)",
            "device_pages": device_pages,
            "write": write,
            "reports": [{"dir": d, "hex": h} for d, h in log],
        }
        path = OUT / f"{name}.json"
        path.write_text(json.dumps(doc, indent=1) + "\n", encoding="utf-8", newline="\n")
        print(f"wrote {path} ({len(log)} reports)")


if __name__ == "__main__":
    main()
