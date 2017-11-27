// wraps an object with a proxy, so we can keep track of mutations
function wrap (obj, mutations, path = []) {
  function recordMutation (op, key, args) {
    let mutation = Object.assign({
      op,
      path: key ? path.concat(key) : path,
      oldValue: key ? clone(obj[key]) : clone(obj)
    }, args)
    mutations.push(mutation)
    // TODO: replace mutations for overriden paths
  }

  // TODO: wrap array methods to record array-specific mutations,
  // otherwise ops like splices and shifts will create N mutations

  return new Proxy(obj, {
    // recursively wrap child objects when accessed
    get (obj, key) {
      let value = obj[key]

      // if value is object, recursively wrap
      if (typeof value === 'object') {
        let childPath = path.concat(key)
        return wrap(value, mutations, childPath)
      }

      return value
    },

    // record mutations
    set (obj, key, value) {
      let wasDefined = key in obj
      if (typeof value === 'object') {
        recordMutation('put', key, {
          newValue: value,
          wasDefined
        })
      } else {
        let newValue = clone(obj)
        newValue[key] = value
        recordMutation('put', null, { newValue })
      }
      obj[key] = value
      return true
    },

    // record deletions as mutations too
    deleteProperty (obj, key) {
      recordMutation('del', key)
      delete obj[key]
      return true
    },

    // ovverride ownKeys to exclude symbol properties
    ownKeys () {
      return Object.getOwnPropertyNames(obj)
    }
  })
}

function clone (value) {
  if (typeof value !== 'object') return value
  // TODO: better deep clone
  return JSON.parse(JSON.stringify(value))
}

module.exports = wrap
