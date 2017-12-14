let test = require('ava')
let { mockDb } = require('./common.js')
let _Node = require('../src/node.js')

test('create node without key', async (t) => {
  let db = mockDb()
  let Node = _Node(db)
  try {
    let node = new Node({})
    t.notOk(node)
  } catch (err) {
    t.is(err.message, 'Key is required')
  }
})

test('create node without value', async (t) => {
  let db = mockDb()
  let Node = _Node(db)
  try {
    let node = new Node({ key: 'foo' })
    t.notOk(node)
  } catch (err) {
    t.is(err.message, 'Value is required')
  }
})

test('create node', async (t) => {
  let db = mockDb()
  let tx = mockDb()
  let Node = _Node(db)
  let node = new Node({ key: 'foo', value: 'bar' }, tx)
  t.is(node.id, 1)
  t.is(node.key, 'foo')
  t.is(node.value, 'bar')
  t.is(node.hash.toString('hex'), 'f2aafaf4e1a1064eed0fc5e1d7d6844fffaccdde46a3ca5f3885e8a685b9b09e')
  t.is(node.kvHash.toString('hex'), '005c0446ea922e105fc1eb084c88ea4724444a42de49a57748241dad50a2f35d')
  t.is(tx.gets.length, 0)
  t.is(tx.dels.length, 0)
  t.is(tx.puts.length, 1)
  t.deepEqual(tx.puts[0], { key: ':idCounter', value: 2 })
  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.is(db.puts.length, 0)
})

test('create node with initial idCounter', async (t) => {
  let db = mockDb()
  let tx = mockDb()
  let Node = _Node(db, 100)
  let node = new Node({ key: 'foo', value: 'bar' }, tx)
  t.is(node.id, 100)
  t.is(node.key, 'foo')
  t.is(node.value, 'bar')
  t.is(node.hash.toString('hex'), 'f2aafaf4e1a1064eed0fc5e1d7d6844fffaccdde46a3ca5f3885e8a685b9b09e')
  t.is(node.kvHash.toString('hex'), '005c0446ea922e105fc1eb084c88ea4724444a42de49a57748241dad50a2f35d')
  t.is(tx.gets.length, 0)
  t.is(tx.dels.length, 0)
  t.deepEqual(tx.puts, [ { key: ':idCounter', value: 101 } ])
  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.is(db.puts.length, 0)
})

test('access null links', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let tx = mockDb()
  let node = new Node({ key: 'foo', value: 'bar' }, tx)

  async function access (getLink) {
    let tx = mockDb()
    t.is(await getLink.call(node, tx), null)
    t.is(tx.gets.length, 0)
  }

  await access(node.left)
  await access(node.right)
  await access(node.parent)
  await access((tx) => node.child(tx, true))
  await access((tx) => node.child(tx, false))
})

test('save node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' }, db)
  await node.save(db)

  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.deepEqual(db.puts, [
    {
      key: ':idCounter',
      value: 2
    }, {
      key: 'n1',
      value: '8qr69OGhBk7tD8Xh19aET/+szd5Go8pfOIXopoW5sJ4AXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQAAA2ZvbwNiYXIAAAA='
    }
  ])
})

test('save child node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' }, db)
  await node.save(db)

  let tx = mockDb()
  let node2 = new Node({ key: 'fo', value: 'bar' }, tx)
  node = await node.put(node2, tx)
  t.is(node2.id, 2)
  t.is(node2.key, 'fo')

  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.deepEqual(db.puts, [
    { key: ':idCounter', value: 2 },
    {
      key: 'n1',
      value: '8qr69OGhBk7tD8Xh19aET/+szd5Go8pfOIXopoW5sJ4AXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQAAA2ZvbwNiYXIAAAA='
    }
  ])

  t.is(tx.gets.length, 0)
  t.is(tx.dels.length, 0)
  t.deepEqual(tx.puts, [
    { key: ':idCounter', value: 3 },
    {
      key: 'n1',
      value: 'kdtH7c7BAJ1kF/TKklMSIp8c1Cvlecnj1UZm3RcqXTYAXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQEAA2ZvbwNiYXICAAA='
    },
    {
      key: 'n2',
      value: 'zEZVbxfdifMgka4nG9eCnpx3LS9ixfj/wu6e2CpAo5x9utCs1Y1EXhDzBL56jeH6ZEMERAVkNpKMQeDaxBp6NwAAAmZvA2JhcgAAAQ=='
    }
  ])

  let tx2 = mockDb()
  t.is(await node.parent(tx2), null)

  // get parent
  tx = mockDb(tx)
  let parent = await node2.parent(tx)
  t.is(parent.id, 1)
  t.is(parent.key, 'foo')
  t.is(parent.value, 'bar')
  t.deepEqual(tx.gets, [ { key: 'n1' } ])

  // get child
  tx = mockDb(tx)
  let child = await node.left(tx)
  t.is(child.id, 2)
  t.is(child.key, 'fo')
  t.is(child.value, 'bar')
  t.deepEqual(tx.gets, [ { key: 'n2' } ])
})

