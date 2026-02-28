/**
 * Postinstall script for @memvid/node.
 * If no prebuilt binary is available for this platform, attempt a source build.
 */

const { existsSync } = require('fs')
const { join } = require('path')
const { execSync } = require('child_process')

const root = join(__dirname, '..')

const platformBinaryMap = {
  'win32-x64': 'memvid-node.win32-x64-msvc.node',
  'darwin-x64': 'memvid-node.darwin-x64.node',
  'darwin-arm64': 'memvid-node.darwin-arm64.node',
  'linux-x64': 'memvid-node.linux-x64-gnu.node',
}

const platformPkgMap = {
  'win32-x64': '@memvid/node-win32-x64-msvc',
  'darwin-x64': '@memvid/node-darwin-x64',
  'darwin-arm64': '@memvid/node-darwin-arm64',
  'linux-x64': '@memvid/node-linux-x64-gnu',
}

const key = `${process.platform}-${process.arch}`
const localBinary = platformBinaryMap[key]
const platformPkg = platformPkgMap[key]

if (!localBinary) {
  console.warn(`@memvid/node: No prebuilt binary for ${key}. Skipping postinstall.`)
  process.exit(0)
}

// Check 1: local .node file exists (dev build)
if (existsSync(join(root, localBinary))) {
  process.exit(0)
}

// Check 2: platform package installed via optionalDependencies
try {
  require.resolve(platformPkg)
  process.exit(0)
} catch {
  // not installed
}

// Fallback: try to build from source
console.warn('@memvid/node: No prebuilt binary found. Attempting source build...')
console.warn('@memvid/node: This requires a working Rust toolchain (https://rustup.rs/).')
try {
  execSync('npm run build', { cwd: root, stdio: 'inherit' })
  if (existsSync(join(root, localBinary))) {
    console.warn('@memvid/node: Source build succeeded.')
  } else {
    throw new Error('Build completed but binary not found.')
  }
} catch (e) {
  console.warn('@memvid/node: Source build failed:', e.message)
  console.warn('@memvid/node: Install Rust from https://rustup.rs/ and run: npm run build')
}
