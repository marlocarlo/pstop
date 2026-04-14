$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  unzipLocation  = $toolsDir
  url64bit       = '__DOWNLOAD_URL__'
  checksum64     = '__CHECKSUM__'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

# Create shims for both pstop and htop
Install-BinFile -Name 'pstop' -Path (Join-Path $toolsDir 'pstop.exe')
Install-BinFile -Name 'htop'  -Path (Join-Path $toolsDir 'htop.exe')
