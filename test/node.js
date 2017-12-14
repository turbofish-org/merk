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
