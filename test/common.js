function mockDb (db) {
  let store = db ? db.store : {}

  let gets = []
  let puts = []
  let dels = []

  // support callbacks for level-transactions
  async function get (key, opts, cb) {
    gets.push({ key })
    let value = store[key]
    if (!value) {
      let err = new Error(`Key ${key} not found`)
      err.notFound = true
      if (cb) return cb(err)
      throw err
    }
    if (cb) return cb(null, value)
    return value
  }
  async function put (key, value) {
    puts.push({ key, value })
    store[key] = value
  }
  async function del (key) {
    dels.push({ key })
    delete store[key]
  }

  let mockDb = {
    gets,
    puts,
    dels,
    get,
    put,
    del,
    batch: () => ({
      get,
      put,
      del,
      write: () => Promise.resolve()
    }),
    store,
    toString: () => 'LevelUP'
  }

  return mockDb
}

// hack to fix deepEqual check for Proxied objects
function deepEqual (t, actual, expected) {
  return t.deepEqual(
    JSON.parse(JSON.stringify(actual)),
    JSON.parse(JSON.stringify(expected))
  )
}

module.exports = {
  mockDb,
  deepEqual
}
