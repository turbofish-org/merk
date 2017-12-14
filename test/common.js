function mockDb (db) {
  let store = db ? db.store : {}
  let gets = []
  let puts = []
  let dels = []
  return {
    async get (key) {
      gets.push({ key })
      let value = store[key]
      if (!value) {
        let err = new Error('Not found')
        err.notFound = true
        throw err
      }
      return value
    },
    async put (key, value) {
      puts.push({ key, value })
      store[key] = value
    },
    async del (key) {
      dels.push({ key })
      delete store[key]
    },
    gets,
    puts,
    dels,
    store
  }
}

module.exports = {
  mockDb
}
