let old = require('old')
let Node = require('./node.js')

class Tree {
  constructor (db, idCounter) {
    if (db == null) {
      throw Error('Must specify a LevelUp interface')
    }
    this.db = db
    this.Node = Node(db, idCounter)
    this.rootNode = null
  }

  rootHash () {
    if (this.rootNode == null) return null
    return this.rootNode.hash
  }

  async setRoot (node) {
    if (this.rootNode != null && this.rootNode.id === node.id) {
      return
    }
    await this.db.put(':root', node.id)
    this.rootNode = node
  }

  async put (key, value) {
    let node = new this.Node({ key, value })

    // no root, set node as root
    if (this.rootNode == null) {
      await node.save()
      await this.setRoot(node)
      return
    }

    let successor = await this.rootNode.put(node)
    await this.setRoot(successor)
  }

  async get (key) {
    if (this.rootNode == null) return null
    let node = await this.rootNode.search(key)
    if (node.key !== key) {
      throw Error(`Key "${key}" not found`)
    }
    return node.value
  }

  async del (key) {
    if (this.rootNode == null) {
      throw Error('Tree is empty')
    }

    let successor = await this.rootNode.delete(key)
    await this.setRoot(successor)
  }
}

Tree.load = async function loadTree (db) {
  let idCounter = await getInt(db, ':idCounter')
  let tree = new Tree(db, idCounter)

  let rootId = await getInt(db, ':root')
  tree.rootNode = await tree.Node.get(rootId)

  return tree
}

async function getInt (db, key) {
  let bytes = await db.get(key)
  return parseInt(bytes.toString())
}

module.exports = old(Tree)
