let { createHash } = require('crypto')

const symbols = {
  mutations: Symbol('mutations'),
  db: Symbol('db'),
  delete: Symbol('delete'),
  root: Symbol('root')
}

function sha256 (data) {
  return createHash('sha256').update(data).digest()
}

function isObject (value) {
  return typeof value === 'object' && value != null
}

// clones an object, without any properties of type 'object'
function baseObject (obj) {
  let base = {}
  for (let key in obj) {
    let value = obj[key]
    if (isObject(value)) continue
    base[key] = value
  }
  return base
}

// gets an object property based on an array key path
function access (obj, path) {
  if (path.length === 0) {
    return [ obj, true ]
  }

  let [ key, ...subpath ] = path
  if (!isObject(obj)) {
    throw Error(`Could not access property "${key}" of ${obj}`)
  }
  if (subpath.length === 0) {
    return [ obj[key], key in obj ]
  }
  return access(obj[key], subpath)
}

// shallow clone
function clone (value) {
  if (!isObject(value)) return value
  return Object.assign({}, value)
}

module.exports = {
  sha256,
  isObject,
  baseObject,
  access,
  clone,
  symbols
}
