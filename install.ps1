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

    # Suppress the default PowerShell UI progress bar
    $ProgressPreference = 'SilentlyContinue'

    $httpClient = New-Object System.Net.Http.HttpClient
    try {
        $response = $httpClient.GetAsync($Url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        
        if (-not $response.IsSuccessStatusCode) {
            throw "Failed to download file. Status code: $($response.StatusCode)"
        }

        $totalBytes = $response.Content.Headers.ContentLength
        if ($null -eq $totalBytes) {
            $totalBytes = 0
        }

        $responseStream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $fileStream = [System.IO.File]::Create($OutFile)
        
        try {
            $buffer = New-Object byte[] 8192
            $bytesRead = 0
            $totalBytesRead = 0
            $progressBarLength = 30

            while (($bytesRead = $responseStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $fileStream.Write($buffer, 0, $bytesRead)
                $totalBytesRead += $bytesRead

                if ($totalBytes -gt 0) {
                    $percentage = [Math]::Round(($totalBytesRead / $totalBytes) * 100)
                    $filledLength = [int][Math]::Round(($totalBytesRead / $totalBytes) * $progressBarLength)
                    $unfilledLength = $progressBarLength - $filledLength
                    
                    $progressBar = "[" + ("=" * $filledLength) + (">" * [int]($unfilledLength -gt 0)) + (" " * [Math]::Max(0, $unfilledLength - 1)) + "]"
                    
                    $mbRead = [Math]::Round($totalBytesRead / 1MB, 2)
                    $mbTotal = [Math]::Round($totalBytes / 1MB, 2)
                    
                    Write-Host -NoNewline "`rDownloading: $progressBar $percentage% ($mbRead MB / $mbTotal MB)   "
                } else {
                    $mbRead = [Math]::Round($totalBytesRead / 1MB, 2)
                    Write-Host -NoNewline "`rDownloading: ($mbRead MB)..."
                }
            }
        } finally {
            $fileStream.Close()
            $responseStream.Close()
        }
    } finally {
        $httpClient.Dispose()
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

# 5. Add to PATH
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable('Path', $userPath + ';$installDir', 'User')
    Write-Host "Added $installDir to User PATH."
}

Write-Host "`n✅ naclac CLI installed successfully!" -ForegroundColor Green
Write-Host "Please restart your PowerShell window to start using 'naclac'!" -ForegroundColor Yellow
