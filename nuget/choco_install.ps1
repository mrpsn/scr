$ErrorActionPreference = 'Stop';

$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$installDir = "$(Get-ToolsLocation)\scanr"

Write-Host "Installing scanr to $installDir"

if (-not (Test-Path $installDir)) {
    mkdir $installDir
}

# Copy the content of the tools directory to the installation directory
Write-Host "Copying files to $installDir"
Copy-Item "$toolsDir\*" "$installDir\" -Recurse -Force

# Add the install directory to the PATH
$path = [System.Environment]::GetEnvironmentVariable('PATH', [System.EnvironmentVariableTarget]::Machine)
if ($path -notlike "*$installDir*") {
    [System.Environment]::SetEnvironmentVariable('PATH', "$path;$installDir", [System.EnvironmentVariableTarget]::Machine)
}

# Add the install directory to the PATH for the current session
$env:Path += ";$installDir"
