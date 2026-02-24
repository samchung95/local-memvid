/**
 * Smoke test for @memvid/node — verifies create, put, search, close cycle.
 * Run: node __test__/smoke.mjs
 */

import { createRequire } from 'module'
import { mkdtempSync, rmSync } from 'fs'
import { join } from 'path'
import { tmpdir } from 'os'

const require = createRequire(import.meta.url)
const memvid = require('../index.js')

const tmp = mkdtempSync(join(tmpdir(), 'memvid-smoke-'))
const filePath = join(tmp, 'test.mv2')

let exitCode = 0

try {
  // 1. Check version
  const ver = memvid.version()
  console.log(`version: ${ver}`)
  if (typeof ver !== 'string' || ver.length === 0) {
    throw new Error('version() returned empty or non-string')
  }

  // 2. Create a new memvid store
  const mv = memvid.JsMemvid.createSync(filePath)
  console.log(`created: ${mv.path}`)

  // 3. Put some content
  const id1 = mv.putBytesSync(Buffer.from('The quick brown fox jumps over the lazy dog'))
  const id2 = mv.putBytesSync(Buffer.from('Machine learning and artificial intelligence'))
  const id3 = mv.putBytesSync(Buffer.from('Rust is a systems programming language'))
  console.log(`put 3 frames: ${id1}, ${id2}, ${id3}`)

  // 4. Commit to disk
  mv.commitSync()
  console.log('committed')

  // 5. Search
  const results = mv.searchSync({ query: 'fox', topK: 5, snippetChars: 100 })
  console.log(`search "fox": ${results.totalHits} hit(s)`)
  if (results.totalHits < 1) {
    throw new Error('Expected at least 1 search hit for "fox"')
  }

  // 6. Read frame metadata
  const frame = mv.frameByIdSync(id1)
  console.log(`frame ${id1}: kind=${frame.kind}`)

  // 7. Frame count
  const count = mv.frameCount
  console.log(`frameCount: ${count}`)
  if (count < 3) {
    throw new Error(`Expected at least 3 frames, got ${count}`)
  }

  // 8. Close
  mv.close()
  console.log('closed')

  console.log('\n--- smoke test passed ---')
} catch (e) {
  console.error('\n--- smoke test FAILED ---')
  console.error(e)
  exitCode = 1
} finally {
  // Clean up temp directory
  try {
    rmSync(tmp, { recursive: true, force: true })
  } catch {
    // ignore cleanup errors
  }
}

process.exit(exitCode)
