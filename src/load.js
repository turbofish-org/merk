let { access, keyToPath } = require('./common.js')
let { parse } = require('./json.js')

// load existing data if it exists
async function load (tree) {
  // TODO: decouple from tree
  // (could be levelup createReadStream)

  let rootNode = await tree.rootNode()
  if (!rootNode) return {} // no existing data

  // get root object
  let cursor = await rootNode.min()
  let root = parse(cursor.value)

  // iterate through all keys in order,
  // create objects based on key/value
  cursor = await cursor.next()
  while (cursor) {
    let path = keyToPath(cursor.key.slice(1))
    let [ parent ] = access(root, path.slice(0, -1))
    let key = path[path.length - 1]
    parent[key] = parse(cursor.value)
    cursor = await cursor.next()
  }

  return root
}

module.exports = load
