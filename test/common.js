function clone (obj) {
  return JSON.parse(JSON.stringify(obj))
}

function mockDb (db) {
  let store = db ? db.store : {}
  let log = {
    gets: [],
    puts: [],
    dels: []
  }

  let createOpFuncs = (store, log) => ({
    get (key, opts) {
      log.gets.push({ key })
      let value = store[key]
      if (!value) {
        let err = new Error(`Key ${key} not found`)
        err.notFound = true
        throw err
      }
      return value
    },
    put (key, value) {
      log.puts.push({ key, value })
      store[key] = value
    },
    del (key) {
      log.dels.push({ key })
      delete store[key]
    }
  })

  let opFuncs = createOpFuncs(store, log)

  function batch () {
    let batchStore = clone(store)
    let batchLog = {
      gets: [],
      puts: [],
      dels: []
    }

    let batchOpFuncs = createOpFuncs(batchStore, batchLog)

    let ops = []
    function opFunc (op) {
      return (...args) => {
        ops.push({ args, op: opFuncs[op] })
        batchOpFuncs[op](...args)
      }
    }

    async function write () {
      for (let { op, args } of ops) {
        op(...args)
      }
    }

    return {
      get: async (...args) => batchOpFuncs.get(...args),
      put: opFunc('put'),
      del: opFunc('del'),
      write
    }
  }

  let mockDb = {
    get: async (...args) => opFuncs.get(...args),
    put: async (...args) => opFuncs.put(...args),
    del: async (...args) => opFuncs.del(...args),
    store,
    ...log,
    batch,
    toString: () => 'LevelUP'
  }

  return mockDb
}

// hack to fix deepEqual check for Proxied objects
function deepEqual (t, actual, expected) {
  return t.deepEqual(clone(actual), clone(expected))
}

module.exports = {
  mockDb,
  deepEqual
}
