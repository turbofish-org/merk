let test = require('ava')
let { mockDb } = require('./common.js')
let { symbols } = require('../src/common.js')
let merk = require('../src/merk.js')

test('create merk without db', (t) => {
  try {
    merk()
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must provide a LevelUP instance')
  }

  try {
    merk({})
    t.fail()
  } catch (err) {
    t.is(err.message, 'Must provide a LevelUP instance')
  }
})

test('create merk', (t) => {
  let db = mockDb()
  let obj = merk(db)

  t.deepEqual(obj, {})

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('set non-object on root', (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = 'bar'

  t.deepEqual(obj, { foo: 'bar' })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {
    [symbols.root]: {}
  })
  t.deepEqual(mutations.after, {
    [symbols.root]: { foo: 'bar' }
  })
})

test('set object on root', (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }

  t.deepEqual(obj, { foo: { x: 5 } })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {
    foo: symbols.delete
  })
  t.deepEqual(mutations.after, {
    foo: { x: 5 }
  })
})

test('mutations are deduped', (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.foo = { x: 6 }

  t.deepEqual(obj, { foo: { x: 6 } })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {
    foo: symbols.delete
  })
  t.deepEqual(mutations.after, {
    foo: { x: 6 }
  })
})

test('delete non-preexisting key', (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  delete obj.foo

  t.deepEqual(obj, {})

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('delete child of non-preexisting key', (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  delete obj.foo.x

  t.deepEqual(obj, { foo: {} })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {
    foo: symbols.delete
  })
  t.deepEqual(mutations.after, {
    foo: {}
  })
})

test('delete multi-level non-preexisting key', (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: { y: 5 }  }
  delete obj.foo

  t.deepEqual(obj, {})

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('commit', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.bar = 'baz'

  await merk.commit(obj)

  t.deepEqual(obj, {
    foo: { x: 5 },
    bar: 'baz'
  })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})

  t.deepEqual(db.puts, [
    { key: '.foo', value: '{"x":5}' },
    { key: '.', value: '{"bar":"baz"}' }
  ])
  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
})

test('mutate after commit', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.bar = 'baz'

  await merk.commit(obj)

  obj.foo.x++
  obj.bar = 'BAZ'

  t.deepEqual(obj, { foo: { x: 6 }, bar: 'BAZ' })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {
    foo: { x: 5 },
    [symbols.root]: { bar: 'baz' }
  })
  t.deepEqual(mutations.after, {
    foo: { x: 6 },
    [symbols.root]: { bar: 'BAZ' }
  })
})

test('delete after commit', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.bar = 'baz'

  await merk.commit(obj)

  delete obj.foo
  delete obj.bar

  t.deepEqual(obj, {})

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {
    foo: { x: 5 },
    [symbols.root]: { bar: 'baz' }
  })
  t.deepEqual(mutations.after, {
    foo: symbols.delete,
    [symbols.root]: {}
  })
})

test('commit deletion', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.bar = 'baz'

  await merk.commit(obj)

  delete obj.foo
  delete obj.bar

  await merk.commit(obj)

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})

  t.deepEqual(db.puts, [
    { key: '.foo', value: '{"x":5}' },
    { key: '.', value: '{"bar":"baz"}' },
    { key: '.', value: '{}' }
  ])
  t.deepEqual(db.dels, [
    { key: '.foo' }
  ])
  t.is(db.gets.length, 0)
})

test('commit without root mutation', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }

  await merk.commit(obj)

  delete obj.foo

  await merk.commit(obj)

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('rollback from null state', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.foo.y = { z: 123 }
  obj.bar = 'baz'

  merk.rollback(obj)

  t.deepEqual(obj, {})

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('rollback from committed state', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.foo.y = { z: 123 }
  obj.bar = 'baz'

  await merk.commit(obj)

  obj.foo.x++
  obj.foo.y.A = {}
  obj.bar = 'BAZ'

  merk.rollback(obj)

  t.deepEqual(obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('rollback without root mutation', async (t) => {
  let db = mockDb()
  let obj = merk(db)

  obj.foo = { x: 5 }
  obj.foo.y = { z: 123 }

  await merk.commit(obj)

  obj.foo.x++
  obj.foo.y.A = {}

  merk.rollback(obj)

  t.deepEqual(obj, {
    foo: { x: 5, y: { z: 123 } }
  })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
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

test('keyToPath', async (t) => {
  t.deepEqual(merk.keyToPath(symbols.root), [])
  t.deepEqual(merk.keyToPath('foo.bar.baz'), [ 'foo', 'bar', 'baz' ])
})
