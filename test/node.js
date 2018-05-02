let test = require('ava')
let { mockDb, deepEqual } = require('./common.js')
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
  t.is(node.hash.toString('hex'), '81d19e175fab94e6370784ca6cd1358cd7ac00c9')
  t.is(node.kvHash.toString('hex'), 'ad059f0ac2733621687adbd8c9c414daa550fe88')
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
  deepEqual(t, db.puts, [
    {
      key: 'nfoo',
      value: 'gdGeF1+rlOY3B4TKbNE1jNesAMmtBZ8KwnM2IWh629jJxBTapVD+iAAAA2JhcgAAAA=='
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
  deepEqual(t, db.puts, [
    {
      key: 'nfoo',
      value: 'gdGeF1+rlOY3B4TKbNE1jNesAMmtBZ8KwnM2IWh629jJxBTapVD+iAAAA2JhcgAAAA=='
    }
  ])

  t.is(tx.gets.length, 0)
  t.is(tx.dels.length, 0)
  deepEqual(t, tx.puts, [
    {
      key: 'nfoo',
      value: 'dr+bSNfQWI8G7CldNgJ45LFadOytBZ8KwnM2IWh629jJxBTapVD+iAEAA2JhcgJmbwAA'
    },
    {
      key: 'nfo',
      value: 'iSvPchhS6jutJq9zLLlKIw08Qrq+CXMsEG8Moyl4/gUZIBb1Hxfk7QAAA2JhcgAAA2Zvbw=='
    }
  ])

  let tx2 = mockDb()
  t.is(await node.parent(tx2), null)

  // get parent
  tx = mockDb(tx)
  let parent = await node2.parent(tx)
  t.is(parent.key, 'foo')
  t.is(parent.value, 'bar')
  deepEqual(t, tx.gets, [ { key: 'nfoo' } ])

  // get child
  tx = mockDb(tx)
  let child = await node.left(tx)
  t.is(child.key, 'fo')
  t.is(child.value, 'bar')
  deepEqual(t, tx.gets, [ { key: 'nfo' } ])
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

  deepEqual(t, tx.gets, [ { key: 'nfo' } ])
  deepEqual(t, tx.dels, [ { key: 'nfo' } ])
  deepEqual(t, tx.puts, [
    {
      key: 'nfoo',
      value: 'gdGeF1+rlOY3B4TKbNE1jNesAMmtBZ8KwnM2IWh629jJxBTapVD+iAAAA2JhcgAAAA=='
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

  deepEqual(t, tx.gets, [ { key: 'nfo' } ])
  deepEqual(t, tx.dels, [ { key: 'nfoo' } ])
  t.is(tx.puts.length, 0)
  t.is(node.key, 'fo')

  tx = mockDb(tx)
  t.is(await node.parent(tx), null)
})

test('build 1000-node tree in fixed order', async (t) => {
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

  t.is(root.hash.toString('hex'), '711729815758ca82749d8d37de017fe435136fde')

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
  t.is(root.hash.toString('hex'), 'abc39acc95dce608fa3febf587fb868c39a74543')
  t.is(node.value, 'lol')
  await traverse(root)
})

test('delete non-existent key', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  let root = new Node({ key: '0', value: 'value' })
  await root.save(db)

  try {
    await root.delete('lol')
    t.fail()
  } catch (err) {
    t.is(err.message, 'Key "lol" not found')
  }
})

test('delete (random keys)', async (t) => {
  t.plan(21)

  let db = mockDb()
  let Node = _Node(db)

  // build tree
  let keys = new Array(19).fill(0).map(() => Math.random().toString(36).slice(3))

  let root = new Node({ key: 'root', value: 'value' })
  await root.save(db)
  for (let key of keys) {
    let node = new Node({ key, value: 'value' })
    root = await root.put(node, db)
  }

  keys.push('root')

  for (let key of keys) {
    root = await root.delete(key, db)
    t.pass()
  }

  t.is(root, null)
})

test('get branch', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  // build tree
  let root = new Node({ key: 'root', value: 'value' })
  await root.save(db)
  for (let i = 0; i < 20; i++) {
    let node = new Node({ key: i.toString(), value: 'value' })
    root = await root.put(node, db)
  }

  let branch = await root.getBranchRange('5', '50', db)
  deepEqual(t, branch, {
    left: '+9Y1B580BnXMvHOGmmDe8ADemaE=',
    right: {
      left: {
        left: {
          left: null,
          right: null,
          key: '4',
          value: 'value'
        },
        right: {
          left: null,
          right: null,
          key: '6',
          value: 'value'
        },
        key: '5',
        value: 'value'
      },
      right: 'g0+bmb75LW2OcJOnuuOgIFB7nbI=',
      kvHash: 'tPSczGXYqqu6v5gGmhwWR+1Tcp0='
    },
    kvHash: 'QwyAOeqT0VqrIkc7Yw6xfAf3LPU='
  })
})

test('get branch at edge', async (t) => {
  let db = mockDb()
  let Node = _Node(db)

  // build tree
  let root = new Node({ key: 'root', value: 'value' })
  await root.save(db)
  for (let i = 0; i < 20; i++) {
    let node = new Node({ key: i.toString(), value: 'value' })
    root = await root.put(node, db)
  }

  let branch = await root.getBranchRange('0', '1', db)
  deepEqual(t, branch, {
    left: {
      left: {
        left: {
          left: {
            left: null,
            right: null,
            key: '0',
            value: 'value'
          },
          right: {
            left: null,
            right: null,
            key: '10',
            value: 'value'
          },
          key: '1',
          value: 'value'
        },
        right: 'QoLxbyGXSYWfHDlwLECzo3DR6Ik=',
        kvHash: 'DND2C4D6Pts17le1Ij2ZuPD8orw='
      },
      right: 'r0QwNnr5JKRuItbAUMLrBc9Uyyg=',
      kvHash: 'SxX/9ZfXlgu1Sjc4ajJjEfe2etw='
    },
    right: 'aLtn69Wx/OFlNv25yMazt+gFbxY=',
    kvHash: 'QwyAOeqT0VqrIkc7Yw6xfAf3LPU='
  })
})
