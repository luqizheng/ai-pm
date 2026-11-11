# start.ps1 —— 全栈部署单元启动（infra + observability + lmms services + ai-pm）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   .\start.ps1               # 构建前端 dist + ai-pm 四服务镜像，然后 up（默认 prod 片）
#   .\start.ps1 --no-build    # 跳过全部构建，直接 up（沿用现有镜像与 dist）
#   .\start.ps1 --no-fe-build # 跳过 pnpm dist 构建（dist 已是最新时用），仍构建 ai-pm 镜像
#   .\start.ps1 --remote      # 跨主机分置片（保留业务端口供 consul 回拨）
#   .\start.ps1 --logs        # 叠加日志栈（loki + promtail）
#   .\start.ps1 --force-recreate  # 其余参数透传 docker compose up
#
# 构建口径（2026-11-11 重写，替换旧 file-service 存在性探测）：
#   - 仅 ai-pm 四服务本地构建：pm-api / pm-worker / ai-pm-worker / ai-frontend-web，
#     共用仓库根 Dockerfile 按 target 出镜像（见 docker-compose.ai-pm.yml）。
#   - ai-frontend-web 前置：ai-frontend/dist 须先在宿主产出（pnpm build），
#     根 Dockerfile 只装 nginx。默认自动执行；--no-fe-build 或 --no-build 跳过。
#   - 其余 lmms 服务一律用 .image.env 的 ${IMAGE_*} 镜像，不构建。
#
# env 加载（后者覆盖前者）：
#   1. .env       —— 环境配置（IP / 口令 / 拓扑）
#                  向上回退：$DIR\..\..\.env → $DIR\..\.env → $DIR\.env
#   2. .env.prod  —— 运维覆盖层（可选；缺失不阻塞）
#   3. .image.env —— **镜像钉版**，最后加载 → 权威
#
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；缺变量直接报错。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [switch]$Remote,
    [switch]$Prod,
    [switch]$Logs,
    [switch]$NoBuild,
    [switch]$NoFeBuild,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$UpArgs
)

$Script:Dir      = Split-Path -Parent $MyInvocation.MyCommand.Path
$Script:RepoRoot = (Resolve-Path (Join-Path $Dir '..\..')).Path
$Project         = 'superpowers-full'

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
# 破坏 bash 版的容错语义；改为每个调用点显式检查 $LASTEXITCODE。

# ── 双横线参数归一 ───────────────────────────────────────────────────────────
# PowerShell 只识别单横线开关（-NoBuild）；习惯上写的 --no-build 会落进
# $UpArgs 透传给 compose 造成歧义。这里显式归一：认识的开关就地生效并从
# $UpArgs 剔除，其余继续透传。
if ($UpArgs) {
    $SwitchMap = @{
        '--remote'      = 'Remote'
        '--prod'        = 'Prod'
        '--logs'        = 'Logs'
        '--no-build'    = 'NoBuild'
        '--no-fe-build' = 'NoFeBuild'
    }
    $keep = @()
    foreach ($a in $UpArgs) {
        $k = "$a".ToLowerInvariant()
        if ($SwitchMap.ContainsKey($k)) {
            Set-Variable -Name $SwitchMap[$k] -Value $true -Scope Script
            Log "flag: $k"
        }
        else {
            $keep += $a
        }
    }
    $UpArgs = $keep
}

$Overlay = if ($Remote) { 'remote' } else { 'prod' }
# 仅 ai-pm 自有服务需要本地构建（docker-compose.ai-pm.yml 定义）
$AiPmServices = @('pm-api', 'pm-worker', 'ai-pm-worker', 'ai-frontend-web')

function Log([string]$msg) { Write-Host "[start:$Project] $msg" }
function Die([string]$msg, [int]$code = 10) {
    Write-Error "[start:$Project] FATAL: $msg" -ErrorAction Continue
    exit $code
}

