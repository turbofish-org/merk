let old = require('old')
let Transaction = require('level-transactions')
let Node = require('./node.js')

class Tree {
  constructor (db) {
    if (!db || db.toString() !== 'LevelUP') {
      throw Error('Must specify a LevelUp interface')
    }
    this.db = db
    this._rootNode = null
    this.lock = null

    this.initialized = false
    this.initialize = this.maybeLoad()
  }

  async maybeLoad () {
    let idCounter = await getInt(this.db, ':idCounter')
    this.Node = Node(this.db, idCounter)

    let rootId = await getInt(this.db, ':root')
    if (rootId != null) {
      this._rootNode = await this.Node.get(rootId)
    }

    this.initialized = true
  }

  async rootNode () {
    await this.initialize
    return this._rootNode
  }

  rootHash () {
    if (this._rootNode == null) return null
    return this._rootNode.hash
  }

  async setRoot (node, tx) {
    await this.initialize

    if (this._rootNode != null && this._rootNode.id === node.id) {
      return
    }

    if (!tx) {
      tx = createTx(this.db)
      var createdTx = true
    }

    await this.db.put(':root', node.id)
    this._rootNode = node

    if (createdTx) {
      await tx.commit()
    }
  }

  async acquireLock () {
    while (true) {
      if (!this.lock) break
      await this.lock
    }

    let _resolve
    let releaseLock = () => {
      this.lock = null
      _resolve()
    }
    this.lock = new Promise((resolve) => {
      _resolve = resolve
    })

    return releaseLock
  }

  async put (key, value) {
    await this.initialize

    let release = await this.acquireLock()

    let tx = createTx(this.db)
    let node = new this.Node({ key, value }, tx)

    // no root, set node as root
    if (this._rootNode == null) {
      await node.save(tx)
      await this.setRoot(node, tx)
      await tx.commit()
      release()
      return
    }

    let successor = await this._rootNode.put(node, tx)
    await this.setRoot(successor, tx)
    await tx.commit()
    release()
  }

  async get (key) {
    await this.initialize

    if (this._rootNode == null) return null
    let node = await this._rootNode.search(key)
    if (node.key !== key) {
      throw Error(`Key "${key}" not found`)
    }
    return node.value
  }

  async del (key) {
    await this.initialize

    if (this._rootNode == null) {
      throw Error('Tree is empty')
    }

    let release = await this.acquireLock()

    let tx = createTx(this.db)
    let successor = await this._rootNode.delete(key, tx)
    await this.setRoot(successor, tx)
    await tx.commit()

    release()
  }
}

async function getInt (db, key) {
  try {
    let bytes = await db.get(key)
    return parseInt(bytes.toString())
  } catch (err) {
    if (err.notFound) return
    throw err
  }
}

module.exports = old(Tree)

// promsifies level-transactions methods
function createTx (db) {
  let tx = new Transaction(db)
  return {
    get: promisify(tx, 'get'),
    put: promisify(tx, 'put'),
    del: promisify(tx, 'del'),
    commit: promisify(tx, 'commit')
  }
}

function promisify (obj, method) {
  return (...args) => {
    return new Promise((resolve, reject) => {
      obj[method](...args, (err, value) => {
        if (err) return reject(err)
        resolve(value)
      })
    })
  }
}
