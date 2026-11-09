# start.ps1 —— 全栈自包含部署单元（infra + observability + services）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   .\start.ps1                 # prod 片（业务端口只留 gateway :3000）；仓内自动 --build
#   .\start.ps1 --remote        # 跨主机分置片（保留业务端口供 consul 回拨）+ 自动 --no-build
#   .\start.ps1 --logs          # 叠加日志栈（loki + promtail）
#   .\start.ps1 --build         # 强制 build（覆盖自动检测）
#   .\start.ps1 --no-build      # 强制跳过 build（覆盖自动检测）
#   .\start.ps1 --force-recreate  # 透传其余 docker compose up 参数
#
# env 加载（后者覆盖前者）：
#   1. .env       —— 环境配置（IP / 口令 / 拓扑）
#                  向上回退：$DIR\..\..\.env → $DIR\..\.env → $DIR\.env
#   2. .env.prod  —— 运维覆盖层（可选；缺失不阻塞）
#   3. .image.env —— **镜像钉版**（平台镜像机器生成 + 基础镜像手工维护），最后加载 → 权威
#
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；缺变量直接报错。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [switch]$Remote,
    [switch]$Prod,
    [switch]$Logs,
    [switch]$Build,
    [switch]$NoBuild,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$UpArgs
)

$Script:Dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Project    = 'superpowers-full'

if ($UpArgs -contains '--help' -or $UpArgs -contains '-h') {
    $inHeader = $true
    foreach ($line in (Get-Content (Join-Path $Dir 'start.ps1') -Encoding UTF8)) {
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
# 注意：不用 $ErrorActionPreference='Stop' 统一拦截——本机 PS5 会把原生命令的
# stderr 输出（如 docker network create 的 "already exists"）升级为致命错误，
# 破坏 bash 版的容错语义；改为每个 docker 调用点显式检查 $LASTEXITCODE。

$Base       = 'docker-compose-services'
$Overlay    = if ($Remote) { 'remote' } else { 'prod' }
$WithLogs   = [int]$Logs.IsPresent
$BuildArg   = ''
if ($Build) { $BuildArg = '--build' }
if ($NoBuild) { $BuildArg = '--no-build' }

function Log([string]$msg) { Write-Host "[start:$Project] $msg" }
function Die([string]$msg, [int]$code = 10) {
    Write-Error "[start:$Project] FATAL: $msg" -ErrorAction Continue
    exit $code
}

# ── 基础文件校验 ─────────────────────────────────────────────────────────────
if (-not (Test-Path (Join-Path $Dir "$Base.yml")))           { Die "$Base.yml not found next to start.ps1" }
if (-not (Test-Path (Join-Path $Dir "$Base.$Overlay.yml")))  { Die "overlay $Base.$Overlay.yml not found" }

$ImageEnv = Join-Path $Dir '.image.env'
if (-not (Test-Path $ImageEnv)) { Die ".image.env not found in $Dir（生成：node scripts/update-image-env.js）" }

# ── 自动 build / image 模式检测 ──────────────────────────────────────────────
# 仓内开发：$Dir\..\..\services\file-service\Dockerfile 存在 → --build
# 远程部署：该路径不存在（目录被拷出仓） → --no-build（用 registry 镜像）
if (-not $BuildArg) {
    $dockerfile = Join-Path $Dir '..\..\services\file-service\Dockerfile'
    $BuildArg = if (Test-Path $dockerfile) { '--build' } else { '--no-build' }
}
Log "mode: $BuildArg (overlay: $Overlay, logs: $WithLogs)"

# ── env 分层加载（dotenv 格式：KEY=VALUE，# 注释，空行忽略） ─────────────────
# 对齐 bash `set -a; source` 语义：非单引号值在加载期展开 ${VAR} / $VAR
# （取 Process 环境 → 同文件前序行与先加载文件的变量可见）；单引号值不展开。
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
Import-DotEnv $ImageEnv

# ── REQUIRED 校验 ────────────────────────────────────────────────────────────
foreach ($v in @(
    'DATABASE_URL','RABBITMQ_DEFAULT_PASS','POSTGRES_PASSWORD',
    'IAM_DEV_BT','TOKEN_DEV_BT','NOTIFICATION_DEV_BT','AUDIT_DEV_BT',
    'PRUE_DEV_BT','GATEWAY_DEV_BT','FILE_DEV_BT')) {
    $val = [Environment]::GetEnvironmentVariable($v, 'Process')
    if ([string]::IsNullOrEmpty($val))        { Die "env var $v empty (set in .env)" }
    if ($val -eq '__SET_BY_OPS__')            { Die "env var $v still __SET_BY_OPS__ placeholder" }
}

foreach ($v in @(
    'IMAGE_API_GATEWAY','IMAGE_TOKEN_SERVICE','IMAGE_IAM_SERVICE',
    'IMAGE_NOTIFICATION_SERVICE','IMAGE_FILE_SERVICE','IMAGE_AUDIT_SERVICE',
    'IMAGE_PRUE_BACKEND','IMAGE_CONSUL','IMAGE_REDIS','IMAGE_RABBITMQ','IMAGE_POSTGRES',
    'IMAGE_PROMETHEUS','IMAGE_ALERTMANAGER','IMAGE_GRAFANA','IMAGE_CONSUL_TEMPLATE',
    'IMAGE_OTEL_COLLECTOR','IMAGE_JAEGER')) {
    if ([string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($v, 'Process'))) {
        Die "env var $v empty (set in .image.env)"
    }
}
if ($WithLogs -eq 1) {
    foreach ($v in @('IMAGE_LOKI','IMAGE_PROMTAIL')) {
        if ([string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($v, 'Process'))) {
            Die "env var $v empty (set in .image.env; needed by --logs)"
        }
    }
}

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
$ComposeEnvArgs = @()
foreach ($f in @((Join-Path $Dir '.env'), (Join-Path $Dir '.env.prod'), $ImageEnv)) {
    if (Test-Path $f) { $ComposeEnvArgs += @('--env-file', $f) }
}

# ── compose 文件链 ───────────────────────────────────────────────────────────
$ComposeFiles = @('-f', 'docker-compose.yml', '-f', "$Base.$Overlay.yml")
if ($WithLogs -eq 1) { $ComposeFiles += @('-f', 'docker-compose-observability.logs.yml') }

# ── 启动 ─────────────────────────────────────────────────────────────────────
$netOut = docker network create superpowers 2>&1
if ($LASTEXITCODE -ne 0 -and "$netOut" -notmatch 'already exists') {
    Die "docker network create failed: $netOut"
}

Push-Location $Dir
try {
    Log "up (overlay: $Overlay, $BuildArg)..."
    $upCmd = @('compose', '-p', $Project) + $ComposeEnvArgs + $ComposeFiles + @('up', '-d')
    if ($BuildArg) { $upCmd += $BuildArg }
    if ($UpArgs)   { $upCmd += $UpArgs }
    & docker @upCmd
    if ($LASTEXITCODE -ne 0) { Die 'up failed' 20 }
    $n = (docker compose -p $Project ps --services 2>$null | Measure-Object).Count
    Log "running: $n services — gateway :3000 | grafana :3001 | prometheus :9090 | jaeger :16686 | consul :8500"
}
finally {
    Pop-Location
}
