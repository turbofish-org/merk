let { createHash } = require('crypto')
let struct = require('varstruct')
let VarInt = require('varint')

// sha512-256
function hash (data) {
  return createHash('sha512')
    .update(data)
    .digest()
    .slice(0, 32)
}

let field = (name, type) => ({ name, type })

let InnerNode = struct([
  field('hash', struct.Buffer(32)),
  field('balance', struct.Int8),
  field('left', VarInt),
  field('right', VarInt)
])

let LeafNode = struct([
  field('hash', struct.Buffer(32)),
  field('key', struct.VarString(VarInt)),
  field('value', struct.VarString(VarInt))
])

module.exports =
class MerkleAVLTree {
  constructor (db) {
    if (db == null) {
      throw Error('Must specify a LevelUp interface')
    }
    this.db = db
  }

  async rootHash () {
    return (await this.rootNode()).hash
  }
}
