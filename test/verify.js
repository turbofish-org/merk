let test = require('ava')
let { mockDb, deepEqual } = require('./common.js')
let merk = require('../src/merk.js')

test('verify single-node root proof', async (t) => {
  let db = mockDb()
  let state = await merk(db)
  state.foo = 'bar'
  await merk.commit(state)

  let rootHash = merk.hash(state)
  let proof = await merk.proof(state)
  let value = merk.verify(rootHash, proof)

  deepEqual(t, value, { foo: 'bar' })
})

test('verify single-node non-object child proof', async (t) => {
  let db = mockDb()
  let state = await merk(db)
  state.foo = 'bar'
  await merk.commit(state)

  let rootHash = merk.hash(state)
  let proof = await merk.proof(state, 'foo')
  let value = merk.verify(rootHash, proof, 'foo')

  t.is(value, 'bar')
})

test('verify multi-node root proof', async (t) => {
  let db = mockDb()
  let state = await merk(db)
  state.foo = 'bar'
  state.baz = { x: 123 }
  await merk.commit(state)

  let rootHash = merk.hash(state)
  let proof = await merk.proof(state)
  let value = merk.verify(rootHash, proof)

  deepEqual(t, value, { foo: 'bar', baz: { x: 123 } })
})

test('verify multi-node child proof', async (t) => {
  let db = mockDb()
  let state = await merk(db)
  state.foo = 'bar'
  state.baz = { x: 123 }
  await merk.commit(state)

  let rootHash = merk.hash(state)
  let proof = await merk.proof(state, 'baz')
  let value = merk.verify(rootHash, proof, 'baz')

  deepEqual(t, value, { x: 123 })
})

test('verify multi-node non-object child proof', async (t) => {
  let db = mockDb()
  let state = await merk(db)
  state.foo = 'bar'
  state.baz = { x: 123 }
  await merk.commit(state)

  let rootHash = merk.hash(state)
  let proof = await merk.proof(state, 'baz.x')
  let value = merk.verify(rootHash, proof, 'baz.x')

  t.is(value, 123)
})

test('verify with incorrect root hash', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  state.foo = 'bar'
  await merk.commit(state)
  let rootHash = merk.hash(state)

  state.baz = { x: 123 }
  await merk.commit(state)

  let proof = await merk.proof(state, 'baz')
  try {
    merk.verify(rootHash, proof, 'baz')
    t.fail()
  } catch (err) {
    t.is(err.message, 'Proof does not match expected root hash')
  }
})

test('verify with array range', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  // state.abc = {n: 123}
  // state.array = [{n: 0}, {n: 1}, {n: 2}, {n: 3}]
  // state.xyz = {n: 456}
  // await merk.commit(state)
  // let rootHash = merk.hash(state)
  // let proof = await merk.proof(state, 'array')

  let rootHash = 'ac2323c6ebaafb3174c6dfac855ed0652f4651af'
  let proof = {
    left: {
      left: { left: null, right: null, key: '.abc', value: '{"n":123}' },
      right: { left: null, right: null, key: '.array.0', value: '{"n":0}' },
      key: '.array',
      value: '[]'
    },
    right: {
      left: { left: null, right: null, key: '.array.2', value: '{"n":2}' },
      right: { left: null, right: null, key: '.xyz', value: '{"n":456}' },
      key: '.array.3',
      value: '{"n":3}'
    },
    key: '.array.1',
    value: '{"n":1}'
  }

  let value = merk.verify(rootHash, proof, 'array')
  deepEqual(t, value, [{n: 0}, {n: 1}, {n: 2}, {n: 3}])
})

test('verify with array child', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  state.abc = {n: 123}
  state.array = [{n: 0}, {n: 1}, {n: 2}, {n: 3}]
  state.xyz = {n: 456}
  await merk.commit(state)
  let rootHash = merk.hash(state)
  let proof = await merk.proof(state, 'array.1')

  let value = merk.verify(rootHash, proof, 'array.1')
  deepEqual(t, value, {n: 1})
})

test('verify with unproven range', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  let rootHash = 'ac2323c6ebaafb3174c6dfac855ed0652f4651af'
  let proof = {
    left: {
      left: 'xxMB1E9t2x1ZoUbKiEi+ffyH7sI=',
      right: { left: null, right: null, key: '.array.0', value: '{"n":0}' },
      key: '.array',
      value: '[]'
    },
    right: {
      left: { left: null, right: null, key: '.array.2', value: '{"n":2}' },
      right: { left: null, right: null, key: '.xyz', value: '{"n":456}' },
      key: '.array.3',
      value: '{"n":3}'
    },
    key: '.array.1',
    value: '{"n":1}'
  }

  try {
    merk.verify(rootHash, proof, 'array')
    t.fail()
  } catch (err) {
    t.is(err.message, 'First key greater than beginning of range')
  }
})

test('verify with unproven range', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  let rootHash = 'ac2323c6ebaafb3174c6dfac855ed0652f4651af'
  let proof = {
    left: {
      left: { left: null, right: null, key: '.abc', value: '{"n":123}' },
      right: { left: null, right: null, key: '.array.0', value: '{"n":0}' },
      key: '.array',
      value: '[]'
    },
    right: {
      left: { left: null, right: null, key: '.array.2', value: '{"n":2}' },
      right: 'dI02/HM0MSGWx6UTqX0zG3TCE1Q=',
      key: '.array.3',
      value: '{"n":3}'
    },
    key: '.array.1',
    value: '{"n":1}'
  }

  try {
    merk.verify(rootHash, proof, 'array')
    t.fail()
  } catch (err) {
    t.is(err.message, 'Last key less than end of range')
  }
})

test('verify with range that was previously broken', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  let rootHash = '8cbbf63175e9390054c2155419ccea4ce38508a0'
  let proof = require('./fixtures/blockchat.json')

  merk.verify(rootHash, proof, 'messages')
  t.pass()
})

test('verify with single-node tree with no root object', async (t) => {
  let db = mockDb()
  let state = await merk(db)

  state.foo = { beep: 'boop' }

  await merk.commit(state)

  let proof = await merk.proof(state)

  let rootHash = '4a492d386d845c19a691698b91d9afaddba0a0fb'
  let value = merk.verify(rootHash, proof)

  t.pass()
})
