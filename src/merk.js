let wrap = require('./wrap.js')
let { stringify, parse } = require('./json.js')
let {
  access,
  symbols,
  baseObject,
  isObject
} = require('./common.js')

class Mutations {
  constructor () {
    this.reset()
  }

  reset () {
    this.before = {}
    this.after = {}
  }

  keyIsNew (key) {
    return this.before[key] === symbols.delete
  }

  hasAncestor (path) {
    return this.ancestor(path) != null
  }

  ancestor (path) {
    for (let i = 1; i < path.length; i++) {
      let ancestorKey = pathToKey(path.slice(0, -i))
      if (ancestorKey in this.before) {
        return this.before[ancestorKey]
      }
    }
  }

  mutate ({ op, path, oldValue, newValue, existed }) {
    let key = pathToKey(path)

    // if first change for this key, record previous state
    if (!(key in this.before)) {
      let value = oldValue
      if (!existed) value = symbols.delete

      // don't record if parent was previously non-existent
      if (this.ancestor(path) !== symbols.delete) {
        this.before[key] = baseObject(value)
      }
    }

    // store updated value for key
    if (op === 'put') {
      this.after[key] = newValue
    } else if (op === 'del') {
      let parentWasDeleted = this.ancestor(path) === symbols.delete
      if (this.keyIsNew(key) || parentWasDeleted) {
        let value = this.after[key]

        if (isObject(value)) {
          // recursively update object properties
          for (let childKey in value) {
            if (typeof value[childKey] !== 'object') continue
            let childPath = path.concat(childKey)
            this.mutate({
              op: 'del',
              path: childPath,
              oldValue: value[childKey],
              newValue: symbols.delete,
              existed: true
            })
          }
        }

        delete this.before[key]
        delete this.after[key]
      } else {
        this.after[key] = symbols.delete
      }
    }
  }
}

function Merk (db) {
  if (!db || db.toString() !== 'LevelUP') {
    throw Error('Must provide a LevelUP instance')
  }

  let mutations = new Mutations()

  let root = {
    [symbols.mutations]: () => mutations,
    [symbols.root]: () => root,
    [symbols.db]: () => db
  }

  let onMutate = (mutation) => mutations.mutate(mutation)
  return wrap(root, onMutate)
}

function pathToKey (path) {
  if (path.length === 0) return symbols.root
  // TODO: support path components w/ "." character ('["foo.bar"]')
  return path.join('.')
}

function keyToPath (key) {
  if (key === symbols.root) return []
  // TODO: support path components w/ "." character ('["foo.bar"]')
  return key.split('.')
}

function assertRoot (root) {
  if (root[symbols.mutations] != null) return
  throw Error('Must specify a root merk object')
}

// revert to last commit
function rollback (root) {
  assertRoot(root)
  let mutations = root[symbols.mutations]()
  let unwrapped = root[symbols.root]()

  // reapply previous values
  for (let key in mutations.before) {
    let value = mutations.before[key]
    let path = keyToPath(key)

    // special case for setting properties on root object
    if (key === symbols.root) {
      Object.assign(unwrapped, value)
      continue
    }

    // assign old value to parent object
    let [ parent ] = access(unwrapped, path.slice(0, -1))
    let lastKey = path[path.length - 1]
    if (value === symbols.delete) {
      delete parent[lastKey]
    } else {
      parent[lastKey] = value
    }
  }

  mutations.reset()
}

// flush to db
async function commit (root) {
  assertRoot(root)
  let mutations = root[symbols.mutations]()
  let db = root[symbols.db]()

  let promises = []

  let mutationKeys = Object.keys(mutations.after)
  if (mutations.after[symbols.root]) {
    // root symbol is a special case since Symbols
    // aren't included in Object.keys
    mutationKeys.push(symbols.root)
  }

  for (let key of mutationKeys) {
    let prefixedKey = '.'
    if (key !== symbols.root) prefixedKey += key

    let value = mutations.after[key]
    if (value === symbols.delete) {
      promises.push(db.del(prefixedKey))
    } else {
      let json = stringify(value)
      promises.push(db.put(prefixedKey, json))
    }
  }

  // wait for all updates to complete
  await Promise.all(promises)

  mutations.reset()
}

function getter (symbol) {
  return function (root) {
    assertRoot(root)
    return root[symbol]()
  }
}

module.exports = Object.assign(Merk, {
  mutations: getter(symbols.mutations),
  rollback,
  commit,
  Mutations
})
