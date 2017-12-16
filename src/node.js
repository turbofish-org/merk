let struct = require('varstruct')
let VarInt = require('varint')
let { sha256 } = require('./common.js')

const nullHash = Buffer.alloc(32)

let VarString = struct.VarString(VarInt)
let codec = struct([
  ['hash', struct.Buffer(32)],
  ['kvHash', struct.Buffer(32)],
  ['leftHeight', struct.UInt8],
  ['rightHeight', struct.UInt8],
  ['value', VarString],
  ['leftKey', VarString],
  ['rightKey', VarString],
  ['parentKey', VarString]
])

const defaults = {
  hash: nullHash,
  kvHash: nullHash,
  leftHeight: 0,
  rightHeight: 0,
  leftKey: '',
  rightKey: '',
  parentKey: '',
  key: ''
}

const nullNode = Object.assign({
  height: () => 1,
  async save () {}
}, defaults)

function nodeKey (key) {
  return 'n' + key
}

function putNode (tx, node) {
  let nodeBytes = codec.encode(node).toString('base64')
  return tx.put(nodeKey(node.key), nodeBytes)
}

function delNode (tx, node) {
  return tx.del(nodeKey(node.key))
}

module.exports = function (db) {
  async function getNode (tx, key) {
    if (key === '') return null
    let nodeB64 = (await tx.get(nodeKey(key))).toString()
    let nodeBytes = Buffer.from(nodeB64, 'base64')
    let decoded = codec.decode(nodeBytes)
    decoded.key = key
    return new Node(decoded)
  }

  class Node {
    constructor (props) {
      if (props.key == null) {
        throw new Error('Key is required')
      }
      if (props.value == null) {
        throw new Error('Value is required')
      }

      Object.assign(this, defaults, props)

      if (this.kvHash.equals(nullHash)) {
        this.calculateKVHash()
      }
      if (this.hash.equals(nullHash)) {
        this.calculateHash()
      }
    }

    isInnerNode () {
      return this.leftKey || this.rightKey
    }

    isLeafNode () {
      return !this.isInnerNode()
    }

    left (tx) {
      return getNode(tx, this.leftKey)
    }

    right (tx) {
      return getNode(tx, this.rightKey)
    }

    child (tx, left) {
      if (left) return this.left(tx)
      return this.right(tx)
    }

    parent (tx) {
      return getNode(tx, this.parentKey)
    }

    save (tx) {
      return putNode(tx, this)
    }

    async setChild (tx, left, child, rebalance = true) {
      if (child != null) {
        child.parentKey = this.key
      } else {
        child = nullNode
      }

      this[left ? 'leftKey' : 'rightKey'] = child.key
      this[left ? 'leftHeight' : 'rightHeight'] = child.height()

      if (rebalance && Math.abs(this.balance()) > 1) {
        return this.rebalance(tx)
      }

      let leftChild = left ? child : await this.left(tx)
      let rightChild = !left ? child : await this.right(tx)
      this.calculateHash(leftChild, rightChild)

      await this.save(tx)
      await child.save(tx)
      return this
    }

    balance () {
      return this.rightHeight - this.leftHeight
    }

    async rebalance (tx) {
      let left = this.balance() < 0
      let child = await this.child(tx, left)

      // check if we should do a double rotation
      let childLeftHeavy = child.balance() < 0
      let childRightHeavy = child.balance() > 0
      let double = left ? childRightHeavy : childLeftHeavy
      if (double) {
        let successor = await child.rotate(tx, !left)
        await this.setChild(tx, left, successor, false)
      }
      return this.rotate(tx, left)
    }

    async rotate (tx, left) {
      let child = await this.child(tx, left)
      let grandChild = await child.child(tx, !left)
      await this.setChild(tx, left, grandChild, false)
      child.parentKey = ''
      await child.setChild(tx, !left, this, false)
      return child
    }

    height () {
      return Math.max(this.leftHeight, this.rightHeight) + 1
    }

    calculateHash (leftChild, rightChild) {
      let input = Buffer.concat([
        leftChild ? leftChild.hash : nullHash,
        rightChild ? rightChild.hash : nullHash,
        this.kvHash
      ])
      this.hash = sha256(input)
      return this.hash
    }

    calculateKVHash () {
      let input = Buffer.concat([
        VarString.encode(this.key),
        VarString.encode(this.value)
      ])
      this.kvHash = sha256(input)
      return this.kvHash
    }

    async put (node, tx) {
      if (node.key === this.key) {
        // same key, just update the value of this node
        this.value = node.value
        this.calculateKVHash()
        await this.save(tx)
        return this
      }

      let left = node.key < this.key
      let child = await this.child(tx, left)
      if (child == null) {
        // no child here, set node as child
        let successor = await this.setChild(tx, left, node)
        return successor
      }

      // recursively put node under child, then update self
      let newChild = await child.put(node, tx)
      let successor = await this.setChild(tx, left, newChild)
      return successor
    }

    async delete (key, tx) {
      if (key === this.key) {
        // delete this node

        if (this.isLeafNode()) {
          // no children
          await delNode(tx, this)
          return null
        }

        // promote successor child to this position
        let left = this.leftHeight > this.rightHeight
        let successor = await this.child(tx, left)
        let otherNode = await this.child(tx, !left)
        if (otherNode != null) {
          // if there is another child then put it under successor
          await successor.put(tx, otherNode)
        }
        successor.parentKey = this.parentKey
        await delNode(tx, this)
        return successor
      }

      let left = key < this.key
      let child = await this.child(tx, left)
      if (child == null) {
        // no child here, key not found
        throw Error(`Key "${key}" not found`)
      }

      let newChild = await child.delete(key, tx)
      let successor = await this.setChild(tx, left, newChild)
      return successor
    }

    async edge (left, tx = db) {
      let cursor = this
      while (true) {
        let child = await cursor.child(tx, left)
        if (child == null) return cursor
        cursor = child
      }
    }
    min () { return this.edge(true) }
    max () { return this.edge(false) }

    async step (left, tx = db) {
      let child = await this.child(tx, left)
      if (child) return child.edge(!left, tx)

      // backtrack
      let cursor = await this.parent(tx)
      while (cursor) {
        let skip = left
          ? cursor.key > this.key
          : cursor.key < this.key
        if (!skip) return cursor
        cursor = await cursor.parent(tx)
      }

      // reached end
      return null
    }
    prev () { return this.step(true) }
    next () { return this.step(false) }
  }

  Node.get = (key) => getNode(db, key)
  return Node
}
