# Drives the PeaceControl tray menu the way Windows Explorer does and checks
# that every command changes real application state, including what the running
# interface shows.
#
# The tray icon lives on a hidden window of class "PeaceControlShellWindow".
# Choosing a menu item makes Windows post WM_COMMAND (0x0111) with the item's
# identifier to that window, so posting the same message here runs exactly the
# handler a user's click runs. The checks read:
#
#   * the UI snapshot the render path writes when PEACECONTROL_UI_DUMP is set
#     (preset, header EQ label, settings status, applied line, target,
#     settings panel open, window state);
#   * the window rectangle and extended style;
#   * settings.json and the applied configuration file;
#   * the log;
#   * a screenshot of the window with the Settings panel open.
#
# Running in the tray parks the main window off-screen and removes its taskbar
# button instead of hiding it, because a hidden window stops receiving paint
# messages and egui only drains the tray queue while it paints.
#
#   powershell -ExecutionPolicy Bypass -File scripts\verify-tray.ps1
#
# Exits 0 when every check passed, 1 otherwise. Requires an interactive desktop
# session (a tray icon cannot exist without one).

param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) "target\release\peacecontrol.exe"),
    [string]$ShotDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) "dist\verification"),
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
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool SetForegroundWindow(System.IntPtr hWnd);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool PrintWindow(System.IntPtr hWnd, System.IntPtr hdc, uint flags);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool SetWindowPos(System.IntPtr hWnd, System.IntPtr after, int x, int y, int cx, int cy, uint flags);
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
$dumpPath = Join-Path $root "ui-state.txt"
$proc = $null
$tray = [IntPtr]::Zero
$main = [IntPtr]::Zero
$failures = 0
$steps = 0
$expectedSteps = 13
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

function Get-Ui {
    if (-not (Test-Path $dumpPath)) { return "" }
    return Get-Content $dumpPath -Raw
}

function Get-UiValue {
    param([string]$Key)
    $line = (Get-Ui) -split "`n" | Where-Object { $_ -like "$Key=*" } | Select-Object -First 1
    if (-not $line) { return $null }
    return $line.Substring($Key.Length + 1).Trim()
}

