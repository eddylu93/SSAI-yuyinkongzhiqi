function versionParts(tag) {
  const match = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(tag)
  if (!match) throw new Error('发布标签需要使用 v主版本.次版本.修订号')
  return match.slice(1).map(BigInt)
}

export async function checkRelease(repository, installed, request = fetch) {
  const endpoint = 'https://api.github.com/repos' + new URL(repository).pathname + '/releases/latest'
  const response = await request(endpoint, {
    credentials: 'omit', referrerPolicy: 'no-referrer', cache: 'no-cache',
    signal: AbortSignal.timeout(10000)
  })
  if (response.status === 404) return {message: '暂未找到公开的正式版本'}
  if (!response.ok) throw new Error('GitHub 暂时无法访问，请稍后重试')
  const release = await response.json()
  if (release.draft || release.prerelease) return {message: '暂未找到公开的正式版本'}
  const current = versionParts(installed)
  const latest = versionParts(release.tag_name)
  const index = latest.findIndex((part, i) => part !== current[i])
  if (index >= 0 && latest[index] > current[index]) {
    return {version: release.tag_name, message: '有新版本可下载'}
  }
  return {message: '当前已是最新版本'}
}
