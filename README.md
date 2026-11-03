# filco keyboard keymap

[![CI](https://github.com/nccnm/filco_configurator/actions/workflows/ci.yml/badge.svg)](https://github.com/nccnm/filco_configurator/actions/workflows/ci.yml)

English | [Tiếng Việt](README.vi.md)

An **unofficial**, VIA-like layer editor for FILCO keyboards configured by FILCO Assist: pick a layer, click a key on the
picture, choose an action (key, modifier combination, media, mouse, system command), review the differences, and write
them to the keyboard. Built with Rust, Tauri 2 and Svelte 5, for Windows 10/11. Not affiliated with or endorsed by
Diatec / FILCO.

Supported model: **Majestouch Xacro M10SP US** (VID 0x2EFD, PID 0x1972), at the `experimental` level: reading, writing
layers and flashing firmware have run successfully on one real keyboard, the author's, and have not been tried on other
machines.

**[Download the installer](https://github.com/nccnm/filco_configurator/releases)**

## Install

1. Download `filco.keyboard.keymap_<version>_x64-setup.exe` and the `.sha256` file next to it from
   [GitHub Releases](https://github.com/nccnm/filco_configurator/releases).
2. Compare the hash in PowerShell, the two strings must match:

   ```powershell
   (Get-FileHash '.\filco.keyboard.keymap_0.1.0_x64-setup.exe' -Algorithm SHA256).Hash.ToLower()
   Get-Content '.\filco.keyboard.keymap_0.1.0_x64-setup.exe.sha256'
   ```

3. Run the installer. It is **not code signed**, so Windows SmartScreen may show "Windows protected your PC": choose
   *More info*, then *Run anyway* (only when the hash matches). It installs for the current user, without admin rights,
   into `%LOCALAPPDATA%\filco keyboard keymap`; uninstall it from *Settings > Apps*.

Needs the WebView2 Runtime (included in Windows 11; the installer downloads it if missing).

## Use

1. **Quit FILCO Assist** (right-click its tray icon, *Exit*). FILCO Assist uses the same configuration port and can
   overwrite layers; while it runs, the app neither reads nor writes.
2. Open the app: it finds the keyboard and reads the 4 layers (about 0.75 seconds). Reading changes nothing on the
   keyboard.
3. Choose a layer (tabs or keys 1-4), click a key on the picture (or move with the arrow keys), and choose an action in
   the panel below. "Apply to" assigns the same action to several layers at once. After an assignment the app selects
   the next key: to the right along the row, then the start of the row below, and from the last row back to the top
   row. Resetting to default, setting tap-hold or a refused assignment stays on the same key. Delete resets the slot to
   default. Ctrl+Z / Ctrl+Y undo and redo.
   The panel opens the right tab for what the key holds (Key, Modifier, Tap-hold, Media, Mouse, System, Advanced), turns
   on the modifiers that go with it, and scrolls so that the current button is in the middle; focus stays on the keyboard
   picture. The Firmware page does the same with its code picker.
4. Press **Save N changes** (Ctrl+S): the app reads the keyboard again, compares it with how it was when editing began,
   makes a backup, then shows a slot-by-slot table of differences (layer, key, now, will be, bytes). Nothing is written
   until you press **Write to keyboard**; the Cancel button has focus by default.
5. Each written layer is read back and compared across all 512 bytes. On an error part way, the app returns every
   written layer to the backup and reports the state of each layer.

On the keyboard picture: a border in the layer colour is an assigned slot (not empty), a coloured dot in the corner is an
unwritten change, faint text is an empty slot (the key follows the firmware's default keymap), and a dashed border is a
slot type the app cannot edit yet (macros and so on), which is kept as it is.

### Backup, restore, export and import

- Before each write an automatic backup goes to `%LOCALAPPDATA%\FilcoConfigurator\backups\<model>\<time>\`, in the same
  folder format as `fnmap.py` (`layer0.bin`...`layer3.bin` and a `manifest.json` with the SHA-256 of each layer).
- The **More** menu: back up now, restore from a backup (loaded into the editor; writing still goes through the
  differences table), open the backup folder, open another backup folder (for example one from `fnmap.py`), import or
  export a `.filco-layers.json` file, copy a whole layer to another layer, light or dark theme, Vietnamese or English.

### Safety

- The app never flashes firmware by itself and sends no command other than page read (0x10), firmware info block read
  (0x30), page write (0x11) and finish (0x02). The transport layer blocks in code every write packet outside a confirmed
  write flow. Firmware is flashed only through the official updater, opened when you press the button, and you press
  Start yourself.
- If the app or the computer stops during a write, the next launch reads the keyboard, compares it with the backup and
  the intended content, and offers to restore.
- A layer tied to a FILCO Assist profile (read from `%APPDATA%\FILCO\filco_assist_config.json`, read-only) gets a warning:
  saving that profile in FILCO Assist rebuilds the whole layer and erases changes made here.
- While Fn is held the firmware ignores layer slots; the Fn layer is fixed in the firmware and the Layer pages cannot
  change it.

### Tap-hold per layer

The **Tap-hold** tab on a Layer page sets, for the selected key: the key sent on tap, what is sent on hold (a key or a
modifier, or hold to enter layer 1-4) and the decision time (200 ms by default). Any key can have it, and the same key can
have its own tap-hold on each layer. Holding past the time, or pressing another key while holding, counts as a hold.
Fn + key still gives the key's Fn function (Fn + ; is still Home). One key can be holding as a key and one key can be
holding as a layer at a time; a second tap-hold key of the same kind pressed meanwhile acts as its default key. The slot
is stored as `08 tap hold time` (time x 10 ms, 0 = 200 ms; hold `f0`-`f3` = layer 1-4).

The stock firmware reads a type 8 slot as empty, so tap-hold needs a firmware built with per-layer tap-hold (TOML
`layer_tap_hold = true`). The app takes care of this: when layers have tap-hold and the keyboard does not run such a
firmware yet, **Save** first builds and tests one (the Firmware page draft plus per-layer tap-hold), the differences table
reports the CheckSum to be flashed, and after writing the layers the app makes a backup and opens the updater. You only
check the CheckSum and press Start; when the keyboard reconnects the app checks the firmware and the layers. Flashing does
not touch the layer pages. If you stop half way, the Save button becomes **Update firmware for tap-hold** until the flash
is done.

On a keyboard running the older kind of tap-hold (fixed in the firmware, next section): the first time you set tap-hold on
a key, or edit an older tap-hold key, the app moves those tap-holds into the slots of all 4 layers in the same undo step,
so they keep working after the flash.

### Older tap-hold and configuration in the firmware

If the keyboard runs a custom firmware with an info block (HID command 0x30), the app reads and **displays** those changes:
a tap-hold key shows its hold action (small, at the top) and its tap action (main label), a "hold to enter layer" key is
highlighted on that layer, and the inspector shows the tap-hold details and the Fn + key changes. Editing an older
tap-hold key on a Layer page converts it to per-layer tap-hold (section above). The stock firmware (or an older patch
without the block) answers NACK and the app just does not show this part.

### Firmware page: edit, build, test, flash

The **Firmware** tab (after Layer 4) edits what lives in the firmware: Fn + key and the default keymap. Tap-hold is not set
here but on the Layer pages (above); the draft keeps the keyboard's older tap-hold, if any, and creates no new older
tap-hold. Design: `docs/FIRMWARE_UI_DESIGN.md` (Vietnamese).

1. The keyboard picture shows Fn + key: the Fn function is the large label, the key name is small on top, and a key with
   no Fn function stays blank. Mouse clicks, mouse movement, scrolling and volume are drawn as icons, like the manual.
   Select a key, then edit in the panel below (Fn + key, Default). Each change is trial-patched at once onto the stock V72
   firmware; a change the firmware does not allow (out of the 39 Fn entries, the Fn key, W/R/X/C depending on the layout...)
   is refused with a reason.
2. The draft saves itself (`%LOCALAPPDATA%\FilcoConfigurator\firmware_draft.toml`); Ctrl+Z / Ctrl+Y, and TOML export and
   import in the **More** menu. The draft starts from the configuration running on the keyboard (read from the info
   block), or stock V72 if there is none.
3. **Build and test**: give it a name and build (3-4 seconds). The firmware runs in an emulator (each key compared with the
   stock one, Fn, layer switching, tap-hold, command 0x30, stack depth); the updater is made only when every test passes.
   Results go to `%LOCALAPPDATA%\FilcoConfigurator\firmware\<name>\`. The info block is always added.
4. **Flash to the keyboard...**: the app backs up the 4 layers, checks that FILCO Assist has quit, then opens the official
   updater. Check that the `CheckSum` line is the number the app shows, then press Start yourself. The keyboard restarts;
   the app reads the info block, confirms it is the build you just made, and reports any layer changed since before the
   flash.

To build without the UI (offline, without opening the keyboard or running the updater):

```powershell
cargo run --release -p filco-cli -- firmware-build <config.toml> [--stock firmware] [--out out]
```

The configuration is a TOML file (examples in `tests/fixtures/firmware/*/config.toml`). Results go to `out/<name>/`; the
updater is built only when every emulator test passes, and you run it yourself, check that the `CheckSum` line matches the
printed tag, and only then press Start. Details: `docs/DEFERRED_FIRMWARE_BUILD.md` (Vietnamese).

The installer includes the stock V72 firmware and the official updater (the `firmware/` folder, see `firmware/README.md`)
so that flashing works on another machine with nothing else to install.

## Adding a model

A model description is data, not code: one JSON file in `models/`, checked against `models/schema/model.schema.json`
(generated from the Rust types) and by the semantic checks in `filco-core`.

- `match`: VID, list of PIDs, HID caps of the configuration port (FILCO Assist filters on usage page 1, input 9, output 2,
  feature 65).
- `protocol`: protocol variant (`costar-v1`), packet length, how slots sit in a page (`costar-interleaved-8`), read and
  write delays. A new model uses 100 ms for both until measured on real hardware.
- `layers`: label and LED colour of each layer.
- `slot_types`: slot types that can be edited (0-4) and those only displayed.
- `layout.keys`: each key with `id`, matrix index `index`, position `x`/`y` and size `w`/`h` in key units, printed `legend`,
  `default_usage` (the standard HID code of the legend), `tags` (`m-key`, `unverified`), `note`.
- `support`: `read_only`, `experimental` (writing allowed with a warning) or `verified`.

Check one file: `cargo run -p filco-cli -- validate-model models\my-model.json`. Add the file to the `builtin::models()`
list in `crates/filco-core/src/model.rs`, then run `pwsh scripts/check.ps1`. The M10SP US model is generated by
`tools/gen_m10sp_model.py` to keep the key rows readable; writing JSON by hand works too.

## Build from source

### With Visual Studio (the usual way)

Needs: Visual Studio 2022 Build Tools with the *Desktop development with C++* workload (MSVC and the Windows SDK), Rust
through rustup (version pinned in `rust-toolchain.toml`), Node.js 22 or newer.

```powershell
npm ci
npx tauri build            # installer: target\release\bundle\nsis\
pwsh scripts/check.ps1     # format, lint, types, all tests
```

Or `pwsh scripts/release.ps1 [-InstallTest]`: builds the installer, checks its size, writes the `.sha256` file, and tries a
silent install and uninstall.

### Without admin rights and without Visual Studio

Nothing changes in the repository; only the machine is configured, in `%USERPROFILE%\.cargo\config.toml` (see
`docs/DECISIONS.md` D1, Vietnamese):

1. Rust: run `rustup-init.exe` (host `x86_64-pc-windows-msvc`).
2. Microsoft's CRT and Windows SDK with [`xwin`](https://github.com/Jake-Shadle/xwin):
   `xwin --accept-license splat --output %LOCALAPPDATA%\xwin-msvc\splat --disable-symlinks
   --preserve-ms-arch-notation` (this accepts Microsoft's license). `cargo-xwin` also does this on a machine that allows
   symlinks (Developer Mode on); where it does not, use `xwin` directly as above.
3. A portable LLVM (`clang-cl`, `llvm-rc`, `llvm-lib` are needed) for Tauri's build script.
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

Once the Build Tools are installed normally, delete these entries.

## Testing

| Command | What it does |
|---|---|
| `pwsh scripts/check.ps1` | text rules, `cargo fmt`, `clippy -D warnings` (also the `e2e-sim` build), all Rust tests (the codec exhaustively over 2^32 slots in release mode), `cargo deny` (licenses, sources), prettier, eslint, svelte-check, vitest |
| `pwsh scripts/e2e.ps1 [-Runs 3] [-Perf] [-Screenshots]` | builds the `e2e-sim` version (a simulated keyboard behind the real IPC), then runs the E2E tests through WebDriver, compares baseline images, measures performance, captures every screen |
| `cargo bench -p filco-service` / `-p filco-hid-windows` | cost of reading 4 layers, HID enumeration |
| `pwsh scripts/release.ps1 -InstallTest` | installer, size budget, SHA-256, install and uninstall |

`check.ps1` needs `cargo-deny` (`cargo install cargo-deny --locked`). E2E needs `tauri-driver`
(`cargo install tauri-driver --locked`) and an `msedgedriver` of the same major version as the machine's WebView2 Runtime
(set its path in the `MSEDGEDRIVER` variable, or unpack it into `%LOCALAPPDATA%\kle-tools\edgedriver-<version>`).

GitHub Actions runs `check.ps1` and `release.ps1 -InstallTest` on each pull request and push to `main`. It does not run E2E,
since the baseline images depend on the machine (DPI, font rendering): run `scripts/e2e.ps1` locally.

## Releasing

Set the same version in `src-tauri/tauri.conf.json`, `package.json` and `Cargo.toml`, merge to `main`, then push a matching
tag, for example `git tag v0.2.0 && git push origin v0.2.0`. The `Release` workflow checks the code, builds the installer
and publishes it with its `.sha256` file as a pre-release.

## Layout

| Folder | Content |
|---|---|
| `crates/filco-core` | slots, pages, diff, model description and schema, `fnmap.py`-style backups, export files |
| `crates/filco-protocol` | feature report packets, checksum, page read and write sessions, write blocking (`WritePolicy`) |
| `crates/filco-sim` | simulated keyboard following the protocol, fault injection, info block for 0x30 |
| `crates/filco-fwinfo` | decodes the firmware info block (command 0x30), read-only |
| `crates/filco-hid-windows` | real HID on Windows (the only place with `unsafe`) |
| `crates/filco-service` | edit session with undo, safe write flow, backup store, journal, FILCO Assist blocking |
| `crates/filco-cli` | developer command line tool: reads the keyboard only (`list`, `caps`, `read`, `decode`, `validate-model`); `firmware-build` builds and tests firmware offline |
| `crates/thumb` | Thumb ARMv6-M (Cortex-M0) assembler and decoder for the code inserted into the firmware |
| `crates/filco-firmware` | builds custom V72 images from the stock one: HEX, Fn and keymap tables, tap-hold code, info block and command 0x30 |
| `crates/m0emu` | pure Rust Cortex-M0 interpreter, stops at instructions it does not model |
| `crates/filco-fwtest` | runs the real firmware in `m0emu`, compares each key with stock, tap-hold, 0x30 over a fake HID, stack depth |
| `crates/filco-updater` | builds an updater from the official one (ZIP in RT_RCDATA resource 4011) |
| `crates/filco-fwbuild` | the build flow: image, HEX, tests, and the updater only when every test passes |
| `src-tauri` | the app: state, thin IPC commands, IPC types generated into TypeScript |
| `ui` | Svelte interface |
| `models` | JSON model descriptions and schema |
| `firmware` | stock V72 firmware and official updater, included in the installer |
| `tests` | fixtures (the author's own layer pages, sources noted, golden firmware), golden bytes from `fnmap.py`, E2E |
| `docs` | assignment, plan, decisions, status, screenshots (mostly Vietnamese) |

## License

The source code is under the MIT license (see `LICENSE`). The bundled IBM Plex fonts are under the SIL Open Font License 1.1.

The MIT license does not cover the stock V72 firmware and the official updater of Diatec / FILCO in `firmware/`, included in
the installer for flashing, nor the firmware builds made from them in `tests/fixtures/firmware/` (see `firmware/README.md`).
