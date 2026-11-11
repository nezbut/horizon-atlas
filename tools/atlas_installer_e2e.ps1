<#
.SYNOPSIS
Horizon Atlas's Windows installer, end to end, in a scratch profile (patch: own-install).

.DESCRIPTION
Runs the built launcher as the setup (horizon-launcher-setup.exe setup --quiet) and the installed copy's
uninstall --quiet, and checks what they do to an old Atlas install (Atlas 0.1.x installed under the official
launcher's names) and to the official launcher:

  1. An old Atlas in ...\Programs\Horizon Launcher, with the official launcher's keys and shortcuts naming it:
     setup without --remove-old-atlas leaves it; with the flag, setup installs Atlas into
     ...\Programs\Horizon Atlas Launcher with Atlas's own keys, and removes the old executable, its shortcuts
     and the old keys.
  2. The official launcher (an executable without Atlas's marker) in ...\Programs\Horizon Launcher, with its
     keys and shortcuts: setup --remove-old-atlas leaves all of it as it was.
  3. The official launcher's keys naming an official launcher installed elsewhere, beside an old Atlas left in
     the default folder: the old Atlas and its shortcut go; the keys, the official launcher and its shortcut
     stay.
  4. An old Atlas copy uninstalled through its own executable (what its old Apps entry runs), with
     --remove-data, beside a new install: the old copy, its shortcuts and the old keys go; the new install, its
     shortcut, its keys and Atlas's own atlas\ data folder stay.
  5. An old Atlas in a folder of the player's that only the official launcher's key names (nothing in the
     default folder): setup --remove-old-atlas finds it through the key and removes it.
  6. The last Atlas, an old copy, uninstalled through its own executable with --remove-data: the old copy, its
     shortcuts and the old keys go, and so does Atlas's own atlas\ data folder.
  7. In every scenario, the data folder both launchers share (%APPDATA%\horizon-launcher: a marker file, a
     server's world) stays after every setup and uninstall.
  8. In every scenario, uninstall --quiet of Atlas removes Atlas's folder, shortcut and keys, and leaves the
     official launcher's keys. With --remove-data (scenario 2) it also deletes Atlas's own atlas\ folder in the
     data folder, and nothing else there.

The launcher's processes get LOCALAPPDATA, APPDATA, USERPROFILE, TEMP, TMP, HORIZON_LAUNCHER_HOME and
HORIZON_LAUNCHER_DESKTOP inside a fresh temporary folder. The registry has no scratch copy: the script works on
the four HKCU keys of the two launchers, refuses to start when any of them exists already (a machine with either
launcher installed), and removes them when it ends. Uninstalling closes only launchers running from the folder it
removes, so no other launcher is touched.

Exits 0 when every check passes, 1 at the first failed check, 2 when it refuses to start.

.PARAMETER Launcher
The built launcher, for example launcher\target\release\horizon-launcher.exe.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Launcher
)

Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'

$OldOwnKey = 'HKCU:\Software\Horizon\Launcher'
$OldUninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HorizonLauncher'
$OwnKey = 'HKCU:\Software\Horizon\Atlas'
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HorizonAtlas'
$ParentKey = 'HKCU:\Software\Horizon'
# Atlas's name on Windows: its folder, its shortcuts and its Uninstall entry (installer.rs, APP_NAME).
$AtlasName = 'Horizon Atlas Launcher'
$Keys = @($OldOwnKey, $OldUninstallKey, $OwnKey, $UninstallKey)
$DataText = 'The data folder both launchers share: no installer touches it.'
# The official launcher's stand-in: a real program without Atlas's marker. A Start menu shortcut to a file that
# is not a real program can't be saved, and the official launcher is a real program too.
$OfficialStandIn = Join-Path $env:SystemRoot 'System32\whoami.exe'

function Fail([string]$message) {
    throw "FAILED: $message"
}

function Check([bool]$ok, [string]$what) {
    if (-not $ok) {
        Fail $what
    }
    Write-Host "  ok: $what"
}

function Get-KeyValue([string]$key, [string]$name) {
    if (-not (Test-Path $key)) {
        return $null
    }
    $property = (Get-ItemProperty -Path $key).PSObject.Properties[$name]
    if ($null -eq $property) {
        return $null
    }
    return [string]$property.Value
}

# Creates the key (and its missing parents) without ever recreating one that exists.
function New-Key([string]$key) {
    if (Test-Path $key) {
        return
    }
    New-Key (Split-Path $key -Parent)
    New-Item -Path $key | Out-Null
}

function Set-Key([string]$key, [hashtable]$values) {
    New-Key $key
    foreach ($name in $values.Keys) {
        New-ItemProperty -Path $key -Name $name -Value $values[$name] -PropertyType String -Force | Out-Null
    }
}

function Remove-Keys {
    foreach ($key in $Keys) {
        if (Test-Path $key) {
            Remove-Item -Path $key -Recurse -Force
        }
    }
}

# The keys the official launcher writes for the install in $dir (and Atlas 0.1.x wrote, under the same names).
function Register-Official([string]$dir) {
    $exe = Join-Path $dir 'horizon-launcher.exe'
    Set-Key $OldOwnKey @{ InstallDir = $dir; Version = '0.2.59' }
    Set-Key $OldUninstallKey @{
        DisplayName = 'Horizon Launcher'
        DisplayVersion = '0.2.59'
        InstallLocation = $dir
        UninstallString = "`"$exe`" uninstall"
    }
}

# Whether $file holds $text in UTF-16, as a shortcut holds its program's path.
function Test-Holds([string]$file, [string]$text) {
    if (-not (Test-Path -LiteralPath $file)) {
        return $false
    }
    $haystack = [BitConverter]::ToString([IO.File]::ReadAllBytes($file))
    $needle = [BitConverter]::ToString([Text.Encoding]::Unicode.GetBytes($text))
    return $haystack.Contains($needle)
}

# A shortcut to $exe as the launcher makes them (target, folder, icon): the installer knows a shortcut by the
# program's path inside it (installer.rs, points_to), so a fixture without it would test nothing.
function New-Link([string]$path, [string]$exe) {
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = $exe
    $link.WorkingDirectory = Split-Path $exe -Parent
    $link.IconLocation = "$exe,0"
    $link.Save()
    if (-not (Test-Holds $path $exe)) {
        Fail "fixture: the shortcut $path does not hold $exe in UTF-16"
    }
}

function Get-Hash([string]$file) {
    if (-not (Test-Path -LiteralPath $file)) {
        return 'missing'
    }
    return (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
}

# The official launcher's install as one text: its executable, its shortcuts and both its keys' values.
function Get-OfficialState([string]$exe, [string[]]$links) {
    $lines = @(
        "exe=$(Get-Hash $exe)",
        "InstallDir=$(Get-KeyValue $OldOwnKey 'InstallDir')",
        "Version=$(Get-KeyValue $OldOwnKey 'Version')",
        "DisplayName=$(Get-KeyValue $OldUninstallKey 'DisplayName')",
        "InstallLocation=$(Get-KeyValue $OldUninstallKey 'InstallLocation')",
        "UninstallString=$(Get-KeyValue $OldUninstallKey 'UninstallString')"
    )
    foreach ($link in $links) {
        $lines += "$link=$(Get-Hash $link)"
    }
    return $lines -join "`n"
}

# A fresh profile for one scenario under the scratch root, and no key of either launcher.
function New-Scratch([string]$name) {
    $root = Join-Path $ScratchRoot $name
    $local = Join-Path $root 'Local'
    $roaming = Join-Path $root 'Roaming'
    $programs = Join-Path $local 'Programs'
    $p = @{
        Root = $root
        Local = $local
        Roaming = $roaming
        Home = Join-Path $root 'Home'
        Temp = Join-Path $root 'Temp'
        Desktop = Join-Path $root 'Desktop'
        StartMenu = Join-Path $roaming 'Microsoft\Windows\Start Menu\Programs'
        Data = Join-Path $roaming 'horizon-launcher'
        Programs = $programs
        OldDir = Join-Path $programs 'Horizon Launcher'
        AtlasDir = Join-Path $programs $AtlasName
        Setup = Join-Path $root 'horizon-launcher-setup.exe'
    }
    $p.AtlasExe = Join-Path $p.AtlasDir 'horizon-launcher.exe'
    $p.OldExe = Join-Path $p.OldDir 'horizon-launcher.exe'
    foreach ($dir in @($p.Local, $p.Roaming, $p.Home, $p.Temp, $p.Desktop, $p.StartMenu, $p.Data, $p.Programs)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    # The setup is the launcher's own binary under the setup's name, as the release ships it.
    Copy-Item -LiteralPath $Launcher -Destination $p.Setup
    # The shared data folder: the official launcher's (a marker, a server's world) and Atlas's own atlas\.
    [IO.File]::WriteAllText((Join-Path $p.Data 'e2e-marker.txt'), $DataText)
    $world = Join-Path $p.Data 'servers\smp\world'
    New-Item -ItemType Directory -Force -Path $world | Out-Null
    [IO.File]::WriteAllText((Join-Path $world 'level.dat'), 'a world')
    New-Item -ItemType Directory -Force -Path (Join-Path $p.Data 'atlas') | Out-Null
    [IO.File]::WriteAllText((Join-Path $p.Data 'atlas\offline.json'), '{"names": []}')
    Remove-Keys
    return $p
}

# Atlas 0.1.x installed in $dir under the official launcher's names: an Atlas build (it holds Atlas's
# marker), what a self-update left beside it, the official launcher's keys naming $dir, and shortcuts to it
# (the Start menu's, the desktop's and an instance's). Returns the shortcuts.
function New-OldAtlas($p, [string]$dir) {
    $exe = Join-Path $dir 'horizon-launcher.exe'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Copy-Item -LiteralPath $Launcher -Destination $exe
    [IO.File]::WriteAllBytes((Join-Path $dir 'horizon-launcher.exe.old-1-0'), [byte[]](77, 90))
    Register-Official $dir
    $links = @(
        (Join-Path $p.StartMenu 'Horizon Launcher.lnk'),
        (Join-Path $p.Desktop 'Horizon Launcher.lnk'),
        (Join-Path $p.Desktop 'My world (Horizon).lnk')
    )
    foreach ($link in $links) {
        New-Link $link $exe
    }
    return $links
}

# Atlas's own install as one text: its executable, its Start menu shortcut and its keys' values.
function Get-AtlasState($p) {
    $link = Join-Path $p.StartMenu ($AtlasName + '.lnk')
    $lines = @(
        "exe=$(Get-Hash $p.AtlasExe)",
        "link=$(Get-Hash $link)",
        "InstallDir=$(Get-KeyValue $OwnKey 'InstallDir')",
        "Version=$(Get-KeyValue $OwnKey 'Version')",
        "DisplayName=$(Get-KeyValue $UninstallKey 'DisplayName')",
        "InstallLocation=$(Get-KeyValue $UninstallKey 'InstallLocation')",
        "UninstallString=$(Get-KeyValue $UninstallKey 'UninstallString')"
    )
    return $lines -join "`n"
}

# The old install in $dir and what named it are gone: the folder, its shortcuts and both old keys.
function Assert-OldAtlasGone([string]$dir, [string[]]$links) {
    Check (-not (Test-Path -LiteralPath $dir)) "the old Atlas folder $dir is gone, with its executable and what updates left"
    Check (-not (Test-Path $OldOwnKey)) 'HKCU\Software\Horizon\Launcher, which named it, is gone'
    Check (-not (Test-Path $OldUninstallKey)) 'the Uninstall entry HorizonLauncher, which named it, is gone'
    foreach ($link in $links) {
        Check (-not (Test-Path -LiteralPath $link)) "its shortcut $(Split-Path $link -Leaf) is gone"
    }
}

# Runs $exe with $arguments in the scratch profile $p and waits for it. Returns its exit code and output.
function Invoke-Launcher($p, [string]$exe, [string[]]$arguments) {
    $scratch = @{
        LOCALAPPDATA = $p.Local
        APPDATA = $p.Roaming
        USERPROFILE = $p.Home
        TEMP = $p.Temp
        TMP = $p.Temp
        HORIZON_LAUNCHER_HOME = $p.Data
        HORIZON_LAUNCHER_DESKTOP = $p.Desktop
        HORIZON_LAUNCHER_NO_SELF_UPDATE = '1'
        HORIZON_LAUNCHER_SETUP = $null
        HORIZON_LAUNCHER_SETUP_DIR = $null
    }
    $saved = @{}
    foreach ($name in $scratch.Keys) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $scratch[$name], 'Process')
    }
    try {
        $quoted = @(foreach ($argument in $arguments) {
                if ($argument -match '[\s"]') { '"' + $argument.Replace('"', '\"') + '"' } else { $argument }
            })
        $script:Runs++
        $out = Join-Path $p.Root "run-$($script:Runs)-stdout.txt"
        $err = Join-Path $p.Root "run-$($script:Runs)-stderr.txt"
        $process = Start-Process -FilePath $exe -ArgumentList $quoted -WorkingDirectory $p.Root -NoNewWindow `
            -Wait -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
        $result = [pscustomobject]@{
            Code = $process.ExitCode
            Out = [IO.File]::ReadAllText($out).Trim()
            Err = [IO.File]::ReadAllText($err).Trim()
        }
        Write-Host "  ran: $(Split-Path $exe -Leaf) $($arguments -join ' ') -> exit $($result.Code)"
        if ($result.Err) {
            Write-Host "  stderr: $($result.Err)"
        }
        return $result
    }
    finally {
        foreach ($name in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
        }
    }
}

function Invoke-Setup($p, [string[]]$extra) {
    $r = Invoke-Launcher $p $p.Setup (@('setup', '--quiet', '--dir', $p.Programs) + $extra)
    Check ($r.Code -eq 0) "setup --quiet $($extra -join ' ') exits 0"
    Check (-not $r.Err.Contains('Not removed')) 'setup removed all it meant to'
    return $r
}

function Assert-AtlasInstalled($p, $r) {
    Check (Test-Path -LiteralPath $p.AtlasExe) "Atlas is installed in its own folder, $($p.AtlasDir)"
    Check ($r.Out -eq $p.AtlasExe) 'setup names the installed launcher'
    Check ((Get-KeyValue $OwnKey 'InstallDir') -eq $p.AtlasDir) 'HKCU\Software\Horizon\Atlas names Atlas''s folder'
    Check ((Get-KeyValue $UninstallKey 'DisplayName') -eq $AtlasName) "the Uninstall entry HorizonAtlas is $AtlasName"
    Check ((Get-KeyValue $UninstallKey 'InstallLocation') -eq $p.AtlasDir) 'the Uninstall entry names Atlas''s folder'
    Check (Test-Holds (Join-Path $p.StartMenu "$AtlasName.lnk") $p.AtlasExe) "the Start menu shortcut $AtlasName starts it"
}

# The shared data folder keeps what the official launcher has there; Atlas's own atlas\ too unless $atlasGone.
function Assert-DataKept($p, [bool]$atlasGone = $false) {
    $marker = Join-Path $p.Data 'e2e-marker.txt'
    $kept = (Test-Path -LiteralPath $marker) -and ([IO.File]::ReadAllText($marker) -eq $DataText)
    $world = Test-Path -LiteralPath (Join-Path $p.Data 'servers\smp\world\level.dat')
    Check ($kept -and $world) 'the shared data folder %APPDATA%\horizon-launcher is untouched'
    $atlas = Test-Path -LiteralPath (Join-Path $p.Data 'atlas\offline.json')
    if ($atlasGone) {
        Check (-not (Test-Path -LiteralPath (Join-Path $p.Data 'atlas'))) 'Atlas''s own atlas\ folder in it is deleted'
    }
    else {
        Check $atlas 'Atlas''s own atlas\ folder in it stays'
    }
}

# Uninstalls Atlas with its installed copy (with --remove-data when $removeData): Atlas's install goes, the
# official launcher's keys stay as they were.
function Assert-UninstallKeepsOfficial($p, [string]$officialBefore, [string]$officialExe, [string[]]$officialLinks, [bool]$removeData = $false) {
    $arguments = @('uninstall', '--quiet')
    if ($removeData) {
        $arguments += '--remove-data'
    }
    $r = Invoke-Launcher $p $p.AtlasExe $arguments
    Check ($r.Code -eq 0) "$($arguments -join ' ') exits 0"
    Check (-not $r.Err.Contains('Not removed')) 'uninstall removed all it meant to'
    Check (-not (Test-Path -LiteralPath $p.AtlasDir)) 'uninstall removes Atlas''s folder'
    Check (-not (Test-Path $OwnKey)) 'uninstall removes HKCU\Software\Horizon\Atlas'
    Check (-not (Test-Path $UninstallKey)) 'uninstall removes the Uninstall entry HorizonAtlas'
    Check (-not (Test-Path -LiteralPath (Join-Path $p.StartMenu "$AtlasName.lnk"))) "uninstall removes the Start menu shortcut $AtlasName"
    if ($officialBefore) {
        Check ((Get-OfficialState $officialExe $officialLinks) -eq $officialBefore) 'uninstall leaves the official launcher, its shortcuts and its keys as they were'
    }
    Assert-DataKept $p $removeData
}

$Launcher = (Resolve-Path -LiteralPath $Launcher).Path
$present = @($Keys | Where-Object { Test-Path $_ })
if ($present.Count -gt 0) {
    Write-Host "error: these keys exist already: $($present -join ', ')."
    Write-Host 'The script removes them when it ends: run it where neither launcher is installed for this account (CI).'
    exit 2
}
$parentExisted = Test-Path $ParentKey
$base = [Environment]::GetEnvironmentVariable('RUNNER_TEMP')
if (-not $base) {
    $base = [IO.Path]::GetTempPath()
}
$ScratchRoot = Join-Path $base ('atlas-installer-e2e-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
$Runs = 0
$failed = $false

try {
    Write-Host '1. An old Atlas in the official launcher''s folder, with the old keys and shortcuts'
    $p = New-Scratch 's1'
    $oldLinks = New-OldAtlas $p $p.OldDir
    $other = Join-Path $p.Root 'other.exe'
    [IO.File]::WriteAllBytes($other, [byte[]](77, 90))
    $otherLink = Join-Path $p.Desktop 'Other program.lnk'
    New-Link $otherLink $other

    $r = Invoke-Setup $p @()
    Assert-AtlasInstalled $p $r
    Check (Test-Path -LiteralPath $p.OldExe) 'without --remove-old-atlas the old Atlas stays'
    Check ((Get-KeyValue $OldOwnKey 'InstallDir') -eq $p.OldDir) 'without --remove-old-atlas the old keys stay'

    $r = Invoke-Setup $p @('--remove-old-atlas')
    Assert-AtlasInstalled $p $r
    Assert-OldAtlasGone $p.OldDir $oldLinks
    Check (Test-Path -LiteralPath $otherLink) 'a shortcut to another program stays'
    Check (Test-Path $ParentKey) 'HKCU\Software\Horizon stays (Atlas''s key is in it)'
    Assert-DataKept $p
    Assert-UninstallKeepsOfficial $p '' '' @()

    Write-Host '2. The official launcher in its folder, with its keys and shortcuts'
    $p = New-Scratch 's2'
    New-Item -ItemType Directory -Force -Path $p.OldDir | Out-Null
    Copy-Item -LiteralPath $OfficialStandIn -Destination $p.OldExe
    Register-Official $p.OldDir
    $officialLinks = @((Join-Path $p.StartMenu 'Horizon Launcher.lnk'), (Join-Path $p.Desktop 'Horizon Launcher.lnk'))
    foreach ($link in $officialLinks) {
        New-Link $link $p.OldExe
    }
    $before = Get-OfficialState $p.OldExe $officialLinks

    $r = Invoke-Setup $p @('--remove-old-atlas')
    Assert-AtlasInstalled $p $r
    Check ((Get-OfficialState $p.OldExe $officialLinks) -eq $before) 'the official launcher, its shortcuts and its keys are as they were'
    Assert-DataKept $p
    # "Also delete" takes Atlas's own atlas\ only: the official launcher's data and keys stay.
    Assert-UninstallKeepsOfficial $p $before $p.OldExe $officialLinks $true

    Write-Host '3. The official keys naming an official launcher elsewhere, beside an old Atlas in the default folder'
    $p = New-Scratch 's3'
    $elsewhere = Join-Path $p.Root 'Games\Horizon Launcher'
    $elsewhereExe = Join-Path $elsewhere 'horizon-launcher.exe'
    New-Item -ItemType Directory -Force -Path $elsewhere | Out-Null
    Copy-Item -LiteralPath $OfficialStandIn -Destination $elsewhereExe
    Register-Official $elsewhere
    $officialLinks = @(Join-Path $p.StartMenu 'Horizon Launcher.lnk')
    New-Link $officialLinks[0] $elsewhereExe
    New-Item -ItemType Directory -Force -Path $p.OldDir | Out-Null
    Copy-Item -LiteralPath $Launcher -Destination $p.OldExe
    $oldLink = Join-Path $p.Desktop 'Horizon Launcher.lnk'
    New-Link $oldLink $p.OldExe
    $before = Get-OfficialState $elsewhereExe $officialLinks

    $r = Invoke-Setup $p @('--remove-old-atlas')
    Assert-AtlasInstalled $p $r
    Check (-not (Test-Path -LiteralPath $p.OldDir)) 'the old Atlas in the default folder is gone'
    Check (-not (Test-Path -LiteralPath $oldLink)) 'its shortcut is gone'
    Check ((Get-OfficialState $elsewhereExe $officialLinks) -eq $before) 'the official launcher elsewhere, its shortcut and the keys naming it are as they were'
    Assert-DataKept $p
    Assert-UninstallKeepsOfficial $p $before $elsewhereExe $officialLinks

    Write-Host '4. An old Atlas copy uninstalled through its own executable, beside a new install'
    $p = New-Scratch 's4'
    $oldLinks = New-OldAtlas $p $p.OldDir
    $r = Invoke-Setup $p @()
    Assert-AtlasInstalled $p $r
    $atlasBefore = Get-AtlasState $p
    # What the old Apps entry "Horizon Launcher" runs (its UninstallString), quietly. "Also delete" is
    # ignored here: the data is the new install's now.
    $r = Invoke-Launcher $p $p.OldExe @('uninstall', '--quiet', '--remove-data')
    Check ($r.Code -eq 0) 'the old copy''s uninstall --quiet --remove-data exits 0'
    Check (-not $r.Err.Contains('Not removed')) 'the old copy''s uninstall removed all it meant to'
    Assert-OldAtlasGone $p.OldDir $oldLinks
    Check ((Get-AtlasState $p) -eq $atlasBefore) 'the new install, its shortcut and its keys are as they were'
    Assert-DataKept $p
    Assert-UninstallKeepsOfficial $p '' '' @()

    Write-Host '5. An old Atlas only the official launcher''s key names, in a folder of the player''s'
    $p = New-Scratch 's5'
    $custom = Join-Path $p.Root 'Games\Horizon Launcher'
    $oldLinks = New-OldAtlas $p $custom
    Check (-not (Test-Path -LiteralPath $p.OldDir)) 'fixture: nothing in the default folder'
    $r = Invoke-Setup $p @('--remove-old-atlas')
    Assert-AtlasInstalled $p $r
    Assert-OldAtlasGone $custom $oldLinks
    Assert-DataKept $p
    Assert-UninstallKeepsOfficial $p '' '' @()

    Write-Host '6. The last Atlas, an old copy, uninstalled through its own executable with --remove-data'
    $p = New-Scratch 's6'
    $oldLinks = New-OldAtlas $p $p.OldDir
    $r = Invoke-Launcher $p $p.OldExe @('uninstall', '--quiet', '--remove-data')
    Check ($r.Code -eq 0) 'the old copy''s uninstall --quiet --remove-data exits 0'
    Check (-not $r.Err.Contains('Not removed')) 'the old copy''s uninstall removed all it meant to'
    Assert-OldAtlasGone $p.OldDir $oldLinks
    Check (-not (Test-Path $OwnKey)) 'no key of Atlas''s was made'
    Assert-DataKept $p $true

    Write-Host 'All checks passed.'
}
catch {
    Write-Host "::error::$($_.Exception.Message)"
    if (-not $_.Exception.Message.StartsWith('FAILED:')) {
        Write-Host $_.InvocationInfo.PositionMessage
    }
    $failed = $true
}
finally {
    Remove-Keys
    if (-not $parentExisted -and (Test-Path $ParentKey)) {
        $parent = Get-Item -Path $ParentKey
        if ($parent.SubKeyCount -eq 0 -and $parent.ValueCount -eq 0) {
            Remove-Item -Path $ParentKey -Force
        }
    }
    Remove-Item -LiteralPath $ScratchRoot -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failed) {
    exit 1
}
exit 0
