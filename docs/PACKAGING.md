# Packaging & Distribution

StateLab ships in two forms. Both run the **same engine and the same UI build**;
they differ only in how the UI is hosted.

| | Desktop app (primary) | Browser host (alternative) |
|---|---|---|
| Artifact | `StateLab_0.1.0_x64_en-US.msi` / `StateLab_0.1.0_x64-setup.exe` | `StateLabServer.exe` |
| Shell | Tauri 2 + WebView2, native window | std-only loopback HTTP server + default browser |
| Crate | `src-tauri` | `crates/statelab-app` |
| IPC | `invoke()` / `Channel` | `fetch` / NDJSON |
| Requires | WebView2 runtime (preinstalled on Win 11) | nothing |

The frontend detects its host at runtime (`isTauri()` in
[`src/lib/invoke.ts`](../src/lib/invoke.ts)), so **one build runs under either**.

## Building the desktop app + installers

```bash
npm run tauri build
```

Produces, under `target/release/bundle/`:

- `msi/StateLab_0.1.0_x64_en-US.msi` (~2.9 MB) — WiX installer
- `nsis/StateLab_0.1.0_x64-setup.exe` (~2.0 MB) — NSIS installer

The unbundled executable is `target/release/statelab.exe` (~8.7 MB).

### Toolchain requirements

Tauri requires the **MSVC** toolchain — it cannot be built with the
self-contained `windows-gnu` toolchain, because many of its dependencies generate
import libraries via `raw-dylib`, which needs a full binutils (`as`, `dlltool`)
that the gnu toolchain does not ship.

```bash
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
```

plus **Visual Studio Build Tools 2022** with these two components:

```
Microsoft.VisualStudio.Component.VC.Tools.x86.x64
Microsoft.VisualStudio.Component.Windows11SDK.26100
```

Install unattended with:

```bash
vs_BuildTools.exe --passive --wait --norestart \
  --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 \
  --add Microsoft.VisualStudio.Component.Windows11SDK.26100
```

> **Gotchas we hit, recorded so nobody repeats them:**
> - Run the **bootstrapper** (`vs_BuildTools.exe`), which self-elevates. The
>   installed `setup.exe modify --passive` refuses to run un-elevated (exit
>   `5007`) and rejects `--wait` as an unknown option.
> - Avoid `--nocache`: our first SDK attempt failed with return code `1335`
>   ("cabinet file corrupt") and rolled the whole SDK back, leaving MSVC present
>   but unlinkable.
> - The SDK writes its libraries progressively. `kernel32.lib` appearing does not
>   mean it is done — Rust also needs `dbghelp.lib`, which lands much later.

## Building the browser host

```bash
bash scripts/package.sh
```

Builds `StateLabServer.exe` (statically linked, ~2 MB, no runtime dependencies)
and stages `dist-package/` with docs and a HOW-TO-RUN file. Zip it with:

```bash
powershell -Command "Compress-Archive -Path 'dist-package/*' -DestinationPath 'statelab-0.1.0.zip' -Force"
```

This host is useful where WebView2 is unavailable, or for quick portable use — no
installer, copy the single file anywhere.

## Icons

The icon set is generated from the real Collatz trajectory for n = 27:

```bash
node scripts/make-icon.mjs      # writes src-tauri/icons/source.png (1024x1024)
npx tauri icon src-tauri/icons/source.png
```

`make-icon.mjs` writes the PNG by hand using Node's built-in `zlib`, so no image
tooling is required.

## Binary naming

`crates/statelab-app` deliberately builds **`StateLabServer.exe`**, not
`StateLab.exe`: the Tauri shell produces `statelab.exe`, and Windows filenames are
case-insensitive, so identical names would make the two hosts silently overwrite
each other in `target/release`.
