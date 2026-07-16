# winget manifest

This directory mirrors the exact path layout used by the
[`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs)
community repository, so the `manifests/` folder here can be copied
straight into a fork of that repo at the same path.

- **Package identifier:** `ErwinLovecraft.DownloadHub` (permanent once
  Microsoft merges a manifest under it — renaming later requires their
  deprecation flow, so double-check before the first submission)
- **Current manifest version:** `1.0.3`, pointing at the
  [`v1.0.2` GitHub Release](https://github.com/erwin-lovecraft/downloadhub/releases/tag/v1.0.2)
  installer asset (`downloadhub_1.0.3_x64-setup.exe`). The tag name and the
  product version embedded in the installer don't match here — that's a
  pre-existing quirk of this repo's manual version-bump process, not a
  winget-specific issue. Always set `PackageVersion` to the version actually
  embedded in the installer (Control Panel/Apps will show that), not the git
  tag.
- Only the NSIS (`nullsoft`) installer is listed. The MSI build also exists
  in the release, but wasn't added here to keep the first submission simple
  — see "Adding the MSI too" below if you want it.

## Important: this does NOT remove the SmartScreen warning

Submitting to winget does not require or provide code signing. Users
installing via `winget install` will see the same "Windows protected your
PC" SmartScreen prompt as a manual download, because that check is based on
the installer's digital signature/reputation, not the distribution channel.
Removing the warning requires actually signing the installer (e.g. via
SignPath Foundation's free OSS program, or Azure Trusted Signing) — that's a
separate, already-discussed piece of work.

## Validating locally

The `winget` CLI can validate a manifest without submitting anything:

```
winget validate --manifest packaging/winget/manifests/e/ErwinLovecraft/DownloadHub/1.0.3
```

To actually test-install from the local manifest (this really installs the
app on your machine, so only do this if you want to):

```
winget install --manifest packaging/winget/manifests/e/ErwinLovecraft/DownloadHub/1.0.3
```

## Submitting to winget-pkgs (manual, one-time for this version)

1. Fork [`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs).
2. Copy this folder's `manifests/e/ErwinLovecraft/DownloadHub/1.0.3/` into
   your fork at the identical path.
3. Commit and open a PR against `microsoft/winget-pkgs` — their automated
   pipeline will validate the manifest and run the installer in a sandbox.
   Their bot may flag the installer as unsigned; that's expected and
   generally not a blocker for OSS submissions, just a lower-trust label
   until it's signed.

Alternatively, Microsoft's [`wingetcreate`](https://github.com/microsoft/winget-create)
tool can open the PR for you directly from this manifest folder:

```
wingetcreate submit --token <your-github-PAT> packaging/winget/manifests/e/ErwinLovecraft/DownloadHub/1.0.3
```

The PAT needs `public_repo` scope on your own GitHub account (it forks
`winget-pkgs` and opens the PR on your behalf) — generate it yourself in
GitHub Settings → Developer settings; this isn't something that should be
scripted into CI without your explicit review each time you choose to.

## Updating for a future release

For each new tagged release, regenerate this version's manifest folder
(`packaging/winget/manifests/e/ErwinLovecraft/DownloadHub/<version>/`) with:

- `PackageVersion` set to the version embedded in that release's installer
  (from `tauri.conf.json`/`package.json`/`Cargo.toml` at build time)
- `InstallerUrl` pointing at that release's `.exe` asset
- `InstallerSha256` — get it without downloading the file via:
  ```
  gh api repos/erwin-lovecraft/downloadhub/releases/tags/<tag> \
    --jq '.assets[] | select(.name | endswith(".exe")) | .digest'
  ```
  (strip the `sha256:` prefix, uppercase it)

Then submit that new version folder the same way as above — winget keeps
every version's manifest, it isn't a replace-in-place update.

## Adding the MSI too

If you'd rather also offer the MSI as an alternate installer, add a second
entry under `Installers:` in the `.installer.yaml` with
`InstallerType: wix`, its own `InstallerUrl`/`InstallerSha256`. This wasn't
included by default since Tauri's WiX-built MSI's exact silent-switch
behavior wasn't verified here — confirm `msiexec /i ... /quiet` works as
expected before publishing it.
