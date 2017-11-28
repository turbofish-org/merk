let Node = require('./node.js')

module.exports = class Tree {
  constructor (db) {
    if (db == null) {
      throw Error('Must specify a LevelUp interface')
    }
    this.db = db
    this.Node = Node(db)
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
    // no root, create new node and set it as root
    if (this.rootNode == null) {
      let node = new this.Node({ key, value })
      await node.save()
      await this.setRoot(node)
      return
    }

    let successor = await this.rootNode.put(key, value)
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
