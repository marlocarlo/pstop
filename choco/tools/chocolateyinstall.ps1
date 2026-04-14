$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  unzipLocation  = $toolsDir
  url64bit       = 'https://github.com/psmux/pstop/releases/download/v0.5.3/pstop-windows-x86_64.zip'
  checksum64     = '37d07b823df9b20bc07163283897f03a45851783072e921d7b30f34b42d6f284'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

# Create shims for both pstop and htop
Install-BinFile -Name 'pstop' -Path (Join-Path $toolsDir 'pstop.exe')
Install-BinFile -Name 'htop'  -Path (Join-Path $toolsDir 'htop.exe')
