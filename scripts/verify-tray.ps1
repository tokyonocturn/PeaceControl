# Drives the PeaceControl tray menu the way Windows Explorer does and checks
# that every command changes real application state.
#
# The tray icon lives on a hidden window of class "PeaceControlShellWindow".
# Choosing a menu item makes Windows post WM_COMMAND (0x0111) with the item's
# identifier to that window, so posting the same message here runs exactly the
# handler a user's click runs. Nothing below fakes UI state: every assertion
# reads the window's position and extended style, the applied Equalizer APO
# file, the settings file or the log.
#
# Running in the tray parks the main window off-screen and removes its taskbar
# button instead of hiding it, because a hidden window stops receiving paint
# messages and egui only drains the tray queue while it paints. The checks
# therefore assert "parked" (off-screen, WS_EX_TOOLWINDOW, still alive) rather
# than "invisible".
#
#   powershell -ExecutionPolicy Bypass -File scripts\verify-tray.ps1
#
# Exits 0 when every command behaved, 1 otherwise. Requires an interactive
# desktop session (a tray icon cannot exist without one).

param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) "target\release\peacecontrol.exe"),
    [int]$TimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"

Add-Type -Namespace PeaceControl -Name Native -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll", CharSet=System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr FindWindow(string lpClassName, string lpWindowName);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool PostMessage(System.IntPtr hWnd, uint Msg, System.IntPtr wParam, System.IntPtr lParam);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern System.IntPtr SendMessage(System.IntPtr hWnd, uint Msg, System.IntPtr wParam, System.IntPtr lParam);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool IsWindowVisible(System.IntPtr hWnd);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool GetWindowRect(System.IntPtr hWnd, out RECT rect);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern System.IntPtr GetWindowLongPtr(System.IntPtr hWnd, int index);
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
'@

$WM_COMMAND = 0x0111
$WM_CLOSE = 0x0010
$WM_SYSCOMMAND = 0x0112
$SC_MINIMIZE = 0xF020
$GWL_EXSTYLE = -20
$WS_EX_TOOLWINDOW = 0x00000080
$PARKED_EDGE = -20000

$MENU_TOGGLE_WINDOW = 1
$MENU_TOGGLE_ENABLED = 2
$MENU_OPEN_SETTINGS = 3
$MENU_EXIT = 4
$MENU_PRESET_BASE = 1000

if (-not (Test-Path $Exe)) {
    Write-Output "The application was not found at $Exe. Build it with cargo build --release first."
    exit 1
}

$running = Get-Process -Name "PeaceControl" -ErrorAction SilentlyContinue
if ($running) {
    Write-Output "PeaceControl is already running; close it so this test drives only its own instance."
    exit 1
}

