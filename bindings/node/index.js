/* eslint-disable */
/* @memvid/node platform loader — auto-selects the correct native binary */

const { existsSync, readFileSync } = require('fs')
const { join } = require('path')

const { platform, arch } = process

let nativeBinding = null
let loadError = null

function isMusl() {
  if (!process.report || typeof process.report.getReport !== 'function') {
    try {
      const lddPath = require('child_process')
        .execSync('which ldd')
        .toString()
        .trim()
      return readFileSync(lddPath, 'utf8').includes('musl')
    } catch {
      return true
    }
  } else {
    const { glibcVersionRuntime } = process.report.getReport().header
    return !glibcVersionRuntime
  }
}

switch (platform) {
  case 'win32':
    switch (arch) {
      case 'x64':
        try {
          if (existsSync(join(__dirname, 'memvid-node.win32-x64-msvc.node'))) {
            nativeBinding = require('./memvid-node.win32-x64-msvc.node')
          } else {
            nativeBinding = require('@memvid/node-win32-x64-msvc')
          }
        } catch (e) {
          loadError = e
        }
        break
      default:
        throw new Error(`Unsupported architecture on Windows: ${arch}`)
    }
    break
  case 'darwin':
    switch (arch) {
      case 'x64':
        try {
          if (existsSync(join(__dirname, 'memvid-node.darwin-x64.node'))) {
            nativeBinding = require('./memvid-node.darwin-x64.node')
          } else {
            nativeBinding = require('@memvid/node-darwin-x64')
          }
        } catch (e) {
          loadError = e
        }
        break
      case 'arm64':
        try {
          if (existsSync(join(__dirname, 'memvid-node.darwin-arm64.node'))) {
            nativeBinding = require('./memvid-node.darwin-arm64.node')
          } else {
            nativeBinding = require('@memvid/node-darwin-arm64')
          }
        } catch (e) {
          loadError = e
        }
        break
      default:
        throw new Error(`Unsupported architecture on macOS: ${arch}`)
    }
    break
  case 'linux':
    switch (arch) {
      case 'x64':
        if (isMusl()) {
          throw new Error(
            'Linux musl is not supported. Please use a glibc-based distribution.',
          )
        }
        try {
          if (existsSync(join(__dirname, 'memvid-node.linux-x64-gnu.node'))) {
            nativeBinding = require('./memvid-node.linux-x64-gnu.node')
          } else {
            nativeBinding = require('@memvid/node-linux-x64-gnu')
          }
        } catch (e) {
          loadError = e
        }
        break
      default:
        throw new Error(`Unsupported architecture on Linux: ${arch}`)
    }
    break
  default:
    throw new Error(`Unsupported OS: ${platform}, architecture: ${arch}`)
}

if (!nativeBinding) {
  if (loadError) {
    throw loadError
  }
  throw new Error(
    `Failed to load @memvid/node native binding for ${platform}-${arch}.\n` +
      'No prebuilt binary found. Install the Rust toolchain and run: npm run build',
  )
}

module.exports = nativeBinding
