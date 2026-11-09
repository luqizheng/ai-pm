# stop.ps1 —— 停止全栈部署单元（infra + observability + services）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   .\stop.ps1              # down（保留 named volumes）
#   .\stop.ps1 --volumes    # 连 named volume 一起删（清库/清监控数据）
#   .\stop.ps1 --help
#
# 定位依据是 project 名，与启动时用的 overlay 片 / --logs 无关。
# 本单元的 project 名是 superpowers-full（与 docker-deploy 的 superpowers-svc 区分）。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$DownArgs
)

Set-StrictMode -Version Latest
# 与 start.ps1 同理：不用 $ErrorActionPreference='Stop'（docker 进度走 stderr，
# PS5 会误判为致命错误）；退出码以 $LASTEXITCODE 为准。

$Script:Dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Project    = 'superpowers-full'

if ($DownArgs -contains '--help' -or $DownArgs -contains '-h') {
    $inHeader = $true
    foreach ($line in (Get-Content (Join-Path $Dir 'stop.ps1') -Encoding UTF8)) {
        if ($inHeader -and $line -match '^#') {
            $line -replace '^# ?', ''
        }
        else {
            $inHeader = $false
        }
    }
    exit 0
}

# compose 插值需要 IMAGE_*（.image.env）：显式 --env-file，使 stop 在新开窗口也能跑
$EnvArgs = @()
foreach ($f in @('.env', '.env.prod', '.image.env')) {
    $p = Join-Path $Dir $f
    if (Test-Path $p) { $EnvArgs += @('--env-file', $p) }
}

Push-Location $Dir
try {
    Write-Host "[stop:$Project] down..."
    & docker compose -p $Project $EnvArgs -f docker-compose.yml down @DownArgs
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
