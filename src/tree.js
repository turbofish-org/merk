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

  async setRoot (id) {
    await this.db.put(':root', id)
  }

  async put (key, value) {
    // no root, create new node and set it as root
    if (this.rootNode == null) {
      this.rootNode = new this.Node({ key, value })
      await this.rootNode.save()
      await this.setRoot(this.rootNode.id)
      return
    }

    let successor = await this.rootNode.put(key, value)
    if (successor.id !== this.rootNode.id) {
      await this.setRoot(successor.id)
    }
  }

  get (key) {
    if (this.rootNode == null) return null

    let node = this.rootNode.search(key)
    if (node.key !== key) {
      throw Error(`Key "${key}" not found`)
    }
    return node.value
  }
}
