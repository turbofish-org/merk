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
  let Node = _Node(db)
  let node = new Node({ key: 'foo', value: 'bar' })
  t.is(node.key, 'foo')
  t.is(node.value, 'bar')
  t.is(node.hash.toString('hex'), 'f2aafaf4e1a1064eed0fc5e1d7d6844fffaccdde46a3ca5f3885e8a685b9b09e')
  t.is(node.kvHash.toString('hex'), '005c0446ea922e105fc1eb084c88ea4724444a42de49a57748241dad50a2f35d')
  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.is(db.puts.length, 0)
})

test('access null links', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let tx = mockDb()
  let node = new Node({ key: 'foo', value: 'bar' })

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

  let node = new Node({ key: 'foo', value: 'bar' })
  await node.save(db)

  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.deepEqual(db.puts, [
    {
      key: 'nfoo',
      value: '8qr69OGhBk7tD8Xh19aET/+szd5Go8pfOIXopoW5sJ4AXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQAAA2JhcgAAAA=='
    }
  ])
})

test('save child node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' })
  await node.save(db)

  let tx = mockDb()
  let node2 = new Node({ key: 'fo', value: 'bar' })
  node = await node.put(node2, tx)
  t.is(node2.key, 'fo')

  t.is(db.gets.length, 0)
  t.is(db.dels.length, 0)
  t.deepEqual(db.puts, [
    {
      key: 'nfoo',
      value: '8qr69OGhBk7tD8Xh19aET/+szd5Go8pfOIXopoW5sJ4AXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQAAA2JhcgAAAA=='
    }
  ])

  t.is(tx.gets.length, 0)
  t.is(tx.dels.length, 0)
  t.deepEqual(tx.puts, [
    {
      key: 'nfoo',
      value: 'kdtH7c7BAJ1kF/TKklMSIp8c1Cvlecnj1UZm3RcqXTYAXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQEAA2JhcgJmbwAA'
    },
    {
      key: 'nfo',
      value: 'zEZVbxfdifMgka4nG9eCnpx3LS9ixfj/wu6e2CpAo5x9utCs1Y1EXhDzBL56jeH6ZEMERAVkNpKMQeDaxBp6NwAAA2JhcgAAA2Zvbw=='
    }
  ])

  let tx2 = mockDb()
  t.is(await node.parent(tx2), null)

  // get parent
  tx = mockDb(tx)
  let parent = await node2.parent(tx)
  t.is(parent.key, 'foo')
  t.is(parent.value, 'bar')
  t.deepEqual(tx.gets, [ { key: 'nfoo' } ])

  // get child
  tx = mockDb(tx)
  let child = await node.left(tx)
  t.is(child.key, 'fo')
  t.is(child.value, 'bar')
  t.deepEqual(tx.gets, [ { key: 'nfo' } ])
})

test('delete child node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' })
  await node.save(db)

  let tx = mockDb(db)
  let node2 = new Node({ key: 'fo', value: 'bar' })
  node = await node.put(node2, tx)

  tx = mockDb(tx)
  node = await node.delete('fo', tx)

  t.deepEqual(tx.gets, [ { key: 'nfo' } ])
  t.deepEqual(tx.dels, [ { key: 'nfo' } ])
  t.deepEqual(tx.puts, [
    {
      key: 'nfoo',
      value: '8qr69OGhBk7tD8Xh19aET/+szd5Go8pfOIXopoW5sJ4AXARG6pIuEF/B6whMiOpHJERKQt5JpXdIJB2tUKLzXQEAA2JhcgAAAA=='
    }
  ])

  tx = mockDb(tx)
  t.is(await node.left(tx), null)
})

test('delete parent node', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let node = new Node({ key: 'foo', value: 'bar' })
  await node.save(db)

  let tx = mockDb(db)
  let node2 = new Node({ key: 'fo', value: 'bar' })
  node = await node.put(node2, tx)

  tx = mockDb(tx)
  node = await node.delete('foo', tx)

  t.deepEqual(tx.gets, [ { key: 'nfo' } ])
  t.deepEqual(tx.dels, [ { key: 'nfoo' } ])
  t.is(tx.puts.length, 0)

  tx = mockDb(tx)
  t.is(await node.parent(tx), null)
})

test('build 1000-node tree', async (t) => {
  t.plan(9000)

  let db = mockDb()
  let Node = _Node(db)

  let root = new Node({ key: '0', value: 'value' })
  await root.save(db)

  for (let i = 1; i < 1000; i++) {
    let key = i.toString()
    let node = new Node({ key, value: 'value' })
    root = await root.put(node, db)
  }

  t.is(root.hash.toString('hex'), 'ae7315867a9ade5ea06a33882ab2e2f4ef4aa044653c7d5685eb7152d78f91fd')

  async function traverse (node) {
    // AVL invariant
    t.true(node.balance() > -2)
    t.true(node.balance() < 2)

    let left = await node.left(db)
    if (left) {
      t.true(left.key < node.key) // in order
      t.is(left.parentKey, node.key) // correct parent
      await traverse(left)
    }

    let right = await node.right(db)
    if (right) {
      t.true(node.key < right.key) // in order
      t.is(right.parentKey, node.key) // correct parent
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

  // update
  let node = new Node({ key: '888', value: 'lol' })
  root = await root.put(node, db)
  node = await Node.get('888')
  t.is(root.hash.toString('hex'), 'ae7315867a9ade5ea06a33882ab2e2f4ef4aa044653c7d5685eb7152d78f91fd')
  t.is(node.value, 'lol')
  await traverse(root)
})
