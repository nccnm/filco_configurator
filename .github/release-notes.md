Unofficial layer and firmware editor for the FILCO Majestouch Xacro M10SP US. Windows 10/11 only.

**Experimental.** Reading, writing layers and flashing firmware have run successfully on one real keyboard, the author's, and have not been tried on other machines. Flashing firmware always carries some risk to the keyboard: use it at your own risk.

## Install

1. Download `{{NAME}}` and its `.sha256` file.
2. Compare the hash in PowerShell, the two strings must match:
   ```powershell
   (Get-FileHash '.\{{NAME}}' -Algorithm SHA256).Hash.ToLower()
   Get-Content '.\{{NAME}}.sha256'
   ```
3. Run the installer. It is **not code signed**, so Windows SmartScreen may warn: choose _More info_, then _Run anyway_, only when the hash matches.

Quit FILCO Assist before using the app.

## Notes

- Only the Majestouch Xacro M10SP US is supported.
- The installer includes the stock V72 firmware and updater of Diatec / FILCO. They are not covered by this project's MIT license.
- Not affiliated with or endorsed by Diatec / FILCO.

Full guide: [README](https://github.com/nccnm/filco_configurator#readme)
