let test = require('ava')
let { mockDb, deepEqual } = require('./common.js')
let merk = require('../src/merk.js')

test('create merk without db', async (t) => {
  try {
    await merk()
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must provide a LevelUP instance')
  }

  try {
    await merk({})
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must provide a LevelUP instance')
  }
})

test('create merk', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  deepEqual(t, obj, {})

  let mutations = merk.mutations(obj)
  deepEqual(t, mutations.before, {})
  deepEqual(t, mutations.after, {})
})

test('create merk with existing data', async (t) => {
  let db = mockDb({
    store: {
      ':root': '.foo',
      'n.foo': 'G+PH6PHsNTbVHY8Kt0AJypGvIU+9uZNSZeEldRF/R+AxQJmP7oqoQAEBB3sieCI6NX0BLgYuZm9vLnkA',
      'n.foo.y': 'yJfgdnJGSp+rACbCM+3xlQeaxlAWd6DqBI4netwFUt3YkCpNxG7GCAAACXsieiI6MTIzfQAABC5mb28=',
      'n.': 'cWSoYAhQYgpWR5tMg1mrQk7NKurl2dH4Ve4Abbq4P+mpcc7kve/sxgAADXsiYmFyIjoiYmF6In0AAAQuZm9v'
    }
  })

  let obj = await merk(db)

  deepEqual(t, obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })

  let mutations = merk.mutations(obj)
  deepEqual(t, mutations.before, {})
  deepEqual(t, mutations.after, {})
})

test('create merk with existing data, with no non-objects on root', async (t) => {
  let db = mockDb({
    store: {
      ':root': '.foo',
      'n.foo': 'nVLY483AXQKEMrv1w66IcV3v/IG9uZNSZeEldRF/R+AxQJmP7oqoQAABB3sieCI6NX0ABi5mb28ueQA=',
      'n.foo.y': 'yJfgdnJGSp+rACbCM+3xlQeaxlAWd6DqBI4netwFUt3YkCpNxG7GCAAACXsieiI6MTIzfQAABC5mb28='
    }
  })

  let obj = await merk(db)

  deepEqual(t, obj, {
    foo: { x: 5, y: { z: 123 } }
  })

  let mutations = merk.mutations(obj)
  deepEqual(t, mutations.before, {})
  deepEqual(t, mutations.after, {})
})

test('rollback', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  deepEqual(t, obj, {})

  obj.foo = { x: 5, y: { z: 123 } }
  obj.bar = 'baz'

  deepEqual(t, obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })

  merk.rollback(obj)

  deepEqual(t, obj, {})
})

test('commit', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.foo = { x: 5, y: { z: 123 } }
  obj.bar = 'baz'

  await merk.commit(obj)

  deepEqual(t, obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })
  t.is(merk.hash(obj).toString('hex'), '1be3c7e8f1ec3536d51d8f0ab74009ca91af214f')

  let mutations = merk.mutations(obj)
  deepEqual(t, mutations.before, {})
  deepEqual(t, mutations.after, {})
})

test('call merk methods on non-merk object', async (t) => {
  try {
    await merk.commit({})
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must specify a root merk object')
  }

  try {
    merk.mutations({})
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must specify a root merk object')
  }

  try {
    merk.rollback({})
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must specify a root merk object')
  }
})

test('rollback on array length increase', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.array = [ 1, 2, 3 ]

  await merk.commit(obj)

  obj.array.push(4)

  merk.rollback(obj)

  deepEqual(t, obj, { array: [ 1, 2, 3 ] })
})

test('rollback on array length increase with objects', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.array = [ {}, {}, {} ]

  await merk.commit(obj)

  obj.array.push({})

  merk.rollback(obj)

  deepEqual(t, obj, { array: [ {}, {}, {} ] })
})

test('rollback on array length decrease', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.array = [ 1, 2, 3 ]

  await merk.commit(obj)

  obj.array.pop()

  merk.rollback(obj)

  deepEqual(t, obj, { array: [ 1, 2, 3 ] })
})

test('rollback on array length decrease with objects', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.array = [ {}, {}, {} ]

  await merk.commit(obj)

  obj.array.pop()

  merk.rollback(obj)

  deepEqual(t, obj, { array: [ {}, {}, {} ] })
})

test('rollback on array length increase with mixed types', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.array = [ {}, {}, {}, 4, 5, 6 ]

  await merk.commit(obj)

  obj.array.push({})

  merk.rollback(obj)

  deepEqual(t, obj, { array: [ {}, {}, {}, 4, 5, 6 ] })
})
