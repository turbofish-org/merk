let wrap = require('./wrap.js')
let { stringify, parse } = require('./json.js')
let Trie = require('merkle-patricia-tree')
let { promisify } = require('util')

// TODO: use an in-memory merkle AVL+ tree instead

const symbols = {
  mutations: Symbol('mutations'),
  tree: Symbol('tree')
}

function Merk (db) {
  let mutations = []
  let root = {
    [symbols.mutations]: mutations,
    [symbols.tree]: new Trie(db)
  }
  return wrap(root, mutations)
}

function getMutations (root) {
  let mutations = root[symbols.mutations]
  if (mutations == null) {
    throw Error('Must specify a root merk object')
  }
  return mutations
}

// revert to last commit
function reset (root) {
  let mutations = getMutations(root)

  // work backwards through mutations and revert values
  for (let i = mutations.length - 1; i >= 0; i--) {
    let { path, oldValue, wasDefined } = mutations[i]

    // follow path (except for last key)
    let cursor = root
    for (let key of path.slice(0, -1)) {
      cursor = cursor[key]
    }

    let lastKey = path[path.length - 1]
    if (wasDefined !== false) {
      // set to old value
      cursor[lastKey] = oldValue
    } else {
      // if it wasn't previously defined, delete it
      delete cursor[lastKey]
    }
  }

  // empty mutations array
  mutations.splice(0, mutations.length)
}

// update merkle tree, and flush to db if one was given
async function commit (root) {
  let mutations = getMutations(root)
  let tree = root[symbols.tree]

  let ops = mutations.map(({ op, path, newValue }) => ({
    type: op,
    key: path.join('\u0000'),
    value: stringify(newValue)
  }))

  // update tree and persist in db
  let batch = promisify(tree.batch.bind(tree))
  await batch(ops)

  // empty mutations array
  mutations.splice(0, mutations.length)
}

module.exports = Object.assign(Merk, {
  mutations: getMutations,
  reset,
  commit
})
