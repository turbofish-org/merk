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
    gets, puts, dels,
    get, put, del,
    store,
    toString: () => 'LevelUP'
  }

  // support callbacks for level-transactions
  mockDb.batch = async function (batch, cb) {
    try {
      for (let { type, key, value } of batch) {
        await mockDb[type](key, value)
      }
    } catch (err) {
      if (cb) return cb(err)
      throw err
    }
    if (cb) cb()
  }

  return mockDb
}

module.exports = {
  mockDb
}