$root = Join-Path $env:TEMP ("peacecontrol-tray-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
$appExe = Join-Path $root "PeaceControl.exe"
$data = Join-Path $root "data"
$target = Join-Path $root "applied.txt"
$settingsPath = Join-Path $data "settings.json"
$logPath = Join-Path $data "logs\peacecontrol.log"
$proc = $null
$tray = [IntPtr]::Zero
$main = [IntPtr]::Zero
$failures = 0
$steps = 0
$expectedSteps = 12
$chosen = $null

function Step {
    param([string]$Name, [scriptblock]$Action)
    try {
        & $Action
        Write-Output "PASS  $Name"
        $script:steps++
    }
    catch {
        Write-Output "FAIL  $Name"
        Write-Output "      $($_.Exception.Message)"
        $script:failures++
        throw
    }
}

function Assert-True {
    param($Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function Wait-For {
    param([scriptblock]$Condition, [int]$Seconds = 8)
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        if (& $Condition) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

function Wait-Assert {
    param([scriptblock]$Condition, [string]$Message, [int]$Seconds = 8)
    if (-not (Wait-For $Condition $Seconds)) { throw $Message }
}

function Get-Settings {
    if (-not (Test-Path $settingsPath)) { return $null }
    return Get-Content $settingsPath -Raw | ConvertFrom-Json
}

function Get-Rect {
    $rect = New-Object PeaceControl.Native+RECT
    [void][PeaceControl.Native]::GetWindowRect($script:main, [ref]$rect)
    return $rect
}

function Test-Parked {
    if ($script:main -eq [IntPtr]::Zero) { return $false }
    if (-not [PeaceControl.Native]::IsWindowVisible($script:main)) { return $false }
    $rect = Get-Rect
    $style = [PeaceControl.Native]::GetWindowLongPtr($script:main, $GWL_EXSTYLE).ToInt64()
    return ($rect.Left -le $PARKED_EDGE) -and (($style -band $WS_EX_TOOLWINDOW) -ne 0)
}

function Test-OnScreen {
    if ($script:main -eq [IntPtr]::Zero) { return $false }
    if (-not [PeaceControl.Native]::IsWindowVisible($script:main)) { return $false }
    $rect = Get-Rect
    $style = [PeaceControl.Native]::GetWindowLongPtr($script:main, $GWL_EXSTYLE).ToInt64()
    return ($rect.Left -gt $PARKED_EDGE) -and (($style -band $WS_EX_TOOLWINDOW) -eq 0)
}

function Get-TrayWindow {
    # [NullString]::Value is what marshals to a real NULL; $null would become
    # an empty title and match nothing.
    return [PeaceControl.Native]::FindWindow("PeaceControlShellWindow", [NullString]::Value)
}

function Post-Command {
    param([int]$Id)
    Assert-True ($script:tray -ne [IntPtr]::Zero) "the tray window is gone"
    $ok = [PeaceControl.Native]::PostMessage($script:tray, $WM_COMMAND, [IntPtr]$Id, [IntPtr]::Zero)
    Assert-True $ok "PostMessage(WM_COMMAND, $Id) failed"
}

try {
    New-Item -ItemType Directory -Path $root, $data -Force | Out-Null
    Copy-Item $Exe $appExe
    New-Item -ItemType File -Path (Join-Path $root "peacecontrol.portable") -Force | Out-Null
    Set-Content -Path $target -Value "# initial content written by the test"
    @{
        enabled = $true
        target_config = $target
        close_to_tray = $true
        minimize_to_tray = $true
        automate_apps = $false
        apply_automatically = $true
    } | ConvertTo-Json | Set-Content -Path $settingsPath

    $proc = Start-Process -FilePath $appExe -PassThru
    Step "tray icon window is created" {
        Assert-True (Wait-For { (Get-TrayWindow) -ne [IntPtr]::Zero } 15) "no window of class PeaceControlShellWindow appeared"
        $script:tray = Get-TrayWindow
    }

    Step "main window is found on screen" {
        $deadline = (Get-Date).AddSeconds(15)
        while ((Get-Date) -lt $deadline -and $main -eq [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 150
            $proc.Refresh()
            if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { $script:main = $proc.MainWindowHandle }
        }
        Assert-True ($main -ne [IntPtr]::Zero) "Get-Process never reported a main window handle"
        Assert-True (Test-OnScreen) "the main window did not start on screen"
    }

    Step "Show/Hide parks the window in the tray" {
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-Parked } "the window was not parked off-screen without a taskbar button"
    }

    Step "Enabled toggle works while parked in the tray" {
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Assert {
            $settings = Get-Settings
            $settings -ne $null -and $settings.enabled -eq $false -and (Get-Item $target).Length -eq 0
        } "settings.enabled stayed true or the applied configuration was not emptied while parked"
    }

    Step "Enabled toggle can be undone while parked" {
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Assert {
            $settings = Get-Settings
            $settings -ne $null -and $settings.enabled -eq $true -and (Get-Item $target).Length -gt 0
        } "settings.enabled stayed false or no configuration was rendered while parked"
    }

    Step "Show/Hide brings the window back on screen" {
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-OnScreen } "the window did not come back on screen"
    }

    Step "Configurations submenu activates and persists a preset" {
        $names = @(Get-ChildItem (Join-Path $data "presets") -Filter *.json | ForEach-Object { $_.BaseName })
        Assert-True ($names.Count -gt 0) "no presets exist to choose from"
        Post-Command $MENU_PRESET_BASE
        Wait-Assert {
            $settings = Get-Settings
            $settings -ne $null -and $settings.last_preset
        } "settings.last_preset was not written"
        $settings = Get-Settings
        $script:chosen = $settings.last_preset
        Assert-True ($names -contains $script:chosen) "the activated preset '$($script:chosen)' is not in the library"
        Wait-Assert { (Test-Path $logPath) -and ((Get-Content $logPath -Raw) -match [regex]::Escape("activated configuration '$($script:chosen)'")) } "the log never recorded the activation of '$($script:chosen)'"
        Assert-True ((Get-Content $target -Raw).Contains($script:chosen)) "the applied configuration does not mention '$($script:chosen)'"
    }

    Step "Settings command restores the window and opens the settings UI" {
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-Parked } "could not park the window before opening settings"
        Post-Command $MENU_OPEN_SETTINGS
        Wait-Assert { Test-OnScreen } "the window was not brought back for the settings dialog"
        Wait-Assert { ((Get-Content $logPath -Raw) -match "settings window opened from the tray") } "the settings handler did not run"
    }

    Step "Close to tray parks instead of exiting" {
        [void][PeaceControl.Native]::SendMessage($main, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
        Wait-Assert { Test-Parked } "the window did not park itself after WM_CLOSE"
        Start-Sleep -Milliseconds 500
        Assert-True (-not $proc.HasExited) "the process exited instead of staying in the tray"
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-OnScreen } "the tray could not restore the window"
    }

    Step "Minimize to tray parks the window" {
        [void][PeaceControl.Native]::SendMessage($main, $WM_SYSCOMMAND, [IntPtr]$SC_MINIMIZE, [IntPtr]::Zero)
        Wait-Assert { Test-Parked } "minimising did not park the window in the tray"
        Assert-True (-not $proc.HasExited) "the process exited when minimised"
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-OnScreen } "the tray could not restore the window after minimising"
    }

    Step "Exit command quits the application" {
        Post-Command $MENU_EXIT
        Wait-Assert { $proc.HasExited } "the process did not exit" 10
    }

    Step "menu changes survive a restart" {
        $before = Get-Settings
        Assert-True ($before.enabled -eq $true) "enabled was not persisted as true"
        Assert-True ($before.last_preset -eq $script:chosen) "last_preset was not persisted"

        $script:proc = Start-Process -FilePath $appExe -PassThru
        Assert-True (Wait-For { (Get-TrayWindow) -ne [IntPtr]::Zero } 15) "the tray window did not come back after the restart"
        $script:tray = Get-TrayWindow
        Start-Sleep -Milliseconds 800
        Assert-True ((Get-Settings).last_preset -eq $script:chosen) "the restart forgot the active configuration"

        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Assert { (Get-Item $target).Length -eq 0 } "bypass after the restart did not empty the configuration"
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Assert { (Get-Item $target).Length -gt 0 } "enabling after the restart did not render a configuration"
        Assert-True ((Get-Content $target -Raw).Contains($script:chosen)) "the restarted application did not load '$($script:chosen)' from disk"

        Post-Command $MENU_EXIT
        Wait-Assert { $proc.HasExited } "the restarted process did not exit" 10
    }
}
catch {
    # The failing step already printed its reason; anything else is unexpected
    # and must not be reported as success.
    if ($steps -eq 0 -or $failures -eq 0) {
        Write-Output "FAIL  unexpected error: $($_.Exception.Message)"
        $failures++
    }
}
finally {
    if ($proc -and -not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    }
    if ($failures -eq 0 -and $steps -ne $expectedSteps) {
        Write-Output "FAIL  only $steps of $expectedSteps tray checks ran"
        $failures++
    }
    if ($failures -eq 0) {
        Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
        Write-Output ""
        Write-Output "All $steps tray menu checks behaved as documented."
    }
    else {
        Write-Output ""
        Write-Output "Tray verification failed; the test data was kept at $root"
    }
}

if ($failures -ne 0) { exit 1 }
exit 0
