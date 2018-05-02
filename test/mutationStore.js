let test = require('ava')
let { mockDb, deepEqual } = require('./common.js')
let { symbols } = require('../src/common.js')
let MutationStore = require('../src/mutationStore.js')

test('create mutationStore', (t) => {
  let ms = MutationStore()
  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})

test('put non-object on root', (t) => {
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { foo: 'bar' },
    existed: true
  })

  deepEqual(t, ms.before, {
    [symbols.root]: {}
  })
  deepEqual(t, ms.after, {
    [symbols.root]: { foo: 'bar' }
  })
})

test('put object on root', (t) => {
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })

  deepEqual(t, ms.before, {
    foo: symbols.delete
  })
  deepEqual(t, ms.after, {
    foo: { x: 5 }
  })
})

test('ms are deduped', (t) => {
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: { x: 6 },
    existed: true
  })

  deepEqual(t, ms.before, {
    foo: symbols.delete
  })
  deepEqual(t, ms.after, {
    foo: { x: 6 }
  })
})

test('delete non-preexisting key', (t) => {
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'del',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: undefined,
    existed: true
  })

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})

test('delete key from non-preexisting object', (t) => {
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: {},
    existed: true
  })

  deepEqual(t, ms.before, {
    foo: symbols.delete
  })
  deepEqual(t, ms.after, {
    foo: {}
  })
})

test('delete multi-level non-preexisting key', (t) => {
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: {},
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo', 'x' ],
    oldValue: undefined,
    newValue: { y: 5 },
    existed: false
  })
  ms.mutate({
    op: 'del',
    path: [ 'foo', 'x' ],
    oldValue: {},
    newValue: undefined,
    existed: true
  })
  ms.mutate({
    op: 'del',
    path: [ 'foo' ],
    oldValue: {},
    newValue: undefined,
    existed: true
  })

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})

test('commit', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { bar: 'baz' },
    existed: true
  })

  await ms.commit(db)

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})

  deepEqual(t, db.puts, [
    { key: '.foo', value: '{"x":5}' },
    { key: '.', value: '{"bar":"baz"}' }
  ])
  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
})

test('mutate after commit', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { bar: 'baz' },
    existed: true
  })

  await ms.commit(db)

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: { x: 6 },
    existed: true
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: { bar: 'baz' },
    newValue: { bar: 'BAZ' },
    existed: true
  })

  deepEqual(t, ms.before, {
    foo: { x: 5 },
    [symbols.root]: { bar: 'baz' }
  })
  deepEqual(t, ms.after, {
    foo: { x: 6 },
    [symbols.root]: { bar: 'BAZ' }
  })

  await ms.commit(db)

  deepEqual(t, db.puts, [
    { key: '.foo', value: '{"x":5}' },
    { key: '.', value: '{"bar":"baz"}' },
    { key: '.foo', value: '{"x":6}' },
    { key: '.', value: '{"bar":"BAZ"}' }
  ])
})

test('delete after commit', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { bar: 'baz' },
    existed: true
  })

  await ms.commit(db)

  ms.mutate({
    op: 'del',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: undefined,
    existed: true
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: { bar: 'baz' },
    newValue: {},
    existed: true
  })

  deepEqual(t, ms.before, {
    foo: { x: 5 },
    [symbols.root]: { bar: 'baz' }
  })
  deepEqual(t, ms.after, {
    foo: symbols.delete,
    [symbols.root]: {}
  })
})

test('commit deletion', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { bar: 'baz' },
    existed: true
  })

  await ms.commit(db)

  ms.mutate({
    op: 'del',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: undefined,
    existed: true
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: { bar: 'baz' },
    newValue: {},
    existed: true
  })

  await ms.commit(db)

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})

  deepEqual(t, db.puts, [
    { key: '.foo', value: '{"x":5}' },
    { key: '.', value: '{"bar":"baz"}' },
    { key: '.', value: '{}' }
  ])
  deepEqual(t, db.dels, [
    { key: '.foo' }
  ])
  t.is(db.gets.length, 0)
})

test('commit without root mutation', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })

  await ms.commit(db)

  ms.mutate({
    op: 'del',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: undefined,
    existed: true
  })

  await ms.commit(db)

  deepEqual(t, db.puts, [ { key: '.foo', value: '{"x":5}' } ])
  deepEqual(t, db.dels, [ { key: '.foo' } ])

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})

test('rollback from null state', async (t) => {
  let ms = MutationStore()

  let obj = {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  }

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo', 'y' ],
    oldValue: undefined,
    newValue: { z: 123 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { bar: 'baz' },
    existed: true
  })

  ms.rollback(obj)

  deepEqual(t, obj, {})
  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})

test('rollback from committed state', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo', 'y' ],
    oldValue: undefined,
    newValue: { z: 123 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: {},
    newValue: { bar: 'baz' },
    existed: true
  })

  await ms.commit(db)

  let obj = {
    foo: { x: 6, y: { z: 123, A: {} } },
    bar: 'BAZ'
  }

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: { x: 6 },
    existed: true
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo', 'y', 'A' ],
    oldValue: undefined,
    newValue: {},
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [],
    oldValue: { bar: 'baz' },
    newValue: { bar: 'BAZ' },
    existed: true
  })

  ms.rollback(obj)

  deepEqual(t, obj, {
    foo: { x: 5, y: { z: 123 } },
    bar: 'baz'
  })

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})

test('rollback without root mutation', async (t) => {
  let db = mockDb()
  let ms = MutationStore()

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: undefined,
    newValue: { x: 5 },
    existed: false
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo', 'y' ],
    oldValue: undefined,
    newValue: { z: 123 },
    existed: false
  })

  await ms.commit(db)

  let obj = {
    foo: { x: 6, y: { z: 123, A: {} } }
  }

  ms.mutate({
    op: 'put',
    path: [ 'foo' ],
    oldValue: { x: 5 },
    newValue: { x: 6 },
    existed: true
  })
  ms.mutate({
    op: 'put',
    path: [ 'foo', 'y', 'A' ],
    oldValue: undefined,
    newValue: {},
    existed: false
  })

  ms.rollback(obj)

  deepEqual(t, obj, {
    foo: { x: 5, y: { z: 123 } }
  })

  deepEqual(t, ms.before, {})
  deepEqual(t, ms.after, {})
})
