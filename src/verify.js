let struct = require('varstruct')
let VarInt = require('varint')
let {
  ripemd160,
  sha256,
  keyToPath,
  access,
  symbols
} = require('./common.js')
let { parse } = require('./json.js')

const nullHash = Buffer.alloc(20)
const VarString = struct.VarString(VarInt)

function childHash (child) {
  if (child == null) {
    return nullHash
  }
  if (typeof child === 'string') {
    return Buffer.from(child, 'base64')
  }
  if (typeof child === 'object') {
    return getHash(child)
  }
}

function getHash (node) {
  let kvHash = node.kvHash
    ? Buffer.from(node.kvHash, 'base64')
    : getKvHash(node)

  let input = Buffer.concat([
    childHash(node.left),
    childHash(node.right),
    kvHash
  ])
  return ripemd160(sha256(input))
}

function getKvHash ({ key, value }) {
  let input = Buffer.concat([
    VarString.encode(key),
    VarString.encode(value)
  ])
  return ripemd160(sha256(input))
}

function flatten (node, nodes = [], path = []) {
  if (node.left && typeof node.left === 'object') {
    flatten(node.left, nodes, path.concat(false))
  }
  node.isEdge = (path.length === 0 ||
    path.reduce((a, b) => a === b)) &&
    node.left == null && node.right == null
  nodes.push(node)
  if (node.right && typeof node.right === 'object') {
    flatten(node.right, nodes, path.concat(true))
  }
  return nodes
}

function verify (expectedRootHash, proof, query = '') {
  let rootHash = getHash(proof).toString('hex')
  if (rootHash !== expectedRootHash) {
    throw Error('Proof does not match expected root hash')
  }

  let from = '.' + query
  let to = '.' + query + '/'
  if (query === '') to = '/'
  let nodes = flatten(proof)

  // special case for single-node tree (only root object)
  if (nodes.length === 1) {
    if (nodes[0].key !== '.') {
      throw Error('Expected node to be root object')
    }
    return JSON.parse(nodes[0].value)
  }

  // get contiguous nodes which have key/value
  let valueNodes = []
  for (let node of nodes) {
    if (!node.key) {
      if (valueNodes.length > 0) break
      continue
    }
    valueNodes.push(node)
  }

  let checkRange = () => {
    let firstKeyPastFrom = valueNodes[0].key >= from
    let firstKeyIsEdge = valueNodes[0].isEdge
    if (firstKeyPastFrom && !firstKeyIsEdge) {
      throw Error('First key greater than beginning of range')
    }

    let lastKeyBeforeTo = valueNodes[valueNodes.length - 1].key <= to
    let lastKeyIsEdge = valueNodes[valueNodes.length - 1].isEdge
    if (lastKeyBeforeTo && !lastKeyIsEdge) {
      throw Error('Last key less than end of range')
    }
  }

  let resultNodes = valueNodes.filter((node) => {
    return node.key >= from && node.key <= to
  })

  // try getting parent object
  if (resultNodes.length === 0) {
    let path = query.split('.')
    let parentKey = '.' + path.slice(0, -1).join('.')
    let valueKey = path[path.length - 1]
    from = parentKey
    to = parentKey + '.'
    checkRange()
    for (let node of valueNodes) {
      if (node.key === parentKey) {
        var parentNode = node
        break
      }
      if (node.key > parentKey) {
        throw Error('Parent node not found')
      }
    }
    let parentValue = parse(parentNode.value)
    return parentValue[valueKey]
  }

  checkRange()

  let result
  for (let node of resultNodes) {
    // remove query prefix
    let key = node.key.slice(from.length)
    if (key === '') key = symbols.root

    // add parsed value to result
    let path = keyToPath(key)
    if (path[0] === '') path.shift()
    let value = parse(node.value)

    if (path.length === 0) {
      result = value
    } else {
      let [ parent ] = access(result, path.slice(0, -1))
      let name = path[path.length - 1]
      parent[name] = value
    }
  }
  return result
}

module.exports = verify