test('delete child node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' }, db)
  await node.save(db)

  let tx = mockDb(db)
  let node2 = new Node({ key: 'fo', value: 'bar' }, tx)
  node = await node.put(node2, tx)

  tx = mockDb(tx)
  node = await node.delete('fo', tx)

  t.deepEqual(tx.gets, [ { key: 'n2' } ])
  t.deepEqual(tx.dels, [ { key: 'n2' } ])
  t.deepEqual(tx.puts, [
    {
      key: 'n1',
      value: '8qr69OGhBk7tD8Xh19aET/+szd5Go8pfOIXopoW5sJ4AXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQEAA2ZvbwNiYXIAAAA='
    }
  ])

  tx = mockDb(tx)
  t.is(await node.left(tx), null)
})

test('delete parent node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' }, db)
  await node.save(db)

  let tx = mockDb(db)
  let node2 = new Node({ key: 'fo', value: 'bar' }, tx)
  node = await node.put(node2, tx)

  tx = mockDb(tx)
  node = await node.delete('foo', tx)

  t.deepEqual(tx.gets, [ { key: 'n2' } ])
  t.deepEqual(tx.dels, [ { key: 'n1' } ])
  t.is(tx.puts.length, 0)

  tx = mockDb(tx)
  t.is(await node.parent(tx), null)
})

test('build 1000-node tree', async (t) => {
  t.plan(10002)

  let db = mockDb()
  let Node = _Node(db)

  let root = new Node({ key: '0', value: 'value' }, db)
  await root.save(db)

  for (let i = 1; i < 1000; i++) {
    let key = i.toString()
    let node = new Node({ key, value: 'value' }, db)
    root = await root.put(node, db)
  }

  t.is(root.id, 7)
  t.is(root.hash.toString('hex'), 'ae7315867a9ade5ea06a33882ab2e2f4ef4aa044653c7d5685eb7152d78f91fd')

  async function traverse (node) {
    // AVL invariant
    t.true(node.balance() > -2)
    t.true(node.balance() < 2)

    let left = await node.left(db)
    if (left) {
      t.true(left.key < node.key) // in order
      t.is(left.parentId, node.id) // correct parent
      await traverse(left)
    }

    let right = await node.right(db)
    if (right) {
      t.true(node.key < right.key) // in order
      t.is(right.parentId, node.id) // correct parent
      await traverse(right)
    }
  }
  await traverse(root)

  // iterate through all nodes
  let keys = new Array(1000).fill(0).map((_, i) => i.toString()).sort()
  let cursor = await root.min()
  for (let key of keys) {
    t.is(cursor.key, key)
    cursor = await cursor.next()
  }

  // get max
  let max = await root.max()
  t.is(max.key, '999')

  // search
  for (let key of keys) {
    let node = await root.search(key)
    t.is(node.key, key)
  }

  // search for non-existent key
  let node = await root.search('lol')
  t.not(node.key, 'lol')

  // update
  node = new Node({ key: '888', value: 'lol' }, db)
  root = await root.put(node, db)
  node = await root.search('888')
  t.is(root.hash.toString('hex'), 'ae7315867a9ade5ea06a33882ab2e2f4ef4aa044653c7d5685eb7152d78f91fd')
  t.is(node.value, 'lol')
  await traverse(root)
})
