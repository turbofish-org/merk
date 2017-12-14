let struct = require('varstruct')
let VarInt = require('varint')
let Transaction = require('level-transactions')
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
  leftHeight: 0,
  rightHeight: 0,
  leftId: 0,
  rightId: 0,
  parentId: 0
}

const nullNode = Object.assign({
  height: () => 1,
  async save () {}
}, defaults)

function nodeIdKey (id) {
  return `n${id}`
}

// promsifies level-transactions methods
function createTx (db) {
  let tx = Transaction(db)

  function promisify (method) {
    return (...args) => {
      return new Promise((resolve, reject) => {
        tx[method](...args, (err, value) => {
          if (err) return reject(err)
          resolve(value)
        })
      })
    }
  }

  return {
    get: promisify('get'),
    put: promisify('put'),
    del: promisify('del'),
    commit: promisify('commit')
  }
}

function createReadOnlyTx (db) {
  return {
    async get (key) {
      let nodeBytes = await db.get(key)
      return Buffer.from(nodeBytes.toString(), 'base64')
    },
    put () { throw Error('Tried to "put" on read-only tx') },
    del () { throw Error('Tried to "del" on read-only tx') }
  }
}

module.exports = function (db, idCounter = 1) {
  function nextID (tx) {
    let id = idCounter++
    tx.put(':idCounter', idCounter)
    return id
  }

  async function getNode (tx, id) {
    if (id === 0) return null
    let nodeBytes = await tx.get(nodeIdKey(id))
    let decoded = codec.decode(Buffer.from(nodeBytes, 'base64'))
    decoded.id = id
    return new Node(decoded)
  }

  function putNode (tx, node) {
    let nodeBytes = codec.encode(node).toString('base64')
    tx.put(nodeIdKey(node.id), nodeBytes)
  }

  function delNode (tx, node) {
    tx.del(nodeIdKey(node.id))
  }

  class Node {
    constructor (props, tx) {
      if (props.key == null) {
        throw new Error('Key is required')
      }
      if (props.value == null) {
        throw new Error('Value is required')
      }

      Object.assign(this, defaults, props)

      if (this.id === 0) {
        this.id = nextID(tx)
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

    left (tx) {
      return getNode(tx, this.leftId)
    }

    right (tx) {
      return getNode(tx, this.rightId)
    }

    child (tx, left) {
      if (left) return this.left(tx)
      return this.right(tx)
    }

    parent (tx) {
      return getNode(tx, this.parentId)
    }

    async save (tx) {
      if (!tx) {
        var createdTx = true
        tx = createTx(db)
      }

      putNode(tx, this)

      if (createdTx) await tx.commit()
    }

    async setChild (tx, left, child, rebalance = true) {
      if (child != null) {
        child.parentId = this.id
      } else {
        child = nullNode
      }

      this[left ? 'leftId' : 'rightId'] = child.id
      this[left ? 'leftHeight' : 'rightHeight'] = child.height()

      if (rebalance && Math.abs(this.balance()) > 1) {
        return this.rebalance(tx)
      }

      let leftChild = left ? child : await this.left(tx)
      let rightChild = !left ? child : await this.right(tx)
      this.calculateHashSync(leftChild, rightChild)

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
      child.parentId = 0
      await child.setChild(tx, !left, this, false)
      return child
    }

    height () {
      return Math.max(this.leftHeight, this.rightHeight) + 1
    }

    async calculateHash (tx) {
      let leftChild = await this.left(tx)
      let rightChild = await this.right(tx)
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

    async search (key, tx) {
      // found key match
      if (key === this.key) return this

      if (!tx) tx = createReadOnlyTx(db)

      // recurse through left child if key is < this.key,
      // otherwise recurse through right
      let left = key < this.key
      let child = await this.child(tx, left)
      // if we don't have a child for this side, return self
      if (child == null) return this
      return child.search(key, tx)
    }

    async put (node, tx) {
      if (!tx) {
        var createdTx = true
        tx = createTx(db)
      }

      async function done (res, err) {
        if (createdTx) {
          if (err) tx.rollback()
          else await tx.commit()
        }
        if (err) throw err
        return res
      }

      if (node.key === this.key) {
        // same key, just update the value of this node
        this.value = node.value
        this.calculateKVHash()
        await this.save(tx)
        return done(this)
      }

      let left = node.key < this.key
      let child = await this.child(tx, left)
      if (child == null) {
        // no child here, set node as child
        let successor = await this.setChild(tx, left, node)
        return done(successor)
      }

      // recursively put node under child, then update self
      let newChild = await child.put(node, tx)
      let successor = await this.setChild(tx, left, newChild)
      return done(successor)
    }

    async delete (key, tx) {
      if (!tx) {
        var createdTx = true
        tx = createTx(db)
      }

      async function done (res, err) {
        if (createdTx) {
          if (err) tx.rollback()
          else await tx.commit()
        }
        if (err) throw err
        return res
      }

      if (key === this.key) {
        // delete this node

        if (this.isLeafNode()) {
          // no children
          delNode(tx, this)
          return done(null)
        }

        // promote successor child to this position
        let left = this.leftHeight > this.rightHeight
        let successor = await this.child(tx, left)
        let otherNode = await this.child(tx, !left)
        if (otherNode != null) {
          // if there is another child then put it under successor
          await successor.put(tx, otherNode)
        }
        successor.parentId = this.parentId
        delNode(tx, this)
        return done(successor)
      }

      let left = key < this.key
      let child = await this.child(tx, left)
      if (child == null) {
        // no child here, key not found
        return done(null, Error(`Key "${key}" not found`))
      }

      let newChild = await child.delete(key, tx)
      let successor = await this.setChild(tx, left, newChild)
      return done(successor)
    }

    async edge (left, tx) {
      if (!tx) tx = createReadOnlyTx(db)
      let cursor = this
      while (true) {
        let child = await cursor.child(tx, left)
        if (child == null) return cursor
        cursor = child
      }
    }
    min () { return this.edge(true) }
    max () { return this.edge(false) }

    async step (left) {
      let tx = createReadOnlyTx(db)
      let child = await this.child(tx, left)
      if (child) return child.edge(!left, tx)

      // backtrack
      let cursor = await this.parent(tx)
      while (cursor) {
        let skip = left ?
          cursor.key > this.key :
          cursor.key < this.key
        if (!skip) return cursor
        cursor = await cursor.parent(tx)
      }

      // reached end
      return null
    }
    prev () { return this.step(true) }
    next () { return this.step(false) }
  }

  Object.assign(Node, {
    get: (id) => getNode(createReadOnlyTx(db), id)
  })

  return Node
}

module.exports.createTx = createTx