# ── 基础文件校验 ─────────────────────────────────────────────────────────────
if (-not (Test-Path (Join-Path $Dir 'docker-compose.yml')))                    { Die 'docker-compose.yml not found next to start.ps1' }
if (-not (Test-Path (Join-Path $Dir "docker-compose-services.$Overlay.yml"))) { Die "overlay docker-compose-services.$Overlay.yml not found" }
if (-not (Test-Path (Join-Path $Dir '.image.env')))                            { Die ".image.env not found in $Dir" }
if (-not (Test-Path (Join-Path $RepoRoot 'Dockerfile')))                       { Die "repo-root Dockerfile not found: $RepoRoot" }

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
Import-DotEnv (Join-Path $Dir '.image.env')

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
if ($Logs.IsPresent) {
    foreach ($v in @('IMAGE_LOKI','IMAGE_PROMTAIL')) {
        if ([string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($v, 'Process'))) {
            Die "env var $v empty (set in .image.env; needed by --logs)"
        }
    }
}

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
$ComposeEnvArgs = @()
foreach ($f in @((Join-Path $Dir '.env'), (Join-Path $Dir '.env.prod'), (Join-Path $Dir '.image.env'))) {
    if (Test-Path $f) { $ComposeEnvArgs += @('--env-file', $f) }
}

# ── compose 文件链 ───────────────────────────────────────────────────────────
# docker-compose.yml 已 include infra / observability / services / ai-pm 四片；
# overlay 叠加在顶层覆盖业务服务端口与拓扑。
$ComposeFiles = @('-f', 'docker-compose.yml', '-f', "docker-compose-services.$Overlay.yml")
if ($Logs.IsPresent) { $ComposeFiles += @('-f', 'docker-compose-observability.logs.yml') }

# ── 前端 dist 构建（ai-frontend-web 的前置产物） ─────────────────────────────
if (-not $NoBuild -and -not $NoFeBuild) {
    $FeDir = Join-Path $RepoRoot 'ai-frontend'
    if (-not (Test-Path (Join-Path $FeDir 'package.json'))) { Die "ai-frontend not found: $FeDir" }
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
        Die 'pnpm not found in PATH（前端构建需要 node>=22 / pnpm>=11；或用 --no-fe-build 跳过）'
    }
    Log "frontend dist build (pnpm build)..."
    Push-Location $FeDir
    try {
        & pnpm build
        if ($LASTEXITCODE -ne 0) { Die 'pnpm build failed' 15 }
    }
    finally {
        Pop-Location
    }
}

# ── ai-pm 镜像构建（仅四服务；其余服务全走 .image.env 镜像） ─────────────────
if (-not $NoBuild) {
    Log "build ai-pm images: $($AiPmServices -join ', ')"
    $buildCmd = @('compose', '-p', $Project) + $ComposeEnvArgs + $ComposeFiles + @('build') + $AiPmServices
    & docker @buildCmd
    if ($LASTEXITCODE -ne 0) { Die 'docker compose build failed' 15 }
}
else {
    Log 'build skipped (--no-build)'
}

# ── 启动 ─────────────────────────────────────────────────────────────────────
$netOut = docker network create superpowers 2>&1
if ($LASTEXITCODE -ne 0 -and "$netOut" -notmatch 'already exists') {
    Die "docker network create failed: $netOut"
}

Push-Location $Dir
try {
    Log "up (overlay: $Overlay)..."
    $upCmd = @('compose', '-p', $Project) + $ComposeEnvArgs + $ComposeFiles + @('up', '-d')
    if ($UpArgs) { $upCmd += $UpArgs }
    & docker @upCmd
    if ($LASTEXITCODE -ne 0) { Die 'up failed' 20 }
    $n = (docker compose -p $Project ps --services 2>$null | Measure-Object).Count
    Log "running: $n services — frontend :8092 | gateway :3000 | grafana :3001 | prometheus :9090 | jaeger :16686 | consul :8500"
}
finally {
    Pop-Location
}
