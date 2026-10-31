import { describe, expect, it } from "vitest";
import { allKeys as KEYS, hasKey, systemLang, translate } from "./i18n";

describe("translate", () => {
  it("fills placeholders in both languages", () => {
    expect(translate("vi", "save_n", { n: 3 })).toBe("Lưu 3 thay đổi");
    expect(translate("en", "save_n", { n: 3 })).toBe("Save 3 changes");
    expect(translate("en", "layer", {})).toBe("Layer {n}");
  });

  it("uses the same placeholders in Vietnamese and English", () => {
    const names = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const k of KEYS) expect(names(translate("vi", k))).toEqual(names(translate("en", k)));
  });

  it("never uses an em dash", () => {
    for (const lang of ["vi", "en"] as const) {
      for (const k of KEYS) expect(translate(lang, k)).not.toContain(String.fromCharCode(0x2014));
    }
  });

  it("knows its keys and the system language", () => {
    expect(hasKey("err_read_failed")).toBe(true);
    expect(hasKey("err_nope")).toBe(false);
    expect(systemLang(["vi-VN", "en-US"])).toBe("vi");
    expect(systemLang(["en-US"])).toBe("en");
  });
});
