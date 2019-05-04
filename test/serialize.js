let test = require('ava')
let { mockDb, deepEqual } = require('./common.js')
let merk = require('../src/merk.js')

test('serialize/deserialize array', async (t) => {
  let db = mockDb()

  let obj = await merk(db)
  obj.array = [ 1, 2, 3 ]
  await merk.commit(obj)

  let obj2 = await merk(db)
  deepEqual(t, obj2, { array: [ 1, 2, 3 ] })
})
