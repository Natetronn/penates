# Runs the real binaries on Windows (CI): create an app, check its Start Menu shortcut, launch it
# through the shortcut, launch the manager, export/remove/import, and save screenshots to smoke-out\.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$out = 'smoke-out'; New-Item -ItemType Directory -Force $out | Out-Null
$env:PENATES_DESKTOP = (Resolve-Path 'target/debug/penates-desktop.exe').Path
$A = (Resolve-Path 'target/debug/penates.exe').Path
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
function Shot($name) {
  try {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save((Join-Path $out $name))
  } catch { "screenshot failed: $_" }
}

& $A create https://example.com --name Smoke --id smoke --icon src-tauri/icons/icon.png
if ($LASTEXITCODE) { throw 'create failed' }
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Penates\Smoke.lnk'
if (-not (Test-Path $lnk)) { throw "no shortcut at $lnk" }
$sc = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
"shortcut: $($sc.TargetPath) $($sc.Arguments) | icon: $($sc.IconLocation)"
if ($sc.Arguments -ne '--run smoke') { throw 'wrong shortcut arguments' }

Start-Process $lnk
Start-Sleep 20
$app = Get-CimInstance Win32_Process -Filter "Name='penates-desktop.exe'" | Where-Object { $_.CommandLine -like '*--run smoke*' }
if (-not $app) { throw 'app did not start from its shortcut' }
Shot 'app.png'

$mgr = Start-Process $env:PENATES_DESKTOP -PassThru
Start-Sleep 15
if ($mgr.HasExited) { throw 'manager exited' }
Shot 'manager.png'
Stop-Process -Id $mgr.Id -Force
$app | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Start-Sleep 2

& $A export smoke -o "$out/smoke.penates"; if ($LASTEXITCODE) { throw 'export failed' }
& $A remove smoke --purge; if ($LASTEXITCODE) { throw 'remove failed' }
if (Test-Path $lnk) { throw 'shortcut not removed' }
& $A import "$out/smoke.penates" --yes; if ($LASTEXITCODE) { throw 'import failed' }
if (-not (Test-Path $lnk)) { throw 'import did not recreate the shortcut' }

# an app with the toolbar: it starts, and the screenshot shows the toolbar above the site
& $A create https://example.com --name 'Smoke toolbar' --id smoke-toolbar --toolbar --no-install
if ($LASTEXITCODE) { throw 'create with --toolbar failed' }
$tb = Start-Process $env:PENATES_DESKTOP -ArgumentList '--run', 'smoke-toolbar' -PassThru
Start-Sleep 15
if ($tb.HasExited) { throw 'toolbar app exited' }
Shot 'toolbar.png'
Stop-Process -Id $tb.Id -Force
'smoke OK'
