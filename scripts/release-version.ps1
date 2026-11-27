param(
    [Parameter(Mandatory)] [string]$Commit,
    [string]$RequestedVersion
)

$ErrorActionPreference = 'Stop'
$versionPattern = '(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)'
$tags = @(git tag --list 'v*' --sort=-version:refname)
if ($LASTEXITCODE -ne 0) { throw 'Could not list version tags' }
$tags = @($tags | Where-Object { $_ -cmatch "^v$versionPattern$" })
$taggedHere = @(git tag --points-at $Commit)
if ($LASTEXITCODE -ne 0) { throw "Could not find tags for $Commit" }
$existingTag = $tags | Where-Object { $_ -cin $taggedHere } | Select-Object -First 1

if ($RequestedVersion) {
    if ($RequestedVersion -cnotmatch "^$versionPattern$") { throw 'Use MAJOR.MINOR.PATCH, for example 2.0.0' }
    $version = [version]$RequestedVersion
    $tag = "v$version"
    if ($tag -cnotin $taggedHere) {
        $main = git rev-parse refs/remotes/origin/main
        if ($LASTEXITCODE -ne 0 -or $main -ne $Commit) { throw 'Run a manual version bump on the current main' }
        if ($tags.Count -gt 0 -and $version -le [version]$tags[0].Substring(1)) {
            throw "The requested version must be greater than $($tags[0])"
        }
    }
} elseif ($existingTag) {
    $version = [version]$existingTag.Substring(1)
} elseif ($tags.Count -gt 0) {
    $previous = [version]$tags[0].Substring(1)
    $version = [version]::new($previous.Major, $previous.Minor, ($previous.Build + 1))
} else {
    $version = [version]((Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value)
}

if ($version.Major -gt 65535 -or $version.Minor -gt 65535 -or $version.Build -lt 0 -or $version.Build -gt 65535) {
    throw 'The Windows version fields must be between 0 and 65535'
}

# A run for an older merge can reach this job after a newer merge has published.
$descendants = @(git tag --contains $Commit)
if ($LASTEXITCODE -ne 0) { throw "Could not inspect the history of $Commit" }
$hasNewerBuild = $tags | Where-Object { $_ -cin $descendants -and $_ -cnotin $taggedHere } | Select-Object -First 1
[pscustomobject]@{
    Version = $version.ToString(3)
    TagExists = "v$version" -cin $taggedHere
    Latest = -not $hasNewerBuild
}
