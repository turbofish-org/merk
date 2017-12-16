let test = require('ava')
let { mockDb } = require('./common.js')
let { symbols } = require('../src/common.js')
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

  t.deepEqual(obj, {})

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('create merk with existing data', async (t) => {
  let db = mockDb({
    store: {
      ':root': 1,
      ':idCounter': '4',
      n1: 'FM21QJMlkitThEAO6vg196MgF/HRaukjRyhwN5aDyJtqqJL6mfKrMKs+oxi76hTTZTAmxGSXJ1+GsXJHzlO8cwEBBC5mb28HeyJ4Ijo1fQMCAA==',
      n2: 'IN4PGZmDAfVOfLK9pEAQm8CHdfPhryGcSD3RfxPqMk/B+aH4xTjNCvJxITWoTsevFD1GIjHZOJ+IzpMhn4ipvwAABi5mb28ueQl7InoiOjEyM30AAAE=',
      n3: '0h1/8jCmYzJL9GFBO8p1DxSvhEAT10GJCfl0zpFBo0UIfXNjzc8RZK/segX2QVOH7PB2YvlaUr5pyZcTaUB4PQAAAS4NeyJiYXIiOiJiYXoifQAAAQ=='
    }
  })

  let obj = await merk(db)

  t.deepEqual(obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })

  let mutations = merk.mutations(obj)
  t.deepEqual(mutations.before, {})
  t.deepEqual(mutations.after, {})
})

test('rollback', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  t.deepEqual(obj, {})

  obj.foo = { x: 5, y: { z: 123 } }
  obj.bar = 'baz'

  t.deepEqual(obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })

  merk.rollback(obj)

  t.deepEqual(obj, {})
})

test('commit', async (t) => {
  let db = mockDb()
  let obj = await merk(db)

  obj.foo = { x: 5, y: { z: 123 } }
  obj.bar = 'baz'

  await merk.commit(obj)

  t.deepEqual(obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })
  t.is(merk.hash(obj).toString('hex'), '14cdb5409325922b5384400eeaf835f7a32017f1d16ae923472870379683c89b')

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
