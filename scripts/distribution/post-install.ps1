# Build the installed system libraries and their offline API documentation.
$ErrorActionPreference = 'Stop'
$scoopRoot = $PSScriptRoot
$scoopPlan = Join-Path $scoopRoot 'share/scoop/install'
$scoopTargets = @()
$scoopAllTargets = $false
$scoopJobs = 6
$scoopWork = Join-Path $scoopRoot 'build/post-install'
$scoopCc = ''
$scoopNativeSysroot = ''

for ($scoopIndex = 0; $scoopIndex -lt $args.Count; $scoopIndex++) {
    $scoopOption = $args[$scoopIndex]
    switch ($scoopOption) {
        { $_ -in '--target', '--jobs', '--work-dir', '--cc', '--native-sysroot' } {
            $scoopIndex++
            if ($scoopIndex -ge $args.Count -or [string]::IsNullOrEmpty($args[$scoopIndex])) {
                throw "$scoopOption requires a value"
            }
            $scoopValue = $args[$scoopIndex]
            switch ($scoopOption) {
                '--target' {
                    if ($scoopValue -cnotin $scoopTargets) { $scoopTargets += $scoopValue }
                }
                '--jobs' {
                    if (-not [int]::TryParse($scoopValue, [ref]$scoopJobs) -or $scoopJobs -lt 1) {
                        throw '--jobs must be positive'
                    }
                }
                '--work-dir' { $scoopWork = $scoopValue }
                '--cc' { $scoopCc = $scoopValue }
                '--native-sysroot' { $scoopNativeSysroot = $scoopValue }
            }
        }
        '--all-targets' { $scoopAllTargets = $true }
        { $_ -in '--help', '-h' } {
            Write-Output @'
Usage: & ./post-install.ps1 [options]
Build release system libraries and offline API documentation after extraction.

  --target TARGET        Prepare a bundled target (repeatable)
  --all-targets          Prepare every bundled target; requires their SDKs
  --jobs N               Maximum parallel build jobs (default: 6)
  --work-dir PATH        Build cache and intermediates (default: build/post-install)
  --cc PATH              C compiler for the selected targets
  --native-sysroot PATH  Native SDK for the selected targets

With no target option, prepare the package's default target.
'@
            exit 0
        }
        default { throw "unknown option: $scoopOption (see --help)" }
    }
}

$scoopAvailable = @(Get-Content -LiteralPath "$scoopPlan/targets.list")
if ($scoopAllTargets) {
    if ($scoopTargets.Count -ne 0) { throw '--target and --all-targets are mutually exclusive' }
    $scoopTargets = $scoopAvailable
} elseif ($scoopTargets.Count -eq 0) {
    $scoopTargets = @($scoopAvailable[0])
}
foreach ($scoopTarget in $scoopTargets) {
    if ($scoopTarget -cnotin $scoopAvailable) {
        throw "target $scoopTarget is not included in this installation"
    }
}

if (-not [IO.Path]::IsPathRooted($scoopWork)) {
    $scoopWork = Join-Path (Get-Location).Path $scoopWork
}
[IO.Directory]::CreateDirectory($scoopWork) | Out-Null
$scoopWork = (Resolve-Path -LiteralPath $scoopWork).Path
$scoopScratch = Join-Path $scoopWork 'scratch'
[IO.Directory]::CreateDirectory($scoopScratch) | Out-Null
$scoopPreviousTmpdir = $env:TMPDIR
$scoopPreviousTmp = $env:TMP
$scoopPreviousTemp = $env:TEMP
try {
    $env:TMPDIR = $scoopScratch
    $env:TMP = $scoopScratch
    $env:TEMP = $scoopScratch
    foreach ($scoopTarget in $scoopTargets) {
        $scoopTargetCc = $scoopCc
        $scoopTargetSysroot = $scoopNativeSysroot
        if ($scoopTarget -like '*-llvm-mingw-*') {
            $scoopPrefix = Join-Path $scoopRoot "lib/$scoopTarget/toolchain"
            if (-not $scoopTargetCc) { $scoopTargetCc = Join-Path $scoopPrefix 'bin/clang.exe' }
            if (-not $scoopTargetSysroot) { $scoopTargetSysroot = $scoopPrefix }
        } elseif (-not $scoopTargetCc) {
            $scoopTargetCc = Join-Path $scoopRoot 'libexec/llvm/bin/clang-cl.exe'
        }
        $scoopNative = @('--cc', $scoopTargetCc)
        if ($scoopTargetSysroot) { $scoopNative += @('--native-sysroot', $scoopTargetSysroot) }
        $scoopCones = @(Get-Content -LiteralPath "$scoopPlan/$scoopTarget.list")
        [IO.Directory]::CreateDirectory("$scoopRoot/lib/$scoopTarget") | Out-Null
        foreach ($scoopCone in $scoopCones) {
            Write-Output "Building $scoopCone for $scoopTarget"
            $scoopCommand = @(
                'build', "$scoopRoot/src/$scoopCone",
                '--release', '--jobs', $scoopJobs, '--target', $scoopTarget,
                '--sysroot', $scoopRoot, '--scoopc', "$scoopRoot/bin/scoopc.exe",
                '--cache-dir', "$scoopWork/cache",
                '--target-dir', "$scoopWork/$scoopTarget/target/$scoopCone",
                '--lockfile', "$scoopWork/$scoopTarget/locks/$scoopCone.lock",
                '-o', "$scoopRoot/lib/$scoopTarget/$scoopCone.slib",
                '--unwind-prefix', "$scoopRoot/lib/$scoopTarget/unwind"
            ) + $scoopNative
            & "$scoopRoot/bin/sco.exe" @scoopCommand
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        }

        Write-Output "Generating API documentation for $scoopTarget"
        $scoopCommand = @('doc') + @(
            foreach ($scoopCone in $scoopCones) { "$scoopRoot/src/$scoopCone/Cone.toml" }
        ) + @(
            '--jobs', $scoopJobs, '--target', $scoopTarget,
            '--sysroot', $scoopRoot, '--scoopc', "$scoopRoot/bin/scoopc.exe",
            '--no-deps', '--deny-doc-warnings', '--cache-dir', "$scoopWork/cache",
            '--target-dir', "$scoopWork/doc-target", '--output-dir', "$scoopRoot/docs/$scoopTarget"
        ) + $scoopNative
        & "$scoopRoot/bin/sco.exe" @scoopCommand
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }

    $scoopLinks = foreach ($scoopTarget in $scoopAvailable) {
        if (Test-Path -LiteralPath "$scoopRoot/docs/$scoopTarget/index.html") {
            '<li><a href="{0}/index.html">{0}</a></li>' -f $scoopTarget
        }
    }
    $scoopHtml = @'
<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Scoop built-in API documentation</title></head>
<body><main><h1>Scoop built-in API documentation</h1>
<p>Select the target used to compile your application.
All pages, source text, and search resources are included locally.</p><ul>
'@ + "`n" + ($scoopLinks -join "`n") + "`n</ul></main></body></html>`n"
    Set-Content -LiteralPath "$scoopRoot/docs/index.html" -Value $scoopHtml -Encoding UTF8
    Write-Output "System libraries and API documentation are ready in $scoopRoot"
} finally {
    $env:TMPDIR = $scoopPreviousTmpdir
    $env:TMP = $scoopPreviousTmp
    $env:TEMP = $scoopPreviousTemp
}
