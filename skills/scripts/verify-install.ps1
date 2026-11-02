# PM-AI 环境自检脚本（PowerShell / Windows）——T-089 占位实现
#
# 用法：.\verify-install.ps1 -BaseUrl http://<server>:<port> [-ApiKey <key>] [-DeviceId <uuid>]
# 退出码：0 = 全部通过；1 = 有失败项（逐项打印 PASS/FAIL）。
# 依赖：PowerShell 5+（无第三方模块）。Key 只进内存与本次请求头，不落盘。
param(
    [Parameter(Mandatory = $true)][string]$BaseUrl,
    [string]$ApiKey,
    [string]$DeviceId
)

$ErrorActionPreference = 'Stop'
$BaseUrl = $BaseUrl.TrimEnd('/')
$results = @()
$accept = 'application/json, text/event-stream'

function Check([string]$Name, [scriptblock]$Block) {
    try {
        $detail = & $Block
        $script:results += [pscustomobject]@{ Name = $Name; Ok = $true; Detail = "$detail" }
        Write-Host "PASS  $Name  $detail"
    } catch {
        $script:results += [pscustomobject]@{ Name = $Name; Ok = $false; Detail = $_.Exception.Message }
        Write-Host "FAIL  $Name  $($_.Exception.Message)"
    }
}

# 1) /health（公开）
Check 'health' {
    $r = Invoke-WebRequest -Uri "$BaseUrl/health" -Method GET -UseBasicParsing -TimeoutSec 10
    if ($r.StatusCode -ne 200) { throw "status $($r.StatusCode)" }
    '200'
}

# 2) /install/{tool} 可读（公开，agent 读 JSON）
Check 'install-guide' {
    $r = Invoke-WebRequest -Uri "$BaseUrl/install/generic?format=json" -Method GET -UseBasicParsing -TimeoutSec 10
    if ($r.StatusCode -ne 200) { throw "status $($r.StatusCode)" }
    '200'
}

if ($ApiKey) {
    $headers = @{ Authorization = "Bearer $ApiKey"; Accept = $accept }

    # 3) 认证：/v1/pm/me
    Check 'authenticated' {
        $r = Invoke-WebRequest -Uri "$BaseUrl/v1/pm/me" -Method GET -Headers $headers -UseBasicParsing -TimeoutSec 10
        if ($r.StatusCode -ne 200) { throw "status $($r.StatusCode)" }
        '200'
    }
} else {
    Write-Host 'SKIP  authenticated  (no -ApiKey given)'
}

if ($ApiKey -and $DeviceId) {
    # 4) 设备可见（headers 归因通道目标存在）
    Check 'device-visible' {
        $r = Invoke-WebRequest -Uri "$BaseUrl/v1/pm/devices/$DeviceId" -Method GET -Headers $headers -UseBasicParsing -TimeoutSec 10
        if ($r.StatusCode -ne 200) { throw "status $($r.StatusCode)" }
        '200'
    }
} else {
    Write-Host 'SKIP  device-visible  (need -ApiKey and -DeviceId)'
}

$failed = @($results | Where-Object { -not $_.Ok })
Write-Host ("verify-install: {0} passed, {1} failed" -f ($results.Count - $failed.Count), $failed.Count)
if ($failed.Count -gt 0) { exit 1 } else { exit 0 }
