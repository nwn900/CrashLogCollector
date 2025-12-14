# --- PowerShell Crash Log Collector for Skyrim SE (30 Minute Window) ---

#region Function to Create Folder Browser Dialog
function Show-FolderBrowserDialog {
    param (
        [string]$Description = "Select a folder",
        [string]$InitialDirectory
    )

    Add-Type -AssemblyName System.Windows.Forms
    $folderBrowser = New-Object System.Windows.Forms.FolderBrowserDialog
    $folderBrowser.Description = $Description
    if ($InitialDirectory) {
        $folderBrowser.SelectedPath = $InitialDirectory
    }

    $result = $folderBrowser.ShowDialog((New-Object System.Windows.Forms.Form -Property @{TopMost = $true }))
    if ($result -eq "OK") {
        return $folderBrowser.SelectedPath
    }
    return $null
}
#endregion

try {
    $scriptPath = $PSScriptRoot
    
    # --- 1. Get Skyrim Logs (30-Minute Window) ---
    $defaultLogPath = [System.Environment]::GetFolderPath('MyDocuments') + "\My Games\Skyrim Special Edition\SKSE"
    
    Write-Host "Opening folder selection for Skyrim logs..."
    $logFolderPath = Show-FolderBrowserDialog -Description "Select the folder containing your Skyrim logs (e.g., ...\SKSE)" -InitialDirectory $defaultLogPath
    
    if (-not $logFolderPath) {
        throw "Log folder selection was canceled. Halting script."
    }

    $allLogFiles = Get-ChildItem -Path $logFolderPath -Filter "*.log" | Sort-Object LastWriteTime -Descending
    
    if ($allLogFiles.Count -eq 0) {
        Write-Warning "No .log files found in the selected directory."
        $logFilesToProcess = @()
    } else {
        $newestLogTime = $allLogFiles[0].LastWriteTime
        
        # CHANGED: Now looks back 30 minutes from the newest log
        $cutoffTime = $newestLogTime.AddMinutes(-30)
        
        $logFilesToProcess = $allLogFiles | Where-Object { $_.LastWriteTime -ge $cutoffTime }
        Write-Host "Found $($logFilesToProcess.Count) logs from the latest session (Window: 30 mins)."
    }

    $logsOutputFile = Join-Path -Path $scriptPath -ChildPath "LOGS.txt"
    if (Test-Path $logsOutputFile) { Clear-Content $logsOutputFile }

    foreach ($logFile in $logFilesToProcess) {
        Add-Content -Path $logsOutputFile -Value "--- Log File: $($logFile.Name) [$($logFile.LastWriteTime)] ---`r`n"
        Get-Content -Path $logFile.FullName | Add-Content -Path $logsOutputFile
        Add-Content -Path $logsOutputFile -Value "`r`n"
    }

    # --- 2. Get MO2 Profile Files ---
    Write-Host "Opening folder selection for your MO2 profile..."
    $mo2ProfilePath = Show-FolderBrowserDialog -Description "Select your current MO2 profile folder"
    
    if (-not $mo2ProfilePath) {
        throw "MO2 profile folder selection was canceled. Halting script."
    }

    # Reliable file grabbing using Where-Object
    $mo2FilesToProcess = Get-ChildItem -Path $mo2ProfilePath -File | Where-Object { $_.Extension -match '\.(txt|ini)$' }
    
    $mo2OutputFile = Join-Path -Path $scriptPath -ChildPath "MO2 PROFILE.txt"
    New-Item -Path $mo2OutputFile -ItemType File -Force | Out-Null
    
    if ($mo2FilesToProcess.Count -eq 0) {
        Write-Warning "No .txt or .ini files found in the MO2 profile folder."
        Add-Content -Path $mo2OutputFile -Value "No text or ini files found in selected folder."
    } else {
        foreach ($file in $mo2FilesToProcess) {
            Add-Content -Path $mo2OutputFile -Value "--- Profile File: $($file.Name) ---`r`n"
            Get-Content -Path $file.FullName | Add-Content -Path $mo2OutputFile
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