function Wait-Ui {
    param([string]$Key, [string]$Value, [string]$Message, [int]$Seconds = 8)
    Wait-Assert { (Get-UiValue $Key) -eq $Value } "$Message (dump: $(Get-UiValue $Key))" $Seconds
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

function Test-BitmapHasContent {
    param($Bitmap)
    $colours = @{}
    $stepX = [Math]::Max(1, [int]($Bitmap.Width / 16))
    $stepY = [Math]::Max(1, [int]($Bitmap.Height / 16))
    for ($x = 0; $x -lt $Bitmap.Width; $x += $stepX) {
        for ($y = 0; $y -lt $Bitmap.Height; $y += $stepY) {
            $colours[$Bitmap.GetPixel($x, $y).ToArgb()] = 1
        }
    }
    return $colours.Count -gt 4
}

function Save-WindowShot {
    param([string]$Name)
    $rect = Get-Rect
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    Assert-True ($width -gt 100 -and $height -gt 100) "the window has no usable size to capture"
    New-Item -ItemType Directory -Path $ShotDirectory -Force | Out-Null
    $path = Join-Path $ShotDirectory $Name
    Add-Type -AssemblyName System.Drawing

    $bitmap = New-Object System.Drawing.Bitmap($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        # PrintWindow renders the window itself, so an overlapping window cannot
        # end up in the capture. Some OpenGL surfaces come back empty that way,
        # so fall back to a screen capture of a temporarily topmost window.
        $hdc = $graphics.GetHdc()
        try {
            $printed = [PeaceControl.Native]::PrintWindow($script:main, $hdc, 2)
        }
        finally {
            $graphics.ReleaseHdc($hdc)
        }
        if (-not $printed -or -not (Test-BitmapHasContent $bitmap)) {
            [void][PeaceControl.Native]::SetWindowPos($script:main, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002)
            [void][PeaceControl.Native]::SetForegroundWindow($script:main)
            Start-Sleep -Milliseconds 600
            $rect = Get-Rect
            $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
            [void][PeaceControl.Native]::SetWindowPos($script:main, [IntPtr](-2), 0, 0, 0, 0, 0x0001 -bor 0x0002)
        }
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    Assert-True (Test-Path $path) "the screenshot was not written"
    Write-Output "      screenshot: $path"
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
    $env:PEACECONTROL_UI_DUMP = $dumpPath

    $proc = Start-Process -FilePath $appExe -PassThru
    Step "tray icon window is created" {
        Assert-True (Wait-For { (Get-TrayWindow) -ne [IntPtr]::Zero } 15) "no window of class PeaceControlShellWindow appeared"
        $script:tray = Get-TrayWindow
    }

    Step "the interface reports its header state" {
        $deadline = (Get-Date).AddSeconds(15)
        while ((Get-Date) -lt $deadline -and $script:main -eq [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 150
            $proc.Refresh()
            if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { $script:main = $proc.MainWindowHandle }
        }
        Assert-True ($script:main -ne [IntPtr]::Zero) "Get-Process never reported a main window handle"
        Assert-True (Test-OnScreen) "the main window did not start on screen"
        Assert-True (Wait-For { (Get-Ui).Length -gt 0 } 10) "the render path never wrote a UI snapshot"
        Assert-True ((Get-UiValue "eq") -eq "EQ on") "the header did not start enabled"
        Assert-True ((Get-UiValue "window") -eq "onscreen") "the snapshot did not report the window on screen"
        Assert-True ((Get-UiValue "settings_open") -eq "false") "the settings panel started open"
        $script:chosen = Get-UiValue "preset"
        Assert-True ($script:chosen -and $script:chosen.Length -gt 0) "the header has no active configuration"
    }

    Step "tray Enabled toggle updates the header and settings view" {
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Ui "eq" "EQ bypassed" "the header still shows the equalizer on"
        Wait-Ui "settings_eq" "Bypassed" "the settings status still shows the equalizer on"
        Wait-Assert { (Get-Item $target).Length -eq 0 } "the applied configuration was not emptied"
        Assert-True ((Get-Settings).enabled -eq $false) "settings.enabled stayed true"
        Assert-True ((Get-UiValue "applied") -like "applied *") "the status bar does not report the bypass apply"
        Save-WindowShot "tray-header-bypassed.png"
    }

    Step "tray Enabled toggle restores the header state" {
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Ui "eq" "EQ on" "the header still shows the equalizer bypassed"
        Wait-Ui "settings_eq" "Enabled" "the settings status still shows the equalizer bypassed"
        Wait-Assert { (Get-Item $target).Length -gt 0 } "the configuration was not rendered again"
    }

    Step "Show/Hide parks the window in the tray" {
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-Parked } "the window was not parked off-screen without a taskbar button"
        Wait-Ui "window" "parked" "the render path still reports the window on screen"
    }

    Step "tray Enabled toggle updates the header while parked" {
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Ui "eq" "EQ bypassed" "the parked interface did not update the header"
        Wait-Assert { (Get-Item $target).Length -eq 0 } "the bypass while parked did not empty the configuration"
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Ui "eq" "EQ on" "the parked interface did not restore the header"
        Wait-Ui "window" "parked" "the toggle brought the window back on screen"
    }

    Step "Show/Hide brings the window back on screen" {
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-OnScreen } "the window did not come back on screen"
        Wait-Ui "window" "onscreen" "the render path did not report the window on screen"
    }

    Step "Configurations submenu updates the header and the settings view" {
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
        Wait-Ui "preset" $script:chosen "the header does not show the activated configuration"
        Wait-Ui "settings_preset" $script:chosen "the settings view does not show the activated configuration"
        Wait-Assert { (Test-Path $logPath) -and ((Get-Content $logPath -Raw) -match [regex]::Escape("activated configuration '$($script:chosen)'")) } "the log never recorded the activation of '$($script:chosen)'"
        Assert-True ((Get-Content $target -Raw).Contains($script:chosen)) "the applied configuration does not mention '$($script:chosen)'"
    }

    Step "Settings command puts the settings panel on screen" {
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-Parked } "could not park the window before opening settings"
        Wait-Ui "window" "parked" "the snapshot did not report the parked window"
        Post-Command $MENU_OPEN_SETTINGS
        Wait-Assert { Test-OnScreen } "the window was not brought back for the settings dialog"
        Wait-Ui "settings_open" "true" "the settings panel is not open in the render path"
        Assert-True ((Get-UiValue "settings_preset") -eq $script:chosen) "the open settings panel does not show the active configuration"
        Save-WindowShot "tray-settings.png"
    }

    Step "Close to tray parks instead of exiting" {
        [void][PeaceControl.Native]::SendMessage($script:main, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
        Wait-Assert { Test-Parked } "the window did not park itself after WM_CLOSE"
        Wait-Ui "window" "parked" "the render path still reports the window on screen after WM_CLOSE"
        Start-Sleep -Milliseconds 500
        Assert-True (-not $proc.HasExited) "the process exited instead of staying in the tray"
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-OnScreen } "the tray could not restore the window"
        Wait-Ui "window" "onscreen" "the render path did not report the restored window"
    }

    Step "Minimize to tray parks the window" {
        [void][PeaceControl.Native]::SendMessage($script:main, $WM_SYSCOMMAND, [IntPtr]$SC_MINIMIZE, [IntPtr]::Zero)
        Wait-Assert { Test-Parked } "minimising did not park the window in the tray"
        Wait-Ui "window" "parked" "the render path did not report the window parked after minimising"
        Assert-True (-not $proc.HasExited) "the process exited when minimised"
        Post-Command $MENU_TOGGLE_WINDOW
        Wait-Assert { Test-OnScreen } "the tray could not restore the window after minimising"
    }

    Step "Exit command quits the application" {
        Post-Command $MENU_EXIT
        Wait-Assert { $proc.HasExited } "the process did not exit" 10
    }

    Step "menu changes survive a restart and the interface shows them" {
        $before = Get-Settings
        Assert-True ($before.enabled -eq $true) "enabled was not persisted as true"
        Assert-True ($before.last_preset -eq $script:chosen) "last_preset was not persisted"

        Remove-Item $dumpPath -ErrorAction SilentlyContinue
        $script:proc = Start-Process -FilePath $appExe -PassThru
        Assert-True (Wait-For { (Get-TrayWindow) -ne [IntPtr]::Zero } 15) "the tray window did not come back after the restart"
        $script:tray = Get-TrayWindow
        Start-Sleep -Milliseconds 800
        Assert-True ((Get-Settings).last_preset -eq $script:chosen) "the restart forgot the active configuration"
        Assert-True (Wait-For { (Get-Ui).Length -gt 0 } 10) "the restarted interface wrote no snapshot"
        Assert-True ((Get-UiValue "preset") -eq $script:chosen) "the restarted header does not show the persisted configuration"
        Assert-True ((Get-UiValue "eq") -eq "EQ on") "the restarted header is not enabled"

        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Ui "eq" "EQ bypassed" "the restarted interface did not follow the tray toggle"
        Wait-Assert { (Get-Item $target).Length -eq 0 } "bypass after the restart did not empty the configuration"
        Post-Command $MENU_TOGGLE_ENABLED
        Wait-Ui "eq" "EQ on" "the restarted interface did not restore the header"
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
        Write-Output ""
        Write-Output "All $steps tray menu checks behaved as documented."
        Write-Output ""
        Write-Output "Last render-path snapshot:"
        Get-Ui | ForEach-Object { Write-Output "  $_" }
        Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
    }
    else {
        Write-Output ""
        Write-Output "Tray verification failed; the test data was kept at $root"
        Write-Output "Last render-path snapshot:"
        Get-Ui | ForEach-Object { Write-Output "  $_" }
    }
}

if ($failures -ne 0) { exit 1 }
exit 0
