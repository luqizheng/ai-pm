# stop.ps1 —— 停止全栈部署单元（infra + observability + lmms services + ai-pm）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   .\stop.ps1              # down（保留 named volumes）
#   .\stop.ps1 --volumes    # 连 named volume 一起删（清库/清监控数据）
#   .\stop.ps1 --help
#
# 定位依据是 project 名，与启动时用的 overlay 片 / --logs 无关
# （compose down 按 com.docker.compose.project 标签清理全部容器，含 --logs
# 叠加的 loki/promtail）。compose 文件链只取顶层聚合 docker-compose.yml
# （其 include 已含 infra / observability / services / ai-pm 四片），
# 用于解析 networks / volumes 声明。
# 本单元的 project 名是 superpowers-full（与 docker-deploy 的 superpowers-svc 区分）。
#
# env 加载与 start.ps1 完全对齐（保证新开窗口直接跑 stop 也能完成插值）：
#   1. .env       —— 向上回退：$DIR\..\..\.env → $DIR\..\.env → $DIR\.env
#   2. .env.prod  —— 运维覆盖层（可选；缺失不阻塞）
#   3. .image.env —— **镜像钉版**，最后加载 → 权威
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$DownArgs
)

$Script:Dir      = Split-Path -Parent $MyInvocation.MyCommand.Path
$Script:RepoRoot = (Resolve-Path (Join-Path $Dir '..\..')).Path
$Project         = 'superpowers-full'

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

Set-StrictMode -Version Latest
# 与 start.ps1 同理：不用 $ErrorActionPreference='Stop'（docker 进度走 stderr，
# PS5 会误判为致命错误）；退出码以 $LASTEXITCODE 为准。

function Log([string]$msg) { Write-Host "[stop:$Project] $msg" }
function Die([string]$msg, [int]$code = 10) {
    Write-Error "[stop:$Project] FATAL: $msg" -ErrorAction Continue
    exit $code
}

# ── 基础文件校验 ─────────────────────────────────────────────────────────────
if (-not (Test-Path (Join-Path $Dir 'docker-compose.yml'))) { Die 'docker-compose.yml not found next to stop.ps1' }

# ── env 分层加载（与 start.ps1 同款 dotenv 解析） ────────────────────────────
function Import-DotEnv([string]$Path) {
    if (-not (Test-Path $Path)) { return }
    foreach ($line in Get-Content $Path -Encoding UTF8) {
        $line = $line.Trim()
        if (-not $line -or $line.StartsWith('#')) { continue }
        $idx = $line.IndexOf('=')
        if ($idx -lt 1) { continue }
        $name  = $line.Substring(0, $idx).Trim()
        $value = $line.Substring($idx + 1).Trim()
        $expand = $true
        if ($value.Length -ge 2 -and $value.StartsWith("'") -and $value.EndsWith("'")) {
            $value  = $value.Substring(1, $value.Length - 2)
            $expand = $false
        }
        elseif ($value.Length -ge 2 -and $value.StartsWith('"') -and $value.EndsWith('"')) {
            $value = $value.Substring(1, $value.Length - 2)
        }
        else {
            $value = $value -replace '\s+#.*$', ''  # 非引号值的行内注释
        }
        if ($expand) {
            $value = [regex]::Replace($value, '\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)', {
                param($m)
                $var = if ($m.Groups[1].Success) { $m.Groups[1].Value } else { $m.Groups[2].Value }
                $v = [Environment]::GetEnvironmentVariable($var, 'Process')
                if ($null -eq $v) { '' } else { $v }
            })
        }
        [Environment]::SetEnvironmentVariable($name, $value, 'Process')
    }
    Log "env: $Path"
}
Import-DotEnv (Join-Path $Dir '..\..\.env')
Import-DotEnv (Join-Path $Dir '..\.env')
Import-DotEnv (Join-Path $Dir '.env')
Import-DotEnv (Join-Path $Dir '..\..\.env.prod')
Import-DotEnv (Join-Path $Dir '..\.env.prod')
Import-DotEnv (Join-Path $Dir '.env.prod')
Import-DotEnv (Join-Path $Dir '..\..\.image.env')
Import-DotEnv (Join-Path $Dir '..\.image.env')
Import-DotEnv (Join-Path $Dir '.image.env')

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
$EnvArgs = @()
foreach ($f in @((Join-Path $Dir '.env'), (Join-Path $Dir '.env.prod'), (Join-Path $Dir '.image.env'))) {
    if (Test-Path $f) { $EnvArgs += @('--env-file', $f) }
}

# ── 停止 ─────────────────────────────────────────────────────────────────────
Push-Location $Dir
try {
    Log "down..."
    $downCmd = @('compose', '-p', $Project) + $EnvArgs + @('-f', 'docker-compose.yml', 'down')
    if ($DownArgs) { $downCmd += $DownArgs }
    & docker @downCmd
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
