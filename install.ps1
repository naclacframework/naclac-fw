# Windows PowerShell Installer for naclac CLI
$owner = "naclacframework"
$repo = "naclac-fw"
$installDir = "$HOME\.local\share\naclac\bin"

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
Invoke-WebRequest -Uri $downloadUrl -OutFile $tempZip

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
