# --- PowerShell Crash Log Collector for Skyrim SE (Low Privileges) ---

#region Function to Create Folder Browser Dialog (Modern Explorer Style)
function Show-FolderBrowserDialog {
    param (
        [string]$Description = "Select a folder",
        [string]$InitialDirectory
    )

    # We load only the necessary .NET assembly. 
    # This runs in standard user mode without requiring C# compilation permissions.
    Add-Type -AssemblyName System.Windows.Forms
    
    $fileBrowser = New-Object System.Windows.Forms.OpenFileDialog
    
    # Configure the dialog to look like a standard Explorer window
    $fileBrowser.Title = $Description
    if ($InitialDirectory -and (Test-Path $InitialDirectory)) {
        $fileBrowser.InitialDirectory = $InitialDirectory
    }

    # Configuration to enable "Folder Picking" via the OpenFile interface
    # This trick enables the Address Bar functionality
    $fileBrowser.ValidateNames = $false    # Allows "Folder Selection" as a name
    $fileBrowser.CheckFileExists = $false  # Don't check for an actual file
    $fileBrowser.CheckPathExists = $true   # Do ensure the folder exists
    $fileBrowser.FileName = "Folder Selection" # Dummy filename to allow the "Open" button to work
    $fileBrowser.Filter = "Folders|`n"         # Visual filter to hide clutter
    
    # ShowDialog is called without an owner window to prevent permission escalation 
    # or window handle manipulation requirements.
    $result = $fileBrowser.ShowDialog()

    if ($result -eq "OK") {
        # The dialog returns "Path\Folder Selection". We strip the dummy filename.
        return [System.IO.Path]::GetDirectoryName($fileBrowser.FileName)
    }
    return $null
}
#endregion

try {
    $scriptPath = $PSScriptRoot
    
    # --- 1. Get Skyrim Logs (30-Minute Window) ---
    # Construct path safely using Environment variable to ensure it works on any user account
    $docsPath = [System.Environment]::GetFolderPath('MyDocuments')
    $defaultLogPath = Join-Path $docsPath "My Games\Skyrim Special Edition\SKSE"
    
    Write-Host "Opening folder selection for Skyrim logs..."
    
    # Dialog 1
    $logFolderPath = Show-FolderBrowserDialog -Description "Navigate to SKSE Logs -> Click 'Open'" -InitialDirectory $defaultLogPath
    
    if (-not $logFolderPath) {
        throw "Log folder selection was canceled. Halting script."
    }

    Write-Host "Selected Log Path: $logFolderPath"

    # Get files with Read-Only intent
    $allLogFiles = Get-ChildItem -Path $logFolderPath -Filter "*.log" | Sort-Object LastWriteTime -Descending
    
    if ($allLogFiles.Count -eq 0) {
        Write-Warning "No .log files found in the selected directory."
        $logFilesToProcess = @()
    } else {
        $newestLogTime = $allLogFiles[0].LastWriteTime
        $cutoffTime = $newestLogTime.AddMinutes(-30)
        
        $logFilesToProcess = $allLogFiles | Where-Object { $_.LastWriteTime -ge $cutoffTime }
        Write-Host "Found $($logFilesToProcess.Count) logs from the latest session (Window: 30 mins)."
    }

    $logsOutputFile = Join-Path -Path $scriptPath -ChildPath "LOGS.txt"
    if (Test-Path $logsOutputFile) { Clear-Content $logsOutputFile -ErrorAction SilentlyContinue }

    # Process logs
    foreach ($logFile in $logFilesToProcess) {
        Add-Content -Path $logsOutputFile -Value "--- Log File: $($logFile.Name) [$($logFile.LastWriteTime)] ---`r`n"
        # Stream content to avoid memory spikes on large files
        Get-Content -Path $logFile.FullName -ReadCount 0 | Add-Content -Path $logsOutputFile
        Add-Content -Path $logsOutputFile -Value "`r`n"
    }

    # --- 2. Get MO2 Profile Files ---
    Write-Host "Opening folder selection for your MO2 profile..."
    
    # Dialog 2
    $mo2ProfilePath = Show-FolderBrowserDialog -Description "Navigate to MO2 Profile -> Click 'Open'"
    
    if (-not $mo2ProfilePath) {
        throw "MO2 profile folder selection was canceled. Halting script."
    }

    Write-Host "Selected MO2 Path: $mo2ProfilePath"

    # Get files (Read-Only)
    $mo2FilesToProcess = Get-ChildItem -Path $mo2ProfilePath -File | Where-Object { $_.Extension -match '\.(txt|ini)$' }
    
    $mo2OutputFile = Join-Path -Path $scriptPath -ChildPath "MO2 PROFILE.txt"
    New-Item -Path $mo2OutputFile -ItemType File -Force | Out-Null
    
    if ($mo2FilesToProcess.Count -eq 0) {
        Write-Warning "No .txt or .ini files found in the MO2 profile folder."
        Add-Content -Path $mo2OutputFile -Value "No text or ini files found in selected folder."
    } else {
        foreach ($file in $mo2FilesToProcess) {
            Add-Content -Path $mo2OutputFile -Value "--- Profile File: $($file.Name) ---`r`n"
            Get-Content -Path $file.FullName -ReadCount 0 | Add-Content -Path $mo2OutputFile
            Add-Content -Path $mo2OutputFile -Value "`r`n"
        }
    }

    Write-Host -ForegroundColor Green "Operation successful!" 
    Write-Host -ForegroundColor Green "Processed $($logFilesToProcess.Count) logs and $($mo2FilesToProcess.Count) profile files."
    Write-Host -ForegroundColor Green "Files 'LOGS.txt' and 'MO2 PROFILE.txt' have been created."

} catch {
    Write-Host -ForegroundColor Red "An error occurred: $_"
    Write-Host -ForegroundColor Red "Operation failed."
} finally {
    Write-Host "This window will close in 5 seconds..."
    Start-Sleep -Seconds 5
}