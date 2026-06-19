# Windows PowerShell Installer for naclac CLI
$owner = "naclacframework"
$repo = "naclac-fw"
$installDir = "$HOME\.local\share\naclac\bin"

# Helper function to download file with a custom text-based progress bar
function Download-FileWithProgress {
    param (
        [string]$Url,
        [string]$OutFile
    )

    # Suppress default PowerShell progress bar
    $ProgressPreference = 'SilentlyContinue'

    $webClient = New-Object System.Net.WebClient
    try {
        # Open the read stream (WebClient automatically follows redirects)
        $responseStream = $webClient.OpenRead($Url)
        $totalBytes = 0
        if ($null -ne $webClient.ResponseHeaders["Content-Length"]) {
            $totalBytes = [Int64]$webClient.ResponseHeaders["Content-Length"]
        }

        $fileStream = [System.IO.File]::Create($OutFile)
        try {
            # 64 KB buffer
            $buffer = New-Object byte[] 65536
            $bytesRead = 0
            $totalBytesRead = 0
            $progressBarLength = 30
            $lastPercentage = -1
            $lastUpdate = [DateTime]::MinValue

            $spin = @('⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏')
            $spinIdx = 0

            while (($bytesRead = $responseStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $fileStream.Write($buffer, 0, $bytesRead)
                $totalBytesRead += $bytesRead

                $spinChar = $spin[$spinIdx % 10]
                $spinIdx++

                if ($totalBytes -gt 0) {
                    $percentage = [Math]::Round(($totalBytesRead / $totalBytes) * 100)
                    $now = [DateTime]::Now
                    
                    # Only update visual display if percentage changes, 100ms has passed, or download finished
                    if ($percentage -ne $lastPercentage -or ($now - $lastUpdate).TotalMilliseconds -ge 100 -or $totalBytesRead -eq $totalBytes) {
                        $lastPercentage = $percentage
                        $lastUpdate = $now

                        $filledLength = [int][Math]::Round(($totalBytesRead / $totalBytes) * $progressBarLength)
                        $unfilledLength = $progressBarLength - $filledLength
                        
                        $progressBar = "[" + ("=" * $filledLength) + (">" * [int]($unfilledLength -gt 0)) + (" " * [Math]::Max(0, $unfilledLength - 1)) + "]"
                        
                        $mbRead = [Math]::Round($totalBytesRead / 1MB, 2)
                        $mbTotal = [Math]::Round($totalBytes / 1MB, 2)
                        
                        Write-Host -NoNewline "`r$spinChar 🚚 Downloading [$progressBar] $percentage% ($mbRead MB / $mbTotal MB)   "
                    }
                } else {
                    $mbRead = [Math]::Round($totalBytesRead / 1MB, 2)
                    Write-Host -NoNewline "`r$spinChar 🚚 Downloading ($mbRead MB)..."
                }
            }
        } finally {
            $fileStream.Close()
            $responseStream.Close()
        }
    } finally {
        $webClient.Dispose()
    }

    # Ensure final state is printed cleanly
    if ($totalBytes -gt 0) {
        $finalBar = "=" * $progressBarLength
        $mbTotal = [Math]::Round($totalBytes / 1MB, 2)
        Write-Host -NoNewline "`r✅ 🚚 Downloading [$finalBar] 100% ($mbTotal MB / $mbTotal MB)   "
    }
    Write-Host "`nDownload complete!"
}

# 1. Query latest release version
$releaseUrl = "https://api.github.com/repos/$owner/$repo/releases/latest"
$release = Invoke-RestMethod -Uri $releaseUrl
$version = $release.tag_name

# 2. Create directory
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir | Out-Null
}

# 3. Download Windows archive
$downloadUrl = "https://github.com/$owner/$repo/releases/download/$version/naclac-x86_64-pc-windows-msvc.zip"
$tempZip = "$env:TEMP\naclac.zip"
Write-Host "Downloading naclac version $version..."
Download-FileWithProgress -Url $downloadUrl -OutFile $tempZip

# 4. Extract
Write-Host "Extracting to $installDir..."
Expand-Archive -Path $tempZip -DestinationPath $installDir -Force
Remove-Item -Path $tempZip

# 5. Add to PATH (using double quotes for proper expansion)
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
# Clean up any literal '$installDir' text from previous run
$userPath = $userPath.Replace(';$installDir', '').Replace('$installDir', '')
[Environment]::SetEnvironmentVariable('Path', $userPath, 'User')

if ($userPath -notlike "*$installDir*") {
    $newUserPath = $userPath + ";$installDir"
    [Environment]::SetEnvironmentVariable('Path', $newUserPath, 'User')
    Write-Host "Added $installDir to User PATH."
}

Write-Host "`n✅ naclac CLI installed successfully!" -ForegroundColor Green
Write-Host "Please restart your PowerShell window to start using 'naclac'!" -ForegroundColor Yellow
