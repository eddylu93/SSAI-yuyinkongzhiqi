import test from 'node:test'
import assert from 'node:assert/strict'
import {checkRelease} from '../src/releases.js'

const repository = 'https://github.com/eddylu93/SSAI-yuyinkongzhiqi'
function check(body, status = 200) {
  return checkRelease(repository, '0.8.0', async (url) => {
    assert.equal(url, 'https://api.github.com/repos/eddylu93/SSAI-yuyinkongzhiqi/releases/latest')
    return Response.json(body, {status})
  })
}

test('compares numeric release tags and handles older or identical builds', async () => {
  assert.equal((await check({tag_name:'v0.10.0'})).version, 'v0.10.0')
  for (const tag_name of ['0.8.0', 'v0.7.99']) {
    assert.equal((await check({tag_name})).message, '当前已是最新版本')
  }
})

test('handles missing releases, previews and failed requests', async () => {
  assert.match((await check({}, 404)).message, /暂未/)
  assert.equal((await check({prerelease:true, tag_name:'v1.0.0-beta'})).version, undefined)
  await assert.rejects(check({}, 403))
  await assert.rejects(check({tag_name:'latest'}))
})
