# Installer test of the Windows CI job (IV-10, as FM-11): silent install with /PATH, Open with
# registration, start, the portable build with a file, silent update, silent uninstall; on the CI
# machine only. Fails with a clear message at the first thing that is wrong.
param(
  [Parameter(Mandatory = $true)][string]$Setup,
  [Parameter(Mandatory = $true)][string]$Portable
)

$ErrorActionPreference = "Stop"
$version = node -p "require('./package.json').version"
$dir = Join-Path $env:LOCALAPPDATA "Programs\Spiegelmond"
$exe = Join-Path $dir "iv.exe"
$uninstaller = Join-Path $dir "uninstall.exe"
$startFolder = Join-Path ([Environment]::GetFolderPath("Programs")) "Spiegelmond"
$startShortcut = Join-Path $startFolder "Spiegelmond.lnk"
$desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "Spiegelmond.lnk"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Spiegelmond"
$data = Join-Path $env:LOCALAPPDATA "Spiegelmond"
$settings = Join-Path $data "settings.json"
$journal = Join-Path $data "journal\journal.jsonl"
$cache = Join-Path $data "cache\thumbs\v1"
$progId = "HKCU:\Software\Classes\Spiegelmond.Image"
$application = "HKCU:\Software\Classes\Applications\iv.exe"
$registered = "HKCU:\Software\RegisteredApplications"

function Check([bool]$ok, [string]$what) {
  if (-not $ok) { Write-Error "Installer test failed: $what" }
  Write-Host "ok: $what"
}

function Install([string]$label, [string]$options) {
  $process = Start-Process -FilePath $Setup -ArgumentList "/S $options" -Wait -PassThru
  Check ($process.ExitCode -eq 0) "$label exits with 0 (got $($process.ExitCode))"
}

function UserPath() {
  return [Environment]::GetEnvironmentVariable("Path", "User")
}

function OpenWith([string]$ext) {
  $key = "HKCU:\Software\Classes\.$ext\OpenWithProgids"
  return (Test-Path $key) -and ((Get-Item $key).GetValueNames() -contains "Spiegelmond.Image")
}

function Uninstall() {
  # The NSIS uninstaller copies itself to TEMP and returns at once: wait for it.
  Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait
  $deadline = (Get-Date).AddSeconds(60)
  while (((Test-Path $exe) -or (Test-Path $uninstallKey)) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 1
  }
  Start-Sleep -Seconds 2
}

Check (-not (Test-Path $dir)) "nothing is installed before the test"
$pathBefore = UserPath

# 1. Install, with `iv` in PATH
Install "silent install with /PATH" "/PATH"
Check (Test-Path $exe) "iv.exe in $dir"
Check (Test-Path $uninstaller) "uninstall.exe next to it"
foreach ($dll in "heif.dll", "libde265.dll") { Check (Test-Path (Join-Path $dir $dll)) "$dll next to iv.exe (HEIC, IV-05)" }
Check (Test-Path (Join-Path $dir "licenses\heif\SOURCES.md")) "the HEIC libraries' licences and sources ship with the program"
Check (Test-Path $startShortcut) "Start shortcut $startShortcut"
Check (@(Get-ChildItem $startFolder -Filter *.lnk).Count -eq 2) "Start folder holds Spiegelmond and its uninstall shortcut"
Check (Test-Path $desktopShortcut) "desktop shortcut"
$entry = Get-ItemProperty $uninstallKey
Check ($entry.DisplayVersion -eq $version) "Apps & features entry with version $version"
Check ($entry.DisplayName -eq "Spiegelmond") "Apps & features entry named Spiegelmond"
foreach ($ext in "jpg", "png", "heic", "tif", "cr3", "nef") { Check (OpenWith $ext) "Open with offers Spiegelmond for .$ext" }
Check ((Get-ItemProperty "$progId\shell\open\command")."(default)" -like "*iv.exe*%1*") "Spiegelmond.Image opens with iv.exe"
Check (Test-Path "$application\SupportedTypes") "Applications\iv.exe lists its types"
Check ((Get-ItemProperty $registered).Spiegelmond -like "*Spiegelmond\Capabilities") "Default apps knows Spiegelmond"
Check ((UserPath) -split ";" -contains $dir) "the program folder is in the user PATH"

# 2. The installed program starts and keeps running; so does the portable build with a file
$app = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 8
Check (-not $app.HasExited) "the installed program starts and keeps running"
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2
$portableDir = Join-Path $env:RUNNER_TEMP "spiegelmond-portable"
New-Item -ItemType Directory -Force $portableDir | Out-Null
Copy-Item $Portable (Join-Path $portableDir "iv.exe")
$photo = Join-Path $portableDir "red.png"
Copy-Item "testdata/red.png" $photo
$app = Start-Process -FilePath (Join-Path $portableDir "iv.exe") -ArgumentList "`"$photo`"" -PassThru
Start-Sleep -Seconds 8
Check (-not $app.HasExited) "the portable build starts with a file and keeps running"
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2
Check ((Get-FileHash $photo).Hash -eq (Get-FileHash "testdata/red.png").Hash) "opening does not change the file"

# 3. Update over the installed version keeps the settings
New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
Set-Content -Path $settings -Value '{"test":"kept"}'
New-Item -ItemType Directory -Force (Split-Path $journal) | Out-Null
Set-Content -Path $journal -Value '{"journal":"kept"}'
New-Item -ItemType Directory -Force $cache | Out-Null
Set-Content -Path (Join-Path $cache "0000000000000000.jpg") -Value "thumbnail"
Install "silent update over the installed version" ""
Check (Test-Path $exe) "iv.exe after the update"
Check ((Get-Content $settings) -eq '{"test":"kept"}') "settings survive the update"
Check (Test-Path $cache) "the thumbnail cache survives the update"
Check (OpenWith "jpg") "Open with survives the update"

# 4. Uninstall
Uninstall
Check (-not (Test-Path $dir)) "the program folder is gone"
Check (-not (Test-Path $uninstallKey)) "the Apps & features entry is gone"
Check (-not (Test-Path $startFolder)) "the Start folder is gone"
Check (-not (Test-Path $desktopShortcut)) "the desktop shortcut is gone"
foreach ($ext in "jpg", "png", "heic", "tif", "cr3", "nef") { Check (-not (OpenWith $ext)) "Open with no longer offers Spiegelmond for .$ext" }
Check (-not (Test-Path $progId)) "Spiegelmond.Image is gone"
Check (-not (Test-Path $application)) "Applications\iv.exe is gone"
Check ($null -eq (Get-ItemProperty $registered -ErrorAction SilentlyContinue).Spiegelmond) "Default apps no longer lists Spiegelmond"
Check (-not ((UserPath) -split ";" -contains $dir)) "the program folder is no longer in the user PATH"
Check ((UserPath) -eq $pathBefore) "the user PATH is as it was before the install"
Check (-not (Test-Path (Join-Path $data "cache"))) "the thumbnail cache is gone"
Check (Test-Path $settings) "settings are kept by default (only the checkbox deletes them)"
Check (Test-Path $journal) "the journal of metadata changes is kept"

# 5. Installing again after the uninstall works from scratch
Install "silent install after the uninstall" ""
Check (Test-Path $exe) "iv.exe after installing again"
Check (-not ((UserPath) -split ";" -contains $dir)) "without /PATH nothing is added to PATH"
Uninstall
Remove-Item -Recurse -Force $data
Write-Host "Installer test passed."
