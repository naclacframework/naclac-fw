# Windows PowerShell Installer for nacvm (Naclac Version Manager)
param (
    [string]$Version = $env:VERSION
)
$ProgressPreference = 'SilentlyContinue'

$owner = "naclacframework"
$repo = "naclac-fw"
$installDir = "$HOME\.nacvm\bin"

# Helper function to download file with a custom text-based progress bar
function Download-FileWithProgress {
    param (
        [string]$Url,
        [string]$OutFile
    )

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

# 1. Determine release version
if ($null -ne $Version -and $Version -ne "") {
    # Ensure version starts with 'v' if it doesn't already
    if ($Version -notlike "v*") {
        $Version = "v$Version"
    }
    $version = $Version
    
    $releaseUrl = "https://api.github.com/repos/$owner/$repo/releases/tags/$version"
    try {
        $release = Invoke-RestMethod -Uri $releaseUrl
        if ($release.prerelease) {
            Write-Host "⚠️  Downloading the pre-release version of $version" -ForegroundColor Yellow
        } else {
            Write-Host "🔍 Downloading stable version $version..."
        }
    } catch {
        Write-Host "❌ Error: Version $version not found." -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "🔍 Querying latest stable release..."
    $releaseUrl = "https://api.github.com/repos/$owner/$repo/releases/latest"
    try {
        $release = Invoke-RestMethod -Uri $releaseUrl
        $version = $release.tag_name
        Write-Host "🔍 Found latest stable version: $version"
    } catch {
        Write-Host "❌ Error: No stable version found. Specify a version if you have to download a pre-release version." -ForegroundColor Red
        exit 1
    }
}

# 2. Create directory
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir | Out-Null
}

# 3. Download Windows archive
$downloadUrl = "https://github.com/$owner/$repo/releases/download/$version/nacvm-x86_64-pc-windows-msvc.zip"
$tempZip = "$env:TEMP\nacvm.zip"
Write-Host "Downloading nacvm version $version..."
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

Write-Host "`n✅ nacvm CLI installed successfully!" -ForegroundColor Green
Write-Host "Please restart your PowerShell window to start using 'nacvm'!" -ForegroundColor Yellow
