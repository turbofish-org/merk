let struct = require('varstruct')
let match = require('varstruct-match')
let VarInt = require('varint')
let { sha256 } = require('./common.js')

const nullHash = Buffer(32).fill(0)

let VarString = struct.VarString(VarInt)
let codec = struct([
  ['hash', struct.Buffer(32)],
  ['kvHash', struct.Buffer(32)],
  ['leftHeight', struct.UInt8],
  ['rightHeight', struct.UInt8],
  ['key', VarString],
  ['value', VarString],
  ['leftId', VarInt],
  ['rightId', VarInt],
  ['parentId', VarInt]
])

const defaults = {
  id: 0,
  hash: nullHash,
  kvHash: nullHash,
  balance: 0,
  leftHeight: 0,
  rightHeight: 0,
  leftId: 0,
  rightId: 0,
  parentId: 0
}

const nullNode = Object.assign(
  { height: () => 1 },
  defaults)

function nodeIdKey (id) {
  return `n${id}`
}

module.exports = function (db) {
  let idCounter = 1
  function nextID () {
    return idCounter++
  }

  async function getNode (id) {
    if (id === 0) return null
    let nodeBytes = await db.get(nodeIdKey(id))
    let decoded = codec.decode(nodeBytes)
    decoded.id = id
    return new Node(decoded)
  }

  async function putNode (node) {
    let nodeBytes = codec.encode(node)
    await db.put(nodeIdKey(node.id), nodeBytes)
    if (node.id === idCounter - 1) {
      await db.put(':idCounter', idCounter)
    }
  }

  async function delNode (node) {
    await db.del(nodeIdKey(node.id))
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

      if (this.id === 0) {
        this.id = nextID()
      }
      if (this.kvHash.equals(nullHash)) {
        this.calculateKVHash()
      }
      if (this.hash.equals(nullHash)) {
        this.calculateHashSync()
      }
    }

    isInnerNode () {
      return this.leftId !== 0 || this.rightId !== 0
    }

    isLeafNode () {
      return !this.isInnerNode()
    }

    left () {
      return getNode(this.leftId)
    }

    right () {
      return getNode(this.rightId)
    }

    child (left) {
      if (left) return this.left()
      return this.right()
    }

    parent () {
      return getNode(this.parentId)
    }

    save () {
      return putNode(this)
    }

    async setChild (left, child) {
      if (child != null) {
        child.parentId = this.id
      } else {
        child = nullNode
      }

      this[left ? 'leftId' : 'rightId'] = child.id
      this[left ? 'leftHeight' : 'rightHeight'] = child.height()
      let balance = this.rightHeight - this.leftHeight
      if (Math.abs(balance) > 1) {
        // TODO: rebalance
        console.log('should balance')
      }

      let leftChild = left ? child : await this.left()
      let rightChild = !left ? child : await this.right()
      this.calculateHashSync(leftChild, rightChild)

      await this.save()
      return this
    }

    height () {
      return Math.max(this.leftHeight, this.rightHeight) + 1
    }

    async calculateHash () {
      let leftChild = await this.left()
      let rightChild = await this.right()
      return calculateHashSync(leftChild, rightChild)
    }

    calculateHashSync (leftChild, rightChild) {
      let input = Buffer.concat([
        leftChild ? leftChild.hash : nullHash,
        rightChild ? rightChild.hash : nullHash,
        this.kvHash
      ])
      return this.hash = sha256(input)
    }

    calculateKVHash () {
      let input = Buffer.concat([
        VarString.encode(this.key),
        VarString.encode(this.value)
      ])
      return this.kvHash = sha256(input)
    }

    async search (key) {
      // found key match
      if (key === this.key) return this

      // recurse through left child if key is < this.key,
      // otherwise recurse through right
      let left = key < this.key
      let child = await this.child(left)
      // if we don't have a child for this side, return self
      if (child == null) return this
      return child.search(key)
    }

    put (key, value) {
      let node = new Node({ key, value })
      return this.putNode(node)
    }

    async putNode (node) {
      if (node.key === this.key) {
        throw Error(`Duplicate key "${node.key}"`)
      }

      let left = node.key < this.key
      let child = await this.child(left)
      if (child == null) {
        // no child here, set node as child
        let successor = await this.setChild(left, node)
        await node.save()
        return successor
      }

      // recursively put node under child, then update self
      let newChild = await child.putNode(node)
      let successor = await this.setChild(left, newChild)
      return successor
    }

    async delete (key) {
      if (key === this.key) {
        // delete this node

        if (this.isLeafNode()) {
          // no children
          await delNode(this)
          return null
        }

        // promote successor child to this position
        let left = this.leftHeight > this.rightHeight
        let successor = await this.child(left)
        let otherNode = await this.child(!left)
        if (otherNode != null) {
          // if there is another child then put it under successor
          await successor.putNode(otherNode)
        }
        successor.parentId = this.parentId
        await delNode(this)
        return successor
      }

      let left = key < this.key
      let child = await this.child(left)
      if (child == null) {
        // no child here, key not found
        throw Error(`Key "${key}" not found`)
      }

      let newChild = await child.delete(key)
      let successor = await this.setChild(left, newChild)
      return successor
    }

    async edge (left) {
      let cursor = this
      while (true) {
        let child = await cursor.child(left)
        if (child == null) return cursor
        cursor = child
      }
    }
    min () { return this.edge(true) }
    max () { return this.edge(false) }
  }

  return Node
}
