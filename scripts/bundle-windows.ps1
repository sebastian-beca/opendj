# Packages a compiled openDJ for Windows as dist/opendj-VERSION-windows-x64.zip
# (portable: unzip anywhere and run openDJ.exe) and
# dist/openDJ-VERSION-windows-x64-setup.exe (per-user install, no admin).
# Usage: scripts/bundle-windows.ps1 BINARY VERSION
# Needs Inno Setup 6 (installed with Chocolatey when missing). When
# SIGN_COMMAND is set, it runs with the file to sign appended, for
# openDJ.exe and then for the setup.
param(
  [Parameter(Mandatory)] [string] $Binary,
  [Parameter(Mandatory)] [string] $Version
)
$ErrorActionPreference = "Stop"

function Sign($file) {
  if ($env:SIGN_COMMAND) {
    Invoke-Expression "$env:SIGN_COMMAND `"$file`""
    if ($LASTEXITCODE) { throw "signing $file failed" }
  }
}

New-Item -ItemType Directory -Force dist | Out-Null
$stage = "dist/openDJ"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory $stage | Out-Null
Copy-Item $Binary "$stage/openDJ.exe"
Copy-Item -Recurse res/controllers "$stage/controllers"
Copy-Item README.md, NOTICE "$stage/"
Copy-Item LICENSE "$stage/LICENSE-GPL2.txt"
# The ASIO SDK built in is GPLv3, so the Windows build as a whole is too.
Copy-Item res/windows/LICENSE-GPL3.txt "$stage/"
Sign "$stage/openDJ.exe"

$zip = "dist/opendj-$Version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip

$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) {
  choco install innosetup -y --no-progress | Out-Null
}
$source = (Resolve-Path $stage).Path
$output = (Resolve-Path dist).Path
& $iscc /Q "/DVersion=$Version" "/DSource=$source" "/DOutput=$output" res/windows/opendj.iss
if ($LASTEXITCODE) { throw "Inno Setup failed" }
Sign "dist/openDJ-$Version-windows-x64-setup.exe"
Remove-Item -Recurse -Force $stage
