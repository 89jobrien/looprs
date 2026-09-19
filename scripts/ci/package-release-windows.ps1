param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [Parameter(Mandatory = $true)][string]$Readme,
    [Parameter(Mandatory = $true)][string]$License,
    [Parameter(Mandatory = $true)][string]$Output
)

$ErrorActionPreference = "Stop"

foreach ($Path in @($Binary, $Readme, $License)) {
    if (-not (Test-Path -PathType Leaf $Path)) {
        throw "required package input is missing: $Path"
    }
}

$OutputPath = [System.IO.Path]::GetFullPath($Output)
$OutputDirectory = [System.IO.Path]::GetDirectoryName($OutputPath)
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$PackageDirectory = Join-Path $OutputDirectory "package"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $PackageDirectory
New-Item -ItemType Directory -Force $PackageDirectory | Out-Null
Copy-Item $Binary (Join-Path $PackageDirectory "looprs.exe")
Copy-Item $Readme (Join-Path $PackageDirectory "README.md")
Copy-Item $License (Join-Path $PackageDirectory "LICENSE")
Remove-Item -Force -ErrorAction SilentlyContinue $OutputPath
Compress-Archive -Path (Join-Path $PackageDirectory "*") -DestinationPath $OutputPath -CompressionLevel Optimal
$Hash = (Get-FileHash -Algorithm SHA256 $OutputPath).Hash.ToLowerInvariant()
$Name = [System.IO.Path]::GetFileName($OutputPath)
"$Hash  $Name" | Set-Content -Encoding ascii "$OutputPath.sha256"
Remove-Item -Recurse -Force $PackageDirectory
