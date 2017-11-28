let { createHash } = require('crypto')

function sha256 (data) {
  return createHash('sha256').update(data).digest()
}

module.exports = { sha256 }
