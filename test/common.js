function mockDb (db) {
  let store = db ? db.store : {}

  let gets = []
  let puts = []
  let dels = []

  async function get (key) {
    gets.push({ key })
    let value = store[key]
    if (!value) {
      let err = new Error('Not found')
      err.notFound = true
      throw err
    }
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

  mockDb.batch = async function (batch) {
    for (let { type, key, value } of batch) {
      await mockDb[type](key, value)
    }
  }

  return mockDb
}

module.exports = {
  mockDb
}